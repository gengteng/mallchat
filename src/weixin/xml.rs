//! XML extractor

use axum::body::{Full, HttpBody};
use axum::extract::rejection::BytesRejection;
use axum::extract::FromRequest;
use axum::headers::{HeaderMap, HeaderValue};
use axum::http::{header, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{body, BoxError};
use bytes::{BufMut, Bytes};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// XML extractor
#[derive(Debug, Clone, Copy, Default)]
pub struct Xml<T>(pub T);

#[axum::async_trait]
impl<T, S, B> FromRequest<S, B> for Xml<T>
where
    T: DeserializeOwned,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = XmlRejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        if xml_content_type(req.headers()) {
            let bytes = Bytes::from_request(req, state).await?;
            let value = serde_xml_rs::from_reader(&*bytes)?;
            Ok(Xml(value))
        } else {
            Err(XmlRejection::MissingXMLContentType)
        }
    }
}

fn xml_content_type(headers: &HeaderMap) -> bool {
    let content_type = if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    let mime = if let Ok(mime) = content_type.parse::<mime::Mime>() {
        mime
    } else {
        return false;
    };

    let is_xml_content_type = mime.type_() == "application"
        && (mime.subtype() == "xml" || mime.suffix().map_or(false, |name| name == "xml"));

    is_xml_content_type
}

impl<T> std::ops::Deref for Xml<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Xml<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<T> for Xml<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for Xml<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        // Use a small initial capacity of 128 bytes like serde_json::to_vec
        // https://docs.rs/serde_json/1.0.82/src/serde_json/ser.rs.html#2189
        let mut buf = bytes::BytesMut::with_capacity(128).writer();
        match serde_xml_rs::to_writer(&mut buf, &self.0) {
            Ok(()) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/xml"),
                )],
                buf.into_inner().freeze(),
            )
                .into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                )],
                err.to_string(),
            )
                .into_response(),
        }
    }
}

/// Xml extractor rejection
#[derive(Debug, thiserror::Error)]
pub enum XmlRejection {
    /// Failed to parse the request body as XML
    #[error("Failed to parse the request body as XML")]
    InvalidXMLBody(#[from] serde_xml_rs::Error),
    /// Expected request with `Content-Type: application/xml`
    #[error("Expected request with `Content-Type: application/xml`")]
    MissingXMLContentType,
    /// bytes rejection
    #[error("{0}")]
    BytesRejection(#[from] BytesRejection),
}

impl IntoResponse for XmlRejection {
    fn into_response(self) -> Response {
        match self {
            e @ XmlRejection::InvalidXMLBody(_) => {
                let mut res = Response::new(body::boxed(Full::from(format!("{}", e))));
                *res.status_mut() = StatusCode::UNPROCESSABLE_ENTITY;
                res
            }
            e @ XmlRejection::MissingXMLContentType => {
                let mut res = Response::new(body::boxed(Full::from(format!("{}", e))));
                *res.status_mut() = StatusCode::UNSUPPORTED_MEDIA_TYPE;
                res
            }
            XmlRejection::BytesRejection(e) => e.into_response(),
        }
    }
}
