//! # 微信 API 交互接口
//!

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Extension, Router};
use axum_valid::Valid;
use serde::Deserialize;
use validator::Validate;

use crate::weixin::{
    WxClient, WxEncryptedRawXmlMessage, WxEvent, WxEventType, WxMessage, WxMessageData,
    WxRawXmlMessage, WxServerParam,
};

/// 微信 API 相关路由
pub fn route() -> Router {
    Router::new().nest(
        "/wx/portal/public",
        Router::new()
            .route("/", get(echo_str))
            .route("/", post(wx_post))
            .route("/callBack", get(call_back)),
    )
}

/// 认证参数
#[derive(Debug, Validate, Deserialize)]
pub struct EchoStr {
    /// 随机字符串
    #[validate(length(min = 1))]
    pub echostr: String,
}

///认证
#[utoipa::path(get, path = "/wx/portal/public")]
pub async fn echo_str(
    Extension(wx): Extension<WxClient>,
    Valid(Query(param)): Valid<Query<WxServerParam<EchoStr>>>,
) -> impl IntoResponse {
    if param.is_signature_valid(wx.token()) {
        (StatusCode::OK, param.data.echostr)
    } else {
        (StatusCode::BAD_REQUEST, String::new())
    }
}

/// 认证回调参数
#[derive(Debug, Validate, Deserialize)]
pub struct CallBackParam {
    /// code
    #[validate(length(min = 1))]
    pub code: String,
}

/// 认证回调
#[utoipa::path(get, path = "/wx/portal/public/callBack")]
pub async fn call_back(
    Valid(Query(_param)): Valid<Query<WxServerParam<CallBackParam>>>,
) -> impl IntoResponse {
    // WxOAuth2AccessToken accessToken = wxService.getOAuth2Service().getAccessToken(code);
    // WxOAuth2UserInfo userInfo = wxService.getOAuth2Service().getUserInfo(accessToken, "zh_CN");
    // wxMsgService.authorize(userInfo);
    Redirect::to("https://mp.weixin.qq.com/")
}

/// 微信请求接收参数
#[derive(Debug, Deserialize, Validate)]
pub struct PostParam {
    /// Open ID
    pub openid: String,
    /// 加密类型
    pub encrypt_type: Option<String>,
    /// 消息签名
    pub msg_signature: Option<String>,
}

/// post
#[utoipa::path(post, path = "/wx/portal/public")]
pub async fn wx_post(
    Valid(Query(param)): Valid<Query<WxServerParam<PostParam>>>,
    Extension(wx_app): Extension<WxClient>,
    data: String,
) -> impl IntoResponse {
    tracing::info!(?param, %data, "wx_post");

    if !param.is_signature_valid(wx_app.token()) {
        return (StatusCode::BAD_REQUEST, String::new());
    }

    if let Some(encrypt_type) = param.data.encrypt_type {
        if encrypt_type.eq_ignore_ascii_case("aes") {
            let encrypted_message = match serde_xml_rs::from_str::<WxEncryptedRawXmlMessage>(&data)
            {
                Ok(message) => message,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()),
            };

            let (from_app_id, message) =
                match encrypted_message.aes_decrypt(wx_app.encoding_aes_key()) {
                    Ok(decrypted) => decrypted,
                    Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()),
                };

            tracing::info!(%from_app_id, ?message, "Received a encrypted message from weixin.");
            if let WxMessageData::Event { event } = &message.data {
                if let WxEvent {
                    event: event @ WxEventType::Subscribe,
                    event_key: Some(event_key),
                    ticket: Some(ticket),
                }
                | WxEvent {
                    event: event @ WxEventType::Scan,
                    event_key: Some(event_key),
                    ticket: Some(ticket),
                } = event
                {
                    const EVENT_KEY_PREFIX: &str = "qrscene_";
                    let event_key: usize =
                        match if let Some(stripped) = event_key.strip_prefix(EVENT_KEY_PREFIX) {
                            stripped.parse()
                        } else {
                            event_key.parse()
                        } {
                            Ok(event_key) => event_key,
                            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()),
                        };
                    tracing::info!(?event, %event_key, %ticket, "Received event");
                }
            }
            (StatusCode::OK, String::new())
        } else {
            (
                StatusCode::NOT_IMPLEMENTED,
                format!("unsupported encryption algorithm: {}", encrypt_type),
            )
        }
    } else {
        let raw = match serde_xml_rs::from_str::<WxRawXmlMessage>(&data) {
            Ok(message) => message,
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
        };

        let message = match WxMessage::try_from(raw) {
            Ok(message) => message,
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
        };

        tracing::info!(?message, "Received a unencrypted message from weixin.");
        (StatusCode::OK, String::new())
    }
}
