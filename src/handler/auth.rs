//! # 登录授权相关
//!

use crate::handler::api::ApiError;
use axum::extract::FromRequestParts;
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::{async_trait, Extension, RequestPartsExt, TypedHeader};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// JWT 使用的加解密 KEY
#[derive(Clone)]
pub struct JwtKeys {
    keys: Arc<(EncodingKey, DecodingKey)>,
}

impl TryFrom<&str> for JwtKeys {
    type Error = jsonwebtoken::errors::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self {
            keys: Arc::new((
                EncodingKey::from_base64_secret(value)?,
                DecodingKey::from_base64_secret(value)?,
            )),
        })
    }
}

impl JwtKeys {
    /// 加密 Key
    pub fn encoding_key(&self) -> &EncodingKey {
        &self.keys.0
    }
    /// 解密 Key
    pub fn decoding_key(&self) -> &DecodingKey {
        &self.keys.1
    }
    /// 使用默认算法 HMAC using SHA-256 签名获得 JWT
    pub fn sign(&self, claims: &Claims) -> Result<String, ApiError> {
        Ok(jsonwebtoken::encode(
            &Header::default(),
            claims,
            self.encoding_key(),
        )?)
    }
    /// 验证并获取 Claims
    pub fn verify(&self, token: &str) -> Result<Claims, ApiError> {
        // 不对exp字段、过期时间做校验？？？
        // MallChat 为什么不使用标准的 Claims
        let mut validation = Validation::default();
        validation.required_spec_claims = HashSet::new();
        validation.validate_exp = false;
        Ok(jsonwebtoken::decode(token, self.decoding_key(), &validation)?.claims)
    }
}

/// 存储到 JWT 中的数据
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Claims {
    /// 用户 ID
    pub uid: i64,
    /// 创建时间
    pub create_time: i64,
}

impl From<i64> for Claims {
    fn from(uid: i64) -> Self {
        Self {
            uid,
            create_time: current_millisecond(),
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let Extension(jwt_keys): Extension<JwtKeys> =
            parts.extract_with_state(state).await.map_err(|_| {
                ApiError::custom(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "JWT keys not correctly initialized",
                )
            })?;
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| ApiError::custom(StatusCode::UNAUTHORIZED, "Invalid token"))?;
        jwt_keys
            .verify(bearer.token())
            .map_err(|_| ApiError::custom(StatusCode::UNAUTHORIZED, "Invalid token"))
    }
}

/// 获取当前时间戳（毫秒）
pub fn current_millisecond() -> i64 {
    use std::time::SystemTime;
    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Clock may have gone backwards.");
    duration.as_millis() as i64
}

#[cfg(test)]
mod tests {
    use crate::handler::auth::{current_millisecond, Claims, JwtKeys};

    #[test]
    fn jwt() -> anyhow::Result<()> {
        let keys = JwtKeys::try_from("omOFP+Ejj/r+u4XeHr+KImZNtP0AlNqgvjLe3C5qics=")?;
        let claims = Claims {
            uid: 12,
            create_time: current_millisecond(),
        };
        let token = keys.sign(&claims)?;
        let claims_verified = keys.verify(&token)?;
        assert_eq!(claims, claims_verified);
        Ok(())
    }
}
