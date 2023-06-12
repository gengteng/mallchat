//! # WebSocket 相关

use axum::Extension;
use parking_lot::RwLock;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::{Arc, Weak};

use crate::storage::model::prelude::User;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, WebSocketUpgrade};
use axum::response::IntoResponse;
use dashmap::DashMap;
use slab::Slab;
use tokio::sync::mpsc::{Receiver, Sender};

/// 建立 WebSocket 连接
pub async fn websocket_on_connect(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(session_manager): Extension<SessionManager>,
) -> impl IntoResponse {
    let receiver = session_manager.accept(addr);
    tracing::info!("Websocket connection from {}", addr);
    ws.on_upgrade(move |socket| handle_websocket(addr, socket, receiver))
}

// 处理 WebSocket 连接
async fn handle_websocket(
    addr: SocketAddr,
    mut socket: WebSocket,
    mut receiver: Receiver<Message>,
) {
    loop {
        tokio::select! {
            recv = socket.recv() => {
                let Some(result) = recv else {
                    tracing::info!(%addr, "WebSocket closed by client.");
                    return;
                };

                let message: axum::extract::ws::Message = match result {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::error!(%error, "Received error from websocket.");
                        return;
                    }
                };

                tracing::info!(?message, "Received message from websocket.");
            }
            to_send = receiver.recv() => {
                let Some(message) = to_send else {
                    tracing::info!(%addr, "Sender dropped by server.");
                    break;
                };

                if let Err(error) = socket.send(message).await {
                    tracing::error!(%error, "Failed to send message to client");
                    break;
                }
            }
        }
    }
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
    pub fn accept(&self, ip_addr: SocketAddr) -> Receiver<Message> {
        let id = self.id_gen.generate();
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        self.sessions.insert(
            id.id(),
            Session {
                id,
                ip_addr,
                role: Role::Guest,
                sender,
            },
        );
        receiver
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
