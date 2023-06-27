//! # API 公共

use axum::http::StatusCode;
use std::borrow::Cow;
use std::mem::size_of;

use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use utoipa::IntoParams;
use validator::Validate;

/// Api 错误的结果
pub type Result<T> = std::result::Result<T, ApiError>;

/// API 结果，作为 JSON BODY 返回
pub type ApiResult<T> = Result<ApiValue<T>>;

/// API 错误
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 数据库错误
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    /// Redis 错误
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    /// JWT 错误
    #[error("JWT error: {0}")]
    JWT(#[from] jsonwebtoken::errors::Error),
    /// UTF-8
    #[error("UTF8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    /// 自定义错误
    #[error("Custom error ({0}) : {1}")]
    Custom(StatusCode, Cow<'static, str>),
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        Self::Custom(StatusCode::INTERNAL_SERVER_ERROR, value.to_string().into())
    }
}

impl ApiError {
    /// 构造一个自定义错误
    pub fn custom(status: StatusCode, message: impl Into<Cow<'static, str>>) -> Self {
        Self::Custom(status, message.into())
    }
    /// 构造一个自定义错误结果
    pub fn custom_err<T>(
        status: StatusCode,
        message: impl Into<Cow<'static, str>>,
    ) -> ApiResult<T> {
        Err(Self::custom(status, message))
    }
    /// 使用当前错误构造一个 `ApiResult<T>`
    pub fn to_api_err<T>(self) -> ApiResult<T> {
        Err(self)
    }
    /// 错误码
    pub fn err_code(&self) -> i32 {
        0
    }
    /// 错误消息
    pub fn err_msg(&self) -> String {
        format!("{}", self)
    }
    /// 错误码
    pub fn http_status_code(&self) -> StatusCode {
        if let Self::Custom(status, _) = self {
            return *status;
        }

        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("success", &false)?;
        map.serialize_entry("errCode", &self.err_code())?;
        map.serialize_entry("errMsg", &self.err_msg())?;
        map.end()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.http_status_code(), Json(self)).into_response()
    }
}

/// 基础分页器
#[derive(Debug, Validate, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct Pager {
    /// 页大小
    #[validate(range(min = 1, max = 100))]
    pub page_size: usize,
    /// 页码
    #[validate(range(min = 1))]
    pub page_no: usize,
}

impl Default for Pager {
    fn default() -> Self {
        Self {
            page_size: 50,
            page_no: 1,
        }
    }
}

/// API 结果
#[derive(Debug)]
pub struct ApiValue<T>(T);

impl ApiValue<()> {
    /// 仅返回成功
    pub fn success() -> ApiResult<()> {
        Ok(ApiValue(()))
    }
}

impl<T> ApiValue<Option<T>> {
    /// 可能为空的数据
    pub fn nullable(data: impl Into<Option<T>>) -> ApiResult<Option<T>> {
        Ok(ApiValue(data.into()))
    }
}

impl<T> ApiValue<T> {
    /// 返回成功数据
    pub fn data(data: T) -> ApiResult<T> {
        Ok(ApiValue(data))
    }
}

/// 转换为 ApiResult
pub trait ToApiData<T> {
    /// 将当前对象转换为 ApiResult
    fn to_api_data(self) -> ApiResult<T>;
}

impl<T> ToApiData<T> for T {
    fn to_api_data(self) -> ApiResult<T> {
        Ok(ApiValue(self))
    }
}

impl<T: Serialize> IntoResponse for ApiValue<T> {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

impl<T: Serialize> Serialize for ApiValue<T> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("success", &true)?;
        if size_of::<T>() != 0 {
            map.serialize_entry("data", &self.0)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde::Serialize;

    use crate::handler::api::{ApiError, ApiValue};

    #[test]
    fn api_result_serialize() -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Data {
            a: i32,
            b: String,
        }

        println!(
            "{}",
            serde_json::to_string(&ApiValue::data(Data {
                a: 1,
                b: String::new(),
            })?)?
        );
        println!(
            "{}",
            serde_json::to_string(
                &ApiError::custom_err::<()>(StatusCode::BAD_REQUEST, "test error")
                    .expect_err("get error")
            )?
        );

        println!("{}", serde_json::to_string(&ApiValue::success()?)?);
        println!("{}", serde_json::to_string(&ApiValue::nullable(12)?)?);
        println!(
            "{}",
            serde_json::to_string(&ApiValue::<Option<i32>>::nullable(None)?)?
        );

        Ok(())
    }
}
