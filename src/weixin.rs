//! # 微信公众平台访问相关
//!

pub mod xml;

use base64::Engine;
use reqwest::Method;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha1::{Digest, Sha1};
use std::fmt::{Debug, Display, Formatter};
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use validator::Validate;

/// 微信公众平台配置
#[derive(Debug, Serialize, Deserialize)]
pub struct WxConfig {
    /// 开发者ID
    pub app_id: String,
    /// 开发者密码
    pub app_secret: String,
    /// 令牌
    pub token: String,
    /// 消息加解密密钥
    pub encoding_aes_key: WxEncodingAesKey,
    /// 超时时间
    #[serde(default = "default::timeout_secs")]
    pub timeout_secs: u64,
}

mod default {
    pub fn timeout_secs() -> u64 {
        10
    }
}

/// 微信公众平台客户端
#[derive(Clone)]
pub struct WxClient {
    config: Arc<WxConfig>,
    client: reqwest::Client,
    access_token: Arc<RwLock<WxAccessToken>>,
}

impl Debug for WxClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WxApp {{ config: {:?}, client, access_token }}",
            self.config
        )
    }
}

impl WxClient {
    /// 新建一个 微信客户端
    pub async fn new(config: WxConfig) -> anyhow::Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;
        let access_token = Self::get_access_token(&client, &config).await?;
        Ok(Self {
            config: Arc::new(config),
            client,
            access_token: Arc::new(RwLock::new(WxAccessToken::from(access_token))),
        })
    }
    /// 开发者 ID
    pub fn app_id(&self) -> &str {
        self.config.app_id.as_str()
    }
    /// 开发者密码
    pub fn app_secret(&self) -> &str {
        self.config.app_secret.as_str()
    }
    /// 令牌
    pub fn token(&self) -> &str {
        self.config.token.as_str()
    }
    /// 消息加解密密钥
    pub fn encoding_aes_key(&self) -> &WxEncodingAesKey {
        &self.config.encoding_aes_key
    }
    /// 刷新 access_token
    pub async fn update_access_token(&self) -> anyhow::Result<()> {
        let need_update = {
            let read = self.access_token.read().await;
            read.expired()
        };
        if need_update {
            let mut write = self.access_token.write().await;
            let need_update = write.expired();
            if need_update {
                let access_token =
                    Self::get_access_token(&self.client, self.config.as_ref()).await?;
                *write = access_token.into();
            }
        }
        Ok(())
    }
    /// 获取 access_token
    pub async fn get_access_token(
        client: &reqwest::Client,
        wx_config: &WxConfig,
    ) -> anyhow::Result<AccessToken> {
        #[derive(Serialize)]
        struct GetAccessToken<'a> {
            grant_type: &'a str,
            appid: &'a str,
            secret: &'a str,
        }
        let query = GetAccessToken {
            grant_type: "client_credential",
            appid: &wx_config.app_id,
            secret: &wx_config.app_secret,
        };
        let resp = client
            .request(Method::GET, "https://api.weixin.qq.com/cgi-bin/token")
            .query(&query)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("Response status is not OK: {}", status);
        }

        let result: WxResult<AccessToken> = resp.json().await?;
        result.into()
    }
    /// 获取
    pub async fn get_qrcode_tick_by_id(
        &self,
        expire_seconds: impl Into<Option<u64>>,
        limit: bool,
        scene_id: NonZeroUsize,
    ) -> anyhow::Result<QrCodeTicket> {
        self.update_access_token().await?;
        let read = self.access_token.read().await;
        let access_token = read.query();
        #[derive(Serialize)]
        pub struct Scene {
            scene_id: NonZeroUsize,
        }
        #[derive(Serialize)]
        struct ActionInfo {
            scene: Scene,
        }
        #[derive(Serialize)]
        struct GetQrCodeById<'a> {
            expire_seconds: Option<u64>,
            action_name: &'a str,
            action_info: ActionInfo,
        }

        let body = GetQrCodeById {
            expire_seconds: expire_seconds.into(),
            action_name: if limit { "QR_LIMIT_SCENE" } else { "QR_SCENE" },
            action_info: ActionInfo {
                scene: Scene { scene_id },
            },
        };

        let resp = self
            .client
            .request(
                Method::POST,
                "https://api.weixin.qq.com/cgi-bin/qrcode/create",
            )
            .query(&[access_token])
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("Response status is not OK: {}", status);
        }

        let result: WxResult<QrCodeTicket> = resp.json().await?;
        result.into()
    }
}

/// 微信服务器消息参数
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct WxServerParam<T> {
    /// 微信加密签名，signature结合了开发者填写的token参数和请求中的timestamp参数、nonce参数。
    #[validate(length(min = 1))]
    pub signature: String,
    /// 时间戳
    #[validate(length(min = 1))]
    pub timestamp: String,
    /// 随机数
    #[validate(length(min = 1))]
    pub nonce: String,
    /// 数据
    #[serde(flatten)]
    pub data: T,
}

impl<T> WxServerParam<T> {
    /// 判断签名是否合法
    pub fn is_signature_valid(&self, token: &str) -> bool {
        let mut array = [token, self.timestamp.as_str(), self.nonce.as_str()];
        array.sort();
        let mut hasher = Sha1::default();
        for s in array {
            hasher.update(s);
        }
        let result = hasher.finalize();
        let calculated_signature = hex::encode(result);
        calculated_signature == self.signature
    }
}

/// 消息类型
#[derive(Debug, Deserialize)]
pub enum WxMessageType {
    /// 文本
    #[serde(rename = "text")]
    Text,
    /// 图片
    #[serde(rename = "image")]
    Image,
    /// 视频
    #[serde(rename = "video")]
    Video,
    /// 语音
    #[serde(rename = "voice")]
    Voice,
    /// 短视频
    #[serde(rename = "shortvideo")]
    ShortVideo,
    /// 事件
    #[serde(rename = "event")]
    Event,
}

impl Serialize for WxMessageType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            WxMessageType::Text => "text",
            WxMessageType::Image => "image",
            WxMessageType::Video => "video",
            WxMessageType::Voice => "voice",
            WxMessageType::ShortVideo => "shortvideo",
            WxMessageType::Event => "event",
        })
    }
}

/// 原始加密过的 XMl 消息
#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", rename = "xml")]
pub struct WxEncryptedRawXmlMessage {
    pub to_user_name: String,
    pub encrypt: String,
}

impl WxEncryptedRawXmlMessage {
    /// 使用 AES256 解密
    ///
    /// 返回：from_app_id 和 消息
    pub fn aes_decrypt(
        &self,
        encoding_aes_key: &WxEncodingAesKey,
    ) -> anyhow::Result<(String, WxMessage)> {
        use aes::cipher::KeyIvInit;
        use aes::cipher::{block_padding::NoPadding, BlockDecryptMut};
        type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

        let mut decrypted = base64::engine::general_purpose::STANDARD.decode(&self.encrypt)?;
        let mut iv = [0u8; 16];
        iv.copy_from_slice(&encoding_aes_key.data[0..16]);
        let data = encoding_aes_key.data;
        let pt = Aes256CbcDec::new(&data.into(), &iv.into())
            .decrypt_padded_mut::<NoPadding>(&mut decrypted)
            .map_err(|e| anyhow::anyhow!("Failed to decrypted data: {e}"))?;
        let mut pad = pt[pt.len() - 1] as usize;
        if !(1..=32).contains(&pad) {
            pad = 0;
        }
        let no_padding = &pt[0..pt.len() - pad];
        let no_padding_len = no_padding.len();
        if no_padding_len < 20 {
            anyhow::bail!("Data length is less than 20");
        }
        let xml_len = match &no_padding[16..20] {
            [b0, b1, b2, b3] => u32::from_be_bytes([*b0, *b1, *b2, *b3]),
            _ => anyhow::bail!("impossible: slice[16..20] length is not 4"),
        } as usize;
        if 20 + xml_len > no_padding_len {
            anyhow::bail!("Not enough data: xml_len={xml_len}, no_padding_len={no_padding_len}");
        }
        let xml_content = std::str::from_utf8(&no_padding[20..20 + xml_len])?;
        let from_appid = std::str::from_utf8(&no_padding[20 + xml_len..no_padding.len()])?;
        let raw = serde_xml_rs::from_str::<WxRawXmlMessage>(xml_content)?;
        Ok((from_appid.to_string(), WxMessage::try_from(raw)?))
    }
}

/// 原始 XMl 消息
#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", rename = "xml")]
pub struct WxRawXmlMessage {
    pub to_user_name: String,
    pub from_user_name: String,
    pub create_time: i32,
    pub msg_type: WxMessageType,
    pub content: Option<String>,
    pub pic_url: Option<String>,
    pub media_id: Option<String>,
    pub format: Option<String>,
    pub recognition: Option<String>,
    pub thumb_media_id: Option<String>,
    pub msg_id: Option<i64>,
    pub msg_data_id: Option<String>,
    pub idx: Option<String>,
    pub event: Option<String>,
    pub event_key: Option<String>,
    pub ticket: Option<String>,
}

/// 消息
#[derive(Debug)]
pub struct WxMessage {
    /// 接收方微信号
    pub to_user_name: String,
    /// 发送方微信号，若为普通用户，则是一个OpenID
    pub from_user_name: String,
    /// 消息创建时间
    pub create_time: i32,
    /// 消息数据
    pub data: WxMessageData,
    /// 消息id，64位整型
    pub msg_id: Option<i64>,
    /// 消息的数据ID（消息如果来自文章时才有）
    pub msg_data_id: Option<String>,
    /// 多图文时第几篇文章，从1开始（消息如果来自文章时才有）
    pub idx: Option<String>,
}

/// 消息数据
#[derive(Debug)]
pub enum WxMessageData {
    /// 文本
    Text {
        /// 文本消息内容
        content: String,
    },
    /// 图片
    Image {
        /// 图片链接（由系统生成）
        pic_url: String,
        /// 图片消息媒体id，可以调用获取临时素材接口拉取数据。
        media_id: String,
    },
    /// 视频
    Video {
        /// 语音消息媒体id，可以调用获取临时素材接口拉取数据。
        media_id: String,
        /// 视频消息缩略图的媒体id，可以调用多媒体文件下载接口拉取数据。
        thumb_media_id: String,
    },
    /// 语音
    Voice {
        /// 语音消息媒体id，可以调用获取临时素材接口拉取数据。
        media_id: String,
        /// 语音格式，如amr，speex等
        format: String,
        /// 语音识别结果，UTF8编码
        recognition: Option<String>,
    },
    /// 短视频
    ShortVideo {
        /// 语音消息媒体id，可以调用获取临时素材接口拉取数据。
        media_id: String,
        /// 视频消息缩略图的媒体id，可以调用多媒体文件下载接口拉取数据。
        thumb_media_id: String,
    },
    /// 事件
    Event {
        /// 事件类型
        event: WxEvent,
    },
}

/// 微信事件类型
#[derive(Debug)]
pub enum WxEventType {
    /// 订阅
    Subscribe,
    /// 取消订阅
    Unsubscribe,
    /// 扫码
    Scan,
}

impl FromStr for WxEventType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "subscribe" => Ok(WxEventType::Subscribe),
            "unsubscribe" => Ok(WxEventType::Unsubscribe),
            "SCAN" => Ok(WxEventType::Scan),
            _ => anyhow::bail!("Unknown event type: {s}"),
        }
    }
}

impl Display for WxEventType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                WxEventType::Subscribe => "subscribe",
                WxEventType::Unsubscribe => "unsubscribe",
                WxEventType::Scan => "SCAN",
            }
        )
    }
}

/// 微信事件数据
#[derive(Debug)]
pub struct WxEvent {
    /// 事件类型
    pub event: WxEventType,
    /// 事件 KEY 值
    pub event_key: Option<String>,
    /// 二维码的ticket，可用来换取二维码图片
    pub ticket: Option<String>,
}

impl From<WxMessage> for WxRawXmlMessage {
    fn from(msg: WxMessage) -> Self {
        let WxMessage {
            to_user_name,
            from_user_name,
            create_time,
            data,
            msg_id,
            msg_data_id,
            idx,
        } = msg;
        match data {
            WxMessageData::Text { content } => WxRawXmlMessage {
                to_user_name,
                from_user_name,
                create_time,
                msg_type: WxMessageType::Text,
                content: Some(content),
                pic_url: None,
                media_id: None,
                format: None,
                recognition: None,
                thumb_media_id: None,
                msg_id,
                msg_data_id,
                idx,
                event: None,
                event_key: None,
                ticket: None,
            },
            WxMessageData::Image { pic_url, media_id } => WxRawXmlMessage {
                to_user_name,
                from_user_name,
                create_time,
                msg_type: WxMessageType::Image,
                content: None,
                pic_url: Some(pic_url),
                media_id: Some(media_id),
                format: None,
                recognition: None,
                thumb_media_id: None,
                msg_id,
                msg_data_id,
                idx,
                event: None,
                event_key: None,
                ticket: None,
            },
            WxMessageData::Video {
                media_id,
                thumb_media_id,
            } => WxRawXmlMessage {
                to_user_name,
                from_user_name,
                create_time,
                msg_type: WxMessageType::Video,
                content: None,
                pic_url: None,
                media_id: Some(media_id),
                format: None,
                recognition: None,
                thumb_media_id: Some(thumb_media_id),
                msg_id,
                msg_data_id,
                idx,
                event: None,
                event_key: None,
                ticket: None,
            },
            WxMessageData::Voice {
                media_id,
                format,
                recognition,
            } => WxRawXmlMessage {
                to_user_name,
                from_user_name,
                create_time,
                msg_type: WxMessageType::Voice,
                content: None,
                pic_url: None,
                media_id: Some(media_id),
                format: Some(format),
                recognition,
                thumb_media_id: None,
                msg_id,
                msg_data_id,
                idx,
                event: None,
                event_key: None,
                ticket: None,
            },
            WxMessageData::ShortVideo {
                media_id,
                thumb_media_id,
            } => WxRawXmlMessage {
                to_user_name,
                from_user_name,
                create_time,
                msg_type: WxMessageType::ShortVideo,
                content: None,
                pic_url: None,
                media_id: Some(media_id),
                format: None,
                recognition: None,
                thumb_media_id: Some(thumb_media_id),
                msg_id,
                msg_data_id,
                idx,
                event: None,
                event_key: None,
                ticket: None,
            },
            WxMessageData::Event {
                event:
                    WxEvent {
                        event,
                        event_key: key,
                        ticket,
                    },
            } => WxRawXmlMessage {
                to_user_name,
                from_user_name,
                create_time,
                msg_type: WxMessageType::Event,
                content: None,
                pic_url: None,
                media_id: None,
                format: None,
                recognition: None,
                thumb_media_id: None,
                msg_id: None,
                msg_data_id: None,
                idx: None,
                event: Some(event.to_string()),
                event_key: key,
                ticket,
            },
        }
    }
}

impl TryFrom<WxRawXmlMessage> for WxMessage {
    type Error = anyhow::Error;

    fn try_from(raw_msg: WxRawXmlMessage) -> Result<Self, Self::Error> {
        let data = match raw_msg.msg_type {
            WxMessageType::Text => WxMessageData::Text {
                content: raw_msg
                    .content
                    .ok_or_else(|| anyhow::anyhow!("Missing content for text message"))?,
            },
            WxMessageType::Image => WxMessageData::Image {
                pic_url: raw_msg
                    .pic_url
                    .ok_or_else(|| anyhow::anyhow!("Missing pic_url for image message"))?,
                media_id: raw_msg
                    .media_id
                    .ok_or_else(|| anyhow::anyhow!("Missing media_id for image message"))?,
            },
            WxMessageType::Video => WxMessageData::Video {
                media_id: raw_msg
                    .media_id
                    .ok_or_else(|| anyhow::anyhow!("Missing media_id for video message"))?,
                thumb_media_id: raw_msg
                    .thumb_media_id
                    .ok_or_else(|| anyhow::anyhow!("Missing thumb_media_id for video message"))?,
            },
            WxMessageType::Voice => WxMessageData::Voice {
                media_id: raw_msg
                    .media_id
                    .ok_or_else(|| anyhow::anyhow!("Missing media_id for voice message"))?,
                format: raw_msg
                    .format
                    .ok_or_else(|| anyhow::anyhow!("Missing format for voice message"))?,
                recognition: raw_msg.recognition,
            },
            WxMessageType::ShortVideo => WxMessageData::ShortVideo {
                media_id: raw_msg
                    .media_id
                    .ok_or_else(|| anyhow::anyhow!("Missing media_id for short video message"))?,
                thumb_media_id: raw_msg.thumb_media_id.ok_or_else(|| {
                    anyhow::anyhow!("Missing thumb_media_id for short video message")
                })?,
            },
            WxMessageType::Event => WxMessageData::Event {
                event: WxEvent {
                    event: raw_msg
                        .event
                        .ok_or_else(|| anyhow::anyhow!("Missing event for event message"))?
                        .parse()?,
                    event_key: raw_msg.event_key,
                    ticket: raw_msg.ticket,
                },
            },
        };
        Ok(WxMessage {
            to_user_name: raw_msg.to_user_name,
            from_user_name: raw_msg.from_user_name,
            create_time: raw_msg.create_time,
            data,
            msg_id: raw_msg.msg_id,
            msg_data_id: raw_msg.msg_data_id,
            idx: raw_msg.idx,
        })
    }
}

/// 微信公众平台 EncodingAesKey 参数
#[derive(Debug)]
pub struct WxEncodingAesKey {
    data: [u8; 32],
}

impl FromStr for WxEncodingAesKey {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 43 {
            anyhow::bail!("The base64 string length is not 43");
        }
        let decoded = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(s)
            .map_err(|e| anyhow::anyhow!("Failed to decode base64 string: {e}"))?;
        let len = decoded.len();
        let data =
            TryFrom::try_from(decoded).map_err(|_| anyhow::anyhow!("Invalid data size: {len}"))?;
        Ok(WxEncodingAesKey { data })
    }
}

impl Display for WxEncodingAesKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(self.data)
        )
    }
}

impl AsRef<[u8; 32]> for WxEncodingAesKey {
    fn as_ref(&self) -> &[u8; 32] {
        &self.data
    }
}

impl<'de> Deserialize<'de> for WxEncodingAesKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EncodingAesKeyVisitor;
        impl Visitor<'_> for EncodingAesKeyVisitor {
            type Value = WxEncodingAesKey;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "A base64 string of length 43 that does not contain an equal sign."
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                WxEncodingAesKey::from_str(v).map_err(|e| Error::custom(e.to_string()))
            }
        }

        deserializer.deserialize_str(EncodingAesKeyVisitor)
    }
}

impl Serialize for WxEncodingAesKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let output = self.to_string();
        serializer.serialize_str(&output)
    }
}

/// 微信公众平台 AccessToken
#[derive(Debug, Deserialize)]
pub struct AccessToken {
    access_token: String,
    expires_in: u64,
}

/// 二维码结果
#[derive(Debug, Deserialize)]
pub struct QrCodeTicket {
    /// 获取的二维码ticket，凭借此ticket可以在有效时间内换取二维码。
    pub ticket: String,
    /// 该二维码有效时间，以秒为单位。 最大不超过2592000（即30天）。
    pub expire_seconds: u64,
    /// 二维码图片解析后的地址，开发者可根据该地址自行生成需要的二维码图片
    pub url: String,
}

/// 微信公众平台 AccessToken
#[derive(Debug)]
pub struct WxAccessToken {
    token: AccessToken,
    timestamp: u64,
}

impl From<AccessToken> for WxAccessToken {
    fn from(token: AccessToken) -> Self {
        Self {
            token,
            timestamp: current_second(),
        }
    }
}

impl WxAccessToken {
    /// 获取 token 用于请求时使用的参数
    pub fn query(&self) -> (&str, &str) {
        ("access_token", &self.token.access_token)
    }
    /// 获取 token
    pub fn token(&self) -> &str {
        &self.token.access_token
    }
    /// 判断是否过期
    pub fn expired(&self) -> bool {
        self.timestamp + self.token.expires_in <= current_second()
    }
}

fn current_second() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock went backwards")
        .as_secs()
}

/// 微信接口返回结果
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WxResult<T> {
    /// 成功
    Success(T),
    /// 错误
    Error {
        /// 错误码
        errcode: u64,
        /// 错误消息
        errmsg: String,
    },
}

impl<T> From<WxResult<T>> for anyhow::Result<T> {
    fn from(value: WxResult<T>) -> Self {
        match value {
            WxResult::Success(value) => Ok(value),
            WxResult::Error { errcode, errmsg } => Err(anyhow::anyhow!(
                "Weixin server responded with error: {} {}",
                errcode,
                errmsg
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::weixin::{AccessToken, WxResult};

    #[test]
    fn wx_result() -> anyhow::Result<()> {
        let success = r#"{"access_token":"ACCESS_TOKEN","expires_in":7200}"#;
        let error = r#"{"errcode":40013,"errmsg":"invalid appid"}"#;

        let success = serde_json::from_str::<WxResult<AccessToken>>(success)?;
        let error = serde_json::from_str::<WxResult<AccessToken>>(error)?;
        println!("{:?}\n{:?}", success, error);
        Ok(())
    }
}
