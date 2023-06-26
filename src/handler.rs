//! # HTTP 请求处理器

use crate::handler::auth::JwtKeys;
use crate::handler::ws::SessionManager;
use crate::weixin::WxClient;
use axum::http::Request;
use axum::routing::get;
use axum::{Extension, Router};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, OnRequest, TraceLayer};
use tower_http::LatencyUnit;
use tracing::{Level, Span};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod api;
pub mod auth;
pub mod chat;
pub mod user;
pub mod wechat;
pub mod ws;

/// HTTP 服务器配置
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpConfig {
    /// 静态文件目录
    pub static_files_path: PathBuf,
    /// HTTP 监听端口
    pub port: u16,
    /// JWT 签名密钥，base64 格式
    pub jwt_secret: String,
}

/// Open API Documentation
#[derive(OpenApi)]
#[openapi(
    info(description = "MallChat APIs"),
    paths(
        chat::get_room_page,
        chat::get_member_page,
        chat::get_member_statistic,
        chat::get_msg_page,
        chat::send_message,
        user::get_user_info,
        user::modify_name,
        user::badges,
        user::wearing_badge,
        // wechat::auth_get,
        // wechat::call_back,
        // wechat::wx_post,
    )
)]
pub struct ApiDoc;

/// 所有路由
pub fn router<P: AsRef<std::path::Path>>(
    with_swagger: bool,
    static_files_path: P,
    storage: DatabaseConnection,
    // cache: redis::Client,
    key: JwtKeys,
    wx_client: WxClient,
) -> Router {
    let router = Router::new()
        .nest_service("/", ServeDir::new(static_files_path))
        .route("/websocket", get(ws::websocket_on_connect))
        .merge(chat::route())
        .merge(user::route())
        .merge(wechat::route())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(RequestTracer::from(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .layer(Extension(storage))
        // .layer(Extension(cache))
        .layer(Extension(key))
        .layer(Extension(wx_client))
        .layer(Extension(SessionManager::default()));
    if with_swagger {
        router.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
    } else {
        router
    }
}

/// 跟踪 HTTP 请求的方法、URI 和 HTTP 版本
#[derive(Debug, Clone, Copy)]
pub struct RequestTracer {
    level: Level,
}

impl From<Level> for RequestTracer {
    fn from(level: Level) -> Self {
        Self { level }
    }
}

impl<B> OnRequest<B> for RequestTracer {
    fn on_request(&mut self, request: &Request<B>, _: &Span) {
        let method = request.method();
        let uri = request.uri();
        let version = request.version();
        match self.level {
            Level::ERROR => {
                tracing::event!(Level::ERROR, %method, %uri, ?version, "started processing request");
            }
            Level::WARN => {
                tracing::event!(Level::WARN, %method, %uri, ?version, "started processing request");
            }
            Level::INFO => {
                tracing::event!(Level::INFO, %method, %uri, ?version, "started processing request");
            }
            Level::DEBUG => {
                tracing::event!(Level::DEBUG, %method, %uri, ?version, "started processing request");
            }
            Level::TRACE => {
                tracing::event!(Level::TRACE, %method, %uri, ?version, "started processing request");
            }
        }
    }
}
