//! # WebSocket 相关

use axum::Extension;
use parking_lot::RwLock;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::{Arc, Weak};

use crate::storage::model::prelude::User;
use crate::weixin::WxClient;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, WebSocketUpgrade};
use axum::response::IntoResponse;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use slab::Slab;
use tokio::sync::mpsc::{Receiver, Sender};

const EXPIRE_SECONDS: u64 = 60 * 60;

/// 建立 WebSocket 连接
pub async fn websocket_on_connect(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(session_manager): Extension<SessionManager>,
    Extension(wx_client): Extension<WxClient>,
) -> impl IntoResponse {
    let (id, receiver) = session_manager.accept(addr);
    tracing::info!(%addr, %id, "Websocket connection established.");
    ws.on_upgrade(move |socket| handle_websocket(id, addr, socket, receiver, wx_client))
}

// 处理 WebSocket 连接
async fn handle_websocket(
    id: usize,
    addr: SocketAddr,
    mut socket: WebSocket,
    mut receiver: Receiver<Message>,
    wx_client: WxClient,
) {
    let Some(id) = NonZeroUsize::new(id) else {
        tracing::error!(%id, %addr, "WebSocket id must be a nonzero usize");
        return;
    };
    loop {
        tokio::select! {
            recv = socket.recv() => {
                let Some(result) = recv else {
                    tracing::info!(%id, %addr, "WebSocket closed by client.");
                    return;
                };

                let message: axum::extract::ws::Message = match result {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::error!(%id, %error, "Received error from websocket.");
                        return;
                    }
                };

                tracing::info!(%id, ?message, "Received message from websocket.");
                match message {
                    Message::Text(json) => {
                        let req = match serde_json::from_str::<Req>(&json) {
                            Ok(req) => req,
                            Err(error) => {
                                tracing::error!(%id, %error, %json, "Failed to deserialize json request from client.");
                                break;
                            }
                        };

                        match req  {
                            Req {
                                r#type: ReqType::Heartbeat,
                                ..
                            } => {
                                // do nothing
                            }
                            Req {
                                r#type: ReqType::Login,
                                ..
                            } => {
                                match wx_client.get_qrcode_tick_by_id(EXPIRE_SECONDS, false, id).await {
                                    Ok(ticket) => {
                                        let resp = Resp {
                                            r#type: RespType::LoginUrl,
                                            data: LoginUrl {
                                                login_url: ticket.url
                                            }
                                        };
                                        match serde_json::to_string(&resp) {
                                            Ok(json) => {
                                                if let Err(error) = socket.send(Message::Text(json)).await {
                                                    tracing::error!(%id, %addr, %error, ?resp, "Failed to send response");
                                                    break;
                                                }
                                            }
                                            Err(error) => {
                                                tracing::error!(%id, %addr, %error, ?resp, "Failed to serialize response");
                                                break;
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        tracing::error!(%id, %error, "Failed to get QRCode tick by id");
                                    }
                                }
                            }
                            Req {
                                r#type: ReqType::Authorize,
                                data: Some(data),
                            } => {
                                tracing::info!(%id, %data, "Received authorize request");
                            }
                            unexpected_req => {
                                tracing::warn!(%id, ?unexpected_req, "Received unexpected request from websocket");
                            }
                        }
                    }
                    Message::Ping(bytes) => {
                        if let Err(error) = socket.send(Message::Pong(bytes)).await {
                            tracing::error!(%id, %error, "Failed to send pong back.");
                            break;
                        }
                    }
                    unexpected_message => {
                        tracing::warn!(%id, ?unexpected_message, "Received unexpected message from websocket");
                    }
                }
            }
            to_send = receiver.recv() => {
                let Some(message) = to_send else {
                    tracing::info!(%id, %addr, "Sender dropped by server.");
                    break;
                };

                if let Err(error) = socket.send(message).await {
                    tracing::error!(%id, %error, "Failed to send message to client");
                    break;
                }
            }
        }
    }
}

/// WebSocket 请求
///
/// btw: 按照 Java 的方式实现太弱了，没有**和类型**
#[derive(Debug, Deserialize)]
pub struct Req {
    /// 请求类型
    pub r#type: ReqType,
    /// 请求数据
    pub data: Option<String>,
}

/// WebSocket 请求类型
#[derive(Debug, serde_repr::Deserialize_repr)]
#[repr(u8)]
pub enum ReqType {
    /// 登录
    Login = 1,
    /// 心跳
    Heartbeat = 2,
    /// 登录
    Authorize = 3,
}

/// WebSocket 响应类型
#[derive(Debug, serde_repr::Serialize_repr)]
#[repr(u8)]
pub enum RespType {
    /// 登录二维码返回
    LoginUrl = 1,
    /// 用户扫描成功等待授权
    LoginScanSuccess = 2,
    /// 用户登录成功返回用户信息
    LoginSuccess = 3,
}

/// WebSocket 响应
#[derive(Debug, Serialize)]
pub struct Resp<T> {
    /// 响应类型
    pub r#type: RespType,
    /// 响应数据
    pub data: T,
}

/// 登录 Url
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginUrl {
    login_url: String,
}

/// 登录认证
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authorize {
    token: String,
}

/// 角色
#[derive(Debug)]
pub enum Role {
    /// 游客
    Guest,
    /// 已登录用户
    Authenticated {
        /// 用户信息
        user: User,
    },
}

/// # WebSocket session
#[derive(Debug)]
pub struct Session {
    /// ID
    pub id: Id,
    /// IP 地址
    pub ip_addr: SocketAddr,
    /// 角色
    pub role: Role,
    /// 消息发送器
    pub sender: Sender<Message>,
}

impl Session {
    /// 发送消息
    pub async fn send(&self, msg: Message) -> anyhow::Result<()> {
        Ok(self.sender.send(msg).await?)
    }
}

/// # Session 管理器
#[derive(Debug, Default, Clone)]
pub struct SessionManager {
    id_gen: IdGenerator,
    sessions: Arc<DashMap<usize, Session>>,
}

impl SessionManager {
    /// 接收一个 WebSocket 连接
    pub fn accept(&self, ip_addr: SocketAddr) -> (usize, Receiver<Message>) {
        let id = self.id_gen.generate();
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        let ws_id = id.id();
        self.sessions.insert(
            id.id(),
            Session {
                id,
                ip_addr,
                role: Role::Guest,
                sender,
            },
        );
        (ws_id, receiver)
    }

    /// 获取某个连接的引用
    pub async fn try_send<T: Serialize>(&self, id: usize, resp: &Resp<T>) -> anyhow::Result<bool> {
        if let Some(pair) = self.sessions.get_mut(&id) {
            pair.sender
                .send(Message::Text(serde_json::to_string(resp)?))
                .await?;
        }

        Ok(false)
    }
}

/// # WebSocket ID 生成器
///
/// 生成不重复的非 0 无符号整数
#[derive(Debug, Clone)]
pub struct IdGenerator {
    slab: Arc<RwLock<Slab<()>>>,
}

impl Default for IdGenerator {
    fn default() -> Self {
        let mut slab = Slab::with_capacity(1024);
        // preserve zero
        let _zero = slab.insert(());

        Self {
            slab: Arc::new(RwLock::new(slab)),
        }
    }
}

impl IdGenerator {
    /// 生成一个新的 ID，会在离开作用域时自动释放
    pub fn generate(&self) -> Id {
        let id = {
            let mut write = self.slab.write();
            write.insert(())
        };

        Id {
            id,
            slab: Arc::downgrade(&self.slab),
        }
    }
}

/// 生成的可自动释放的 ID
#[derive(Debug)]
pub struct Id {
    id: usize,
    slab: Weak<RwLock<Slab<()>>>,
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl PartialEq<Self> for Id {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl Eq for Id {}

impl Id {
    /// ID 值
    pub fn id(&self) -> usize {
        self.id
    }
}

impl From<Id> for usize {
    fn from(value: Id) -> Self {
        value.id
    }
}

impl AsRef<usize> for Id {
    fn as_ref(&self) -> &usize {
        &self.id
    }
}

impl Drop for Id {
    fn drop(&mut self) {
        if let Some(slab) = self.slab.upgrade() {
            slab.write().remove(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::handler::ws::IdGenerator;

    #[test]
    fn id_manager() {
        let id_manager = IdGenerator::default();
        {
            let id1 = id_manager.generate();
            let id2 = id_manager.generate();
            assert_eq!(id1.id(), 1);
            assert_eq!(id2.id(), 2);
        }

        // id1 and id2 should have been removed
        let id3 = id_manager.generate();
        let id4 = id_manager.generate();
        assert_eq!(id3.id(), 1);
        assert_eq!(id4.id(), 2);
        drop(id4);
        let id5 = id_manager.generate();
        assert_eq!(id5.id(), 2);
        drop(id3);
        let id6 = id_manager.generate();
        assert_eq!(id6.id(), 1);
    }
}
