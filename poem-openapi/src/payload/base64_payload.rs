use std::ops::{Deref, DerefMut};

use base64::engine::{Engine, general_purpose::STANDARD};
use bytes::Bytes;
use futures_util::TryFutureExt;
use poem::{IntoResponse, Request, RequestBody, Response, Result};

use crate::{
    ApiResponse,
    error::ParseRequestPayloadError,
    payload::{ParsePayload, Payload},
    registry::{MetaMediaType, MetaResponse, MetaResponses, MetaSchema, MetaSchemaRef, Registry},
};

/// A binary payload encoded with `base64`.
///
/// # Examples
///
/// ```rust
/// use poem::{
///     Body, IntoEndpoint, Request, Result,
///     error::BadRequest,
///     http::{Method, StatusCode, Uri},
///     test::TestClient,
/// };
/// use poem_openapi::{
///     OpenApi, OpenApiService,
///     payload::{Base64, Json},
/// };
/// use tokio::{io::AsyncReadExt, sync::Mutex};
///
/// #[derive(Default)]
/// struct MyApi {
///     data: Mutex<Vec<u8>>,
/// }
///
/// #[OpenApi]
/// impl MyApi {
///     #[oai(path = "/upload", method = "post")]
///     async fn upload_binary(&self, data: Base64<Vec<u8>>) -> Json<usize> {
///         let len = data.len();
///         assert_eq!(data.0, b"abcdef");
///         *self.data.lock().await = data.0;
///         Json(len)
///     }
///
///     #[oai(path = "/download", method = "get")]
///     async fn download_binary(&self) -> Base64<Vec<u8>> {
///         Base64(self.data.lock().await.clone())
///     }
/// }
///
/// let api = OpenApiService::new(MyApi::default(), "Demo", "0.1.0");
/// let cli = TestClient::new(api);
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let resp = cli
///     .post("/upload")
///     .content_type("text/plain")
///     .body("YWJjZGVm")
///     .send()
///     .await;
/// resp.assert_status_is_ok();
/// resp.assert_text("6").await;
///
/// let resp = cli.get("/download").send().await;
/// resp.assert_status_is_ok();
/// resp.assert_text("YWJjZGVm").await;
/// # });
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Base64<T>(pub T);

impl<T> Deref for Base64<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Base64<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Send> Payload for Base64<T> {
    const CONTENT_TYPE: &'static str = "text/plain; charset=utf-8";

    fn check_content_type(content_type: &str) -> bool {
        matches!(content_type.parse::<mime::Mime>(), Ok(content_type) if content_type.type_() == "text"
                && (content_type.subtype() == "plain"
                || content_type
                    .suffix()
                    .is_some_and(|v| v == "plain")))
    }

    fn schema_ref() -> MetaSchemaRef {
        MetaSchemaRef::Inline(Box::new(MetaSchema {
            format: Some("string"),
            ..MetaSchema::new("bytes")
        }))
    }
}

async fn read_base64(body: &mut RequestBody) -> Result<Vec<u8>> {
    let body = async move { body.take() }
        .and_then(|body| body.into_vec())
        .await
        .map_err(|err| ParseRequestPayloadError {
            reason: err.to_string(),
        })?;
    let data = STANDARD
        .decode(body)
        .map_err(|err| ParseRequestPayloadError {
            reason: err.to_string(),
        })?;
    Ok(data)
}

impl ParsePayload for Base64<Vec<u8>> {
    const IS_REQUIRED: bool = true;

    async fn from_request(_request: &Request, body: &mut RequestBody) -> Result<Self> {
        read_base64(body).await.map(Self)
    }
}

impl ParsePayload for Base64<Bytes> {
    const IS_REQUIRED: bool = true;

    async fn from_request(_request: &Request, body: &mut RequestBody) -> Result<Self> {
        read_base64(body).await.map(|data| Self(data.into()))
    }
}

impl<T: AsRef<[u8]> + Send> IntoResponse for Base64<T> {
    fn into_response(self) -> Response {
        Response::builder()
            .content_type(Self::CONTENT_TYPE)
            .body(STANDARD.encode(self.0.as_ref()))
    }
}

impl<T: AsRef<[u8]> + Send> ApiResponse for Base64<T> {
    fn meta() -> MetaResponses {
        MetaResponses {
            responses: vec![MetaResponse {
                description: "",
                status: Some(200),
                status_range: None,
                content: vec![MetaMediaType {
                    content_type: Self::CONTENT_TYPE,
                    schema: Self::schema_ref(),
                }],
                headers: vec![],
            }],
        }
    }

    fn register(_registry: &mut Registry) {}
}

impl_apirequest_for_payload!(Base64<Vec<u8>>);
impl_apirequest_for_payload!(Base64<Bytes>);
