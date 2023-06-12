//! # 聊天相关
//!

use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use axum_valid::Valid;
use redis::AsyncCommands;
use sea_orm::{ConnectionTrait, DatabaseConnection};

use crate::handler::api::{ApiError, ApiResult, ApiValue, Pager, ToApiData};
use crate::handler::auth::Claims;

/// 聊天相关路由
pub fn route() -> Router {
    Router::new().nest(
        "/capi/chat",
        Router::new()
            .route("/public/room/page", get(get_room_page))
            .route("/public/member/page", get(get_member_page))
            .route("/public/member/statistic", get(get_member_statistic))
            .route("/public/msg/page", get(get_msg_page))
            .route("/msg", post(send_message))
            .route("/msg/mark", put(send_message_mark)),
    )
}

/// 会话列表
#[utoipa::path(get, path = "/capi/chat/public/room/page", params(Pager))]
pub async fn get_room_page(
    Valid(Query(pager)): Valid<Query<Pager>>, // TODO: 这里使用了 Valid 就会导致 swagger 前端不生效
    Extension(cache): Extension<redis::Client>,
) -> ApiResult<Option<String>> {
    tracing::info!(?pager, "get_room_page");
    let mut connection = cache.get_async_connection().await?;
    let value: Option<String> = connection.get("some-key").await?;
    value.to_api_data()
}

/// 群成员列表
#[utoipa::path(get, path = "/capi/chat/public/member/page")]
pub async fn get_member_page(Extension(db): Extension<DatabaseConnection>) -> ApiResult<u64> {
    let r = db.execute_unprepared("select 1").await?;
    r.rows_affected().to_api_data()
}

/// 群成员人数统计
#[utoipa::path(get, path = "/capi/chat/public/member/statistic")]
pub async fn get_member_statistic(_claims: Claims) -> ApiResult<()> {
    ApiValue::success()
}

/// 消息列表
#[utoipa::path(get, path = "/capi/chat/public/msg/page")]
pub async fn get_msg_page() -> ApiResult<()> {
    ApiValue::success()
}

/// 发送消息
#[utoipa::path(post, path = "/capi/chat/msg", request_body = Pager)]
pub async fn send_message(Valid(Json(pager)): Valid<Json<Pager>>) -> ApiResult<()> {
    tracing::info!(?pager, "send_message");
    ApiError::custom_err(StatusCode::NOT_IMPLEMENTED, "TODO")
}

/// 消息标记
#[utoipa::path(put, path = "/capi/chat/msg/mark")]
pub async fn send_message_mark() -> ApiResult<()> {
    ApiValue::success()
}
