//! # 用户管理相关接口
//!

use axum::routing::{get, put};
use axum::Router;

use crate::handler::api::{ApiResult, ApiValue};
use crate::handler::auth::Claims;

/// 用户管理相关路由
pub fn route() -> Router {
    Router::new().nest(
        "/capi/user",
        Router::new()
            .route("/userInfo", get(get_user_info))
            .route("/name", put(modify_name))
            .route("/badges", get(badges))
            .route("/badge", put(wearing_badge)),
    )
}

/// 用户详情
#[utoipa::path(get, path = "/capi/user/userInfo")]
pub async fn get_user_info(_claims: Claims) -> ApiResult<()> {
    tracing::info!("get_user_info");
    ApiValue::success()
}

/// 修改用户名
#[utoipa::path(put, path = "/capi/user/name")]
pub async fn modify_name() -> ApiResult<()> {
    ApiValue::success()
}

/// 可选徽章预览
#[utoipa::path(get, path = "/capi/user/badges")]
pub async fn badges() -> ApiResult<()> {
    ApiValue::success()
}

/// 佩戴徽章
#[utoipa::path(put, path = "/capi/user/badge")]
pub async fn wearing_badge() -> ApiResult<()> {
    ApiValue::success()
}
