//! # 微信 API 交互接口
//!

use crate::handler::auth::current_millisecond;
use crate::handler::ws::{Resp, RespType, SessionManager};
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Extension, Router};
use axum_valid::Valid;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use validator::Validate;

use crate::weixin::xml::Xml;
use crate::weixin::{
    WxClient, WxConfig, WxEncryptedRawXmlMessage, WxEvent, WxEventType, WxMessage, WxMessageData,
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
    Extension(connection): Extension<DatabaseConnection>,
    Extension(session_manager): Extension<SessionManager>,
    data: String,
) -> Response {
    tracing::info!(?param, %data, "wx_post");

    if !param.is_signature_valid(wx_app.token()) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let message = if let Some(encrypt_type) = param.data.encrypt_type {
        if encrypt_type.eq_ignore_ascii_case("aes") {
            let encrypted_message = match serde_xml_rs::from_str::<WxEncryptedRawXmlMessage>(&data)
            {
                Ok(message) => message,
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            };

            let (from_app_id, message) =
                match encrypted_message.aes_decrypt(wx_app.encoding_aes_key()) {
                    Ok(decrypted) => decrypted,
                    Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
                };

            tracing::info!(%from_app_id, ?message, "Received a encrypted message from weixin.");
            message
        } else {
            return (
                StatusCode::NOT_IMPLEMENTED,
                format!("unsupported encryption algorithm: {}", encrypt_type),
            )
                .into_response();
        }
    } else {
        let raw = match serde_xml_rs::from_str::<WxRawXmlMessage>(&data) {
            Ok(message) => message,
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        };

        let message = match WxMessage::try_from(raw) {
            Ok(message) => message,
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        };

        tracing::info!(?message, "Received a unencrypted message from weixin.");
        message
    };

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
            let event_key: usize = match if let Some(stripped) =
                event_key.strip_prefix(EVENT_KEY_PREFIX)
            {
                stripped.parse()
            } else {
                event_key.parse()
            } {
                Ok(event_key) => event_key,
                Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
            };
            tracing::info!(?event, %event_key, %ticket, "Received event");

            return match handle_scan(
                &message.from_user_name,
                &message.to_user_name,
                event_key,
                connection,
                session_manager,
                wx_app.config(),
            )
            .await
            {
                Ok(Some(xml)) => (StatusCode::OK, xml).into_response(),
                Ok(None) => StatusCode::OK.into_response(),
                Err(error) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                }
            };
        }
    }

    StatusCode::OK.into_response()
}

async fn handle_scan(
    from_user: &str,
    to_user: &str,
    websocket_id: usize,
    connection: DatabaseConnection,
    session_manager: SessionManager,
    wx_config: &WxConfig,
) -> anyhow::Result<Option<Xml<WxRawXmlMessage>>> {
    use crate::storage::model::user::*;
    if let Some(_user) = Entity::find()
        .filter(Column::OpenId.eq(from_user))
        .one(&connection)
        .await?
    {
        // TODO login
        return Ok(None);
    }

    // register
    let register = ActiveModel {
        open_id: Set(from_user.to_string()),
        ..Default::default()
    };
    let _inserted = Entity::insert(register).exec(&connection).await?;
    // TODO save openid -> connection id to map
    // OPENID_EVENT_CODE_MAP.put(fromUser, eventKey);
    //授权流程,给用户发送授权消息，并且异步通知前端扫码成功
    tokio::spawn(async move {
        let resp = Resp {
            r#type: RespType::LoginScanSuccess,
            data: (),
        };
        if let Err(error) = session_manager.try_send(websocket_id, &resp).await {
            tracing::error!(%error, %websocket_id, ?resp, "Failed to send response to websocket");
        }
    });
    let callback_url = format!("{}/wx/portal/public/callBack", wx_config.app_id); // TODO use url
    let encoded_callback_url = urlencoding::encode(&callback_url);
    let skip_url = format!("https://open.weixin.qq.com/connect/oauth2/authorize?appid={}&redirect_uri={}&response_type=code&scope=snsapi_userinfo&state=STATE#wechat_redirect", wx_config.app_id, encoded_callback_url);
    let message = WxMessage {
        to_user_name: from_user.to_string(),
        from_user_name: to_user.to_string(),
        create_time: (current_millisecond() / 1000) as i32,
        data: WxMessageData::Text {
            content: format!("请点击链接授权：<a href=\"{skip_url}\">登录</a>"),
        },
        msg_id: None,
        msg_data_id: None,
        idx: None,
    };
    Ok(Some(Xml(message.into())))
}
