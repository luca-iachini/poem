//! Some common error types.

use std::{
    convert::Infallible,
    error::Error as StdError,
    fmt::{self, Debug, Display, Formatter},
    string::FromUtf8Error,
};

use http::Method;

use crate::{http::StatusCode, IntoResponse, Response};

macro_rules! define_http_error {
    ($($(#[$docs:meta])* ($name:ident, $status:ident);)*) => {
        $(
        $(#[$docs])*
        #[allow(non_snake_case)]
        #[inline]
        pub fn $name(err: impl StdError + Send + Sync + 'static) -> Error {
            Error::new(err, StatusCode::$status)
        }
        )*
    };
}

/// Represents a type that can be converted to [`Error`].
pub trait ResponseError {
    /// The status code of this error.
    fn status(&self) -> StatusCode;
}

enum ErrorSource {
    BoxedError(Box<dyn StdError + Send + Sync>),
    #[cfg(feature = "anyhow")]
    Anyhow(anyhow::Error),
}

impl Debug for ErrorSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSource::BoxedError(err) => Debug::fmt(err, f),
            #[cfg(feature = "anyhow")]
            ErrorSource::Anyhow(err) => Debug::fmt(err, f),
        }
    }
}

/// General response error.
///
/// # Create from any error types
///
/// ```
/// use poem::{error::InternalServerError, handler, Result};
///
/// #[handler]
/// async fn index() -> Result<String> {
///     Ok(std::fs::read_to_string("example.txt").map_err(InternalServerError)?)
/// }
/// ```
///
/// # Create you own error type
///
/// ```
/// use poem::{error::ResponseError, handler, http::StatusCode, Result};
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("my error")]
/// struct MyError;
///
/// impl ResponseError for MyError {
///     fn status(&self) -> StatusCode {
///         StatusCode::BAD_GATEWAY
///     }
/// }
///
/// fn do_something() -> Result<(), MyError> {
///     todo!()
/// }
///
/// #[handler]
/// async fn index() -> Result<()> {
///     Ok(do_something()?)
/// }
/// ```
///
/// # Downcast the error to concrete error type
///
/// ```
/// use poem::{error::NotFoundError, Error};
///
/// let err: Error = NotFoundError.into();
///
/// assert!(err.is::<NotFoundError>());
/// assert_eq!(err.downcast_ref::<NotFoundError>(), Some(&NotFoundError));
/// ```
#[derive(Debug)]
pub struct Error {
    status: StatusCode,
    source: ErrorSource,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.source {
            ErrorSource::BoxedError(err) => Display::fmt(err, f),
            #[cfg(feature = "anyhow")]
            ErrorSource::Anyhow(err) => Display::fmt(err, f),
        }
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl<T: ResponseError + StdError + Send + Sync + 'static> From<T> for Error {
    fn from(err: T) -> Self {
        let status = err.status();
        Error::new(err, status)
    }
}

impl From<Box<dyn StdError + Send + Sync>> for Error {
    fn from(err: Box<dyn StdError + Send + Sync>) -> Self {
        Error {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            source: ErrorSource::BoxedError(err),
        }
    }
}

impl From<(StatusCode, Box<dyn StdError + Send + Sync>)> for Error {
    fn from((status, err): (StatusCode, Box<dyn StdError + Send + Sync>)) -> Self {
        Error {
            status,
            source: ErrorSource::BoxedError(err),
        }
    }
}

#[cfg(feature = "anyhow")]
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            source: ErrorSource::Anyhow(err),
        }
    }
}

#[cfg(feature = "anyhow")]
impl From<(StatusCode, anyhow::Error)> for Error {
    fn from((status, err): (StatusCode, anyhow::Error)) -> Self {
        Error {
            status,
            source: ErrorSource::Anyhow(err),
        }
    }
}

impl From<StatusCode> for Error {
    fn from(status: StatusCode) -> Self {
        Error::from_status(status)
    }
}

impl Error {
    /// Create a new error object from any error type with `503
    /// INTERNAL_SERVER_ERROR` status code.
    #[inline]
    pub fn new<T: StdError + Send + Sync + 'static>(err: T, status: StatusCode) -> Self {
        Self {
            status,
            source: ErrorSource::BoxedError(Box::new(err)),
        }
    }

    /// create a new error object from status code.
    pub fn from_status(status: StatusCode) -> Self {
        #[derive(Debug, thiserror::Error)]
        #[error("{0}")]
        struct StatusError(StatusCode);

        impl ResponseError for StatusError {
            fn status(&self) -> StatusCode {
                self.0
            }
        }

        StatusError(status).into()
    }

    /// Create a new error object from string with `503 INTERNAL_SERVER_ERROR`
    /// status code.
    pub fn from_string(msg: impl Into<String>, status: StatusCode) -> Self {
        #[derive(Debug, thiserror::Error)]
        #[error("{0}")]
        struct StringError(String);

        Self::new(StringError(msg.into()), status)
    }

    /// Returns the status code of this error.
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Downcast this error object by reference.
    #[inline]
    pub fn downcast_ref<T: StdError + Send + Sync + 'static>(&self) -> Option<&T> {
        match &self.source {
            ErrorSource::BoxedError(err) => err.downcast_ref::<T>(),
            #[cfg(feature = "anyhow")]
            ErrorSource::Anyhow(err) => err.downcast_ref::<T>(),
        }
    }

    /// Attempts to downcast the error to a concrete error type.
    #[inline]
    pub fn downcast<T: StdError + Send + Sync + 'static>(self) -> Result<T, Error> {
        let status = self.status;

        match self.source {
            ErrorSource::BoxedError(err) => match err.downcast::<T>() {
                Ok(err) => Ok(*err),
                Err(err) => Err(Error {
                    status,
                    source: ErrorSource::BoxedError(err),
                }),
            },
            #[cfg(feature = "anyhow")]
            ErrorSource::Anyhow(err) => match err.downcast::<T>() {
                Ok(err) => Ok(err),
                Err(err) => Err(Error {
                    status,
                    source: ErrorSource::Anyhow(err),
                }),
            },
        }
    }

    /// Returns `true` if the error type is the same as `T`.
    #[inline]
    pub fn is<T: StdError + Debug + Send + Sync + 'static>(&self) -> bool {
        match &self.source {
            ErrorSource::BoxedError(err) => err.is::<T>(),
            #[cfg(feature = "anyhow")]
            ErrorSource::Anyhow(err) => err.is::<T>(),
        }
    }

    /// Consumes this to return a response object.
    pub fn as_response(&self) -> Response {
        let msg = match &self.source {
            ErrorSource::BoxedError(err) => err.to_string(),
            #[cfg(feature = "anyhow")]
            ErrorSource::Anyhow(err) => err.to_string(),
        };
        Response::builder().status(self.status).body(msg)
    }
}

define_http_error!(
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::BAD_REQUEST`].
    (BadRequest, BAD_REQUEST);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::UNAUTHORIZED`].
    (Unauthorized, UNAUTHORIZED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::PAYMENT_REQUIRED`].
    (PaymentRequired, PAYMENT_REQUIRED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::FORBIDDEN`].
    (Forbidden, FORBIDDEN);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::NOT_FOUND`].
    (NotFound, NOT_FOUND);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::METHOD_NOT_ALLOWED`].
    (MethodNotAllowed, METHOD_NOT_ALLOWED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::NOT_ACCEPTABLE`].
    (NotAcceptable, NOT_ACCEPTABLE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::PROXY_AUTHENTICATION_REQUIRED`].
    (ProxyAuthenticationRequired, PROXY_AUTHENTICATION_REQUIRED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::REQUEST_TIMEOUT`].
    (RequestTimeout, REQUEST_TIMEOUT);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::CONFLICT`].
    (Conflict, CONFLICT);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::GONE`].
    (Gone, GONE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::LENGTH_REQUIRED`].
    (LengthRequired, LENGTH_REQUIRED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::PAYLOAD_TOO_LARGE`].
    (PayloadTooLarge, PAYLOAD_TOO_LARGE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::URI_TOO_LONG`].
    (UriTooLong, URI_TOO_LONG);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::UNSUPPORTED_MEDIA_TYPE`].
    (UnsupportedMediaType, UNSUPPORTED_MEDIA_TYPE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::RANGE_NOT_SATISFIABLE`].
    (RangeNotSatisfiable, RANGE_NOT_SATISFIABLE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::IM_A_TEAPOT`].
    (ImATeapot, IM_A_TEAPOT);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::MISDIRECTED_REQUEST`].
    (MisdirectedRequest, MISDIRECTED_REQUEST);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::UNPROCESSABLE_ENTITY`].
    (UnprocessableEntity, UNPROCESSABLE_ENTITY);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::LOCKED`].
    (Locked, LOCKED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::FAILED_DEPENDENCY`].
    (FailedDependency, FAILED_DEPENDENCY);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::UPGRADE_REQUIRED`].
    (UpgradeRequired, UPGRADE_REQUIRED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::PRECONDITION_FAILED`].
    (PreconditionFailed, PRECONDITION_FAILED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::PRECONDITION_REQUIRED`].
    (PreconditionRequired, PRECONDITION_REQUIRED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::TOO_MANY_REQUESTS`].
    (TooManyRequests, TOO_MANY_REQUESTS);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE`].
    (RequestHeaderFieldsTooLarge, REQUEST_HEADER_FIELDS_TOO_LARGE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS`].
    (UnavailableForLegalReasons, UNAVAILABLE_FOR_LEGAL_REASONS);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::EXPECTATION_FAILED`].
    (ExpectationFailed, EXPECTATION_FAILED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::INTERNAL_SERVER_ERROR`].
    (InternalServerError, INTERNAL_SERVER_ERROR);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::NOT_IMPLEMENTED`].
    (NotImplemented, NOT_IMPLEMENTED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::BAD_GATEWAY`].
    (BadGateway, BAD_GATEWAY);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::SERVICE_UNAVAILABLE`].
    (ServiceUnavailable, SERVICE_UNAVAILABLE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::GATEWAY_TIMEOUT`].
    (GatewayTimeout, GATEWAY_TIMEOUT);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::HTTP_VERSION_NOT_SUPPORTED`].
    (HttpVersionNotSupported, HTTP_VERSION_NOT_SUPPORTED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::VARIANT_ALSO_NEGOTIATES`].
    (VariantAlsoNegotiates, VARIANT_ALSO_NEGOTIATES);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::INSUFFICIENT_STORAGE`].
    (InsufficientStorage, INSUFFICIENT_STORAGE);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::LOOP_DETECTED`].
    (LoopDetected, LOOP_DETECTED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::NOT_EXTENDED`].
    (NotExtended, NOT_EXTENDED);
    /// Wraps any error into [`Error`] and the status code is [`StatusCode::NETWORK_AUTHENTICATION_REQUIRED`].
    (NetworkAuthenticationRequired, NETWORK_AUTHENTICATION_REQUIRED);
);

/// A specialized Result type for Poem.
pub type Result<T, E = Error> = ::std::result::Result<T, E>;

/// Represents a type that can be converted to `poem::Result<T>`.
///
/// # Example
///
/// ```
/// use poem::error::{IntoResult, NotFoundError};
///
/// let res = "abc".into_result();
/// assert!(matches!(res, Ok("abc")));
///
/// let res = Err::<(), _>(NotFoundError).into_result();
/// assert!(res.is_err());
/// let err = res.unwrap_err();
/// assert!(err.is::<NotFoundError>());
/// ```
pub trait IntoResult<T: IntoResponse> {
    /// Consumes this value returns a `poem::Result<T>`.
    fn into_result(self) -> Result<T>;
}

impl<T, E> IntoResult<T> for Result<T, E>
where
    T: IntoResponse,
    E: Into<Error> + Debug + Send + Sync + 'static,
{
    #[inline]
    fn into_result(self) -> Result<T> {
        self.map_err(Into::into)
    }
}

impl<T: IntoResponse> IntoResult<T> for T {
    #[inline]
    fn into_result(self) -> Result<T> {
        Ok(self)
    }
}

macro_rules! define_simple_errors {
    ($($(#[$docs:meta])* ($name:ident, $status:ident, $err_msg:literal);)*) => {
        $(
        $(#[$docs])*
        #[derive(Debug, thiserror::Error, Copy, Clone, Eq, PartialEq)]
        #[error($err_msg)]
        pub struct $name;

        impl ResponseError for $name {
            fn status(&self) -> StatusCode {
                StatusCode::$status
            }
        }
        )*
    };
}

define_simple_errors!(
    /// Only the endpoints under the router can get the path parameters, otherwise this error will occur.
    (ParsePathError, BAD_REQUEST, "invalid path params");

    /// Error occurred in the router.
    (NotFoundError, NOT_FOUND, "not found");

    /// Error occurred in the router.
    (MethodNotAllowedError, METHOD_NOT_ALLOWED, "method not allowed");

    /// Error occurred in the `Cors` middleware.
    (CorsError, UNAUTHORIZED, "unauthorized");
);

/// A possible error value when reading the body.
#[derive(Debug, thiserror::Error)]
pub enum ReadBodyError {
    /// Body has been taken by other extractors.
    #[error("the body has been taken")]
    BodyHasBeenTaken,

    /// Body is not a valid utf8 string.
    #[error("parse utf8: {0}")]
    Utf8(#[from] FromUtf8Error),

    /// Io error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl ResponseError for ReadBodyError {
    fn status(&self) -> StatusCode {
        match self {
            ReadBodyError::BodyHasBeenTaken => StatusCode::INTERNAL_SERVER_ERROR,
            ReadBodyError::Utf8(_) => StatusCode::BAD_REQUEST,
            ReadBodyError::Io(_) => StatusCode::BAD_REQUEST,
        }
    }
}

/// A possible error value when parsing cookie.
#[cfg(feature = "cookie")]
#[cfg_attr(docsrs, doc(cfg(feature = "cookie")))]
#[derive(Debug, thiserror::Error)]
pub enum ParseCookieError {
    /// Cookie value is illegal.
    #[error("cookie is illegal")]
    CookieIllegal,

    /// A `Cookie` header is required.
    #[error("`Cookie` header is required")]
    CookieHeaderRequired,

    /// Cookie value is illegal.
    #[error("cookie is illegal: {0}")]
    ParseJsonValue(#[from] serde_json::Error),
}

#[cfg(feature = "cookie")]
impl ResponseError for ParseCookieError {
    fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

/// A possible error value when extracts data from request fails.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
#[error("data of type `{0}` was not found.")]
pub struct GetDataError(pub &'static str);

impl ResponseError for GetDataError {
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

/// A possible error value when parsing form.
#[derive(Debug, thiserror::Error)]
pub enum ParseFormError {
    /// Invalid content type.
    #[error("invalid content type `{0}`, expect: `application/x-www-form-urlencoded`")]
    InvalidContentType(String),

    /// `Content-Type` header is required.
    #[error("expect content type `application/x-www-form-urlencoded`")]
    ContentTypeRequired,

    /// Url decode error.
    #[error("url decode: {0}")]
    UrlDecode(#[from] serde_urlencoded::de::Error),
}

impl ResponseError for ParseFormError {
    fn status(&self) -> StatusCode {
        match self {
            ParseFormError::InvalidContentType(_) => StatusCode::BAD_REQUEST,
            ParseFormError::ContentTypeRequired => StatusCode::BAD_REQUEST,
            ParseFormError::UrlDecode(_) => StatusCode::BAD_REQUEST,
        }
    }
}

/// A possible error value when parsing JSON.
#[derive(Debug, thiserror::Error)]
#[error("parse: {0}")]
pub struct ParseJsonError(#[from] pub serde_json::Error);

impl ResponseError for ParseJsonError {
    fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

/// A possible error value when parsing query.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ParseQueryError(#[from] pub serde_urlencoded::de::Error);

impl ResponseError for ParseQueryError {
    fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

/// A possible error value when parsing multipart.
#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
#[derive(Debug, thiserror::Error)]
pub enum ParseMultipartError {
    /// Invalid content type.
    #[error("invalid content type `{0}`, expect: `multipart/form-data`")]
    InvalidContentType(String),

    /// `Content-Type` header is required.
    #[error("expect content type `multipart/form-data`")]
    ContentTypeRequired,

    /// Parse error.
    #[error("parse: {0}")]
    Multipart(#[from] multer::Error),
}

#[cfg(feature = "multipart")]
impl ResponseError for ParseMultipartError {
    fn status(&self) -> StatusCode {
        match self {
            ParseMultipartError::InvalidContentType(_) => StatusCode::BAD_REQUEST,
            ParseMultipartError::ContentTypeRequired => StatusCode::BAD_REQUEST,
            ParseMultipartError::Multipart(_) => StatusCode::BAD_REQUEST,
        }
    }
}

/// A possible error value when parsing typed headers.
#[derive(Debug, thiserror::Error)]
pub enum ParseTypedHeaderError {
    /// A specified header is required.
    #[error("header `{0}` is required")]
    HeaderRequired(String),

    /// Parse error.
    #[error("parse: {0}")]
    TypedHeader(#[from] headers::Error),
}

impl ResponseError for ParseTypedHeaderError {
    fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

/// A possible error value when handling websocket.
#[cfg(feature = "websocket")]
#[cfg_attr(docsrs, doc(cfg(feature = "websocket")))]
#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
    /// Invalid protocol
    #[error("invalid protocol")]
    InvalidProtocol,

    /// Upgrade Error
    #[error(transparent)]
    UpgradeError(#[from] UpgradeError),
}

#[cfg(feature = "websocket")]
impl ResponseError for WebSocketError {
    fn status(&self) -> StatusCode {
        match self {
            WebSocketError::InvalidProtocol => StatusCode::BAD_REQUEST,
            WebSocketError::UpgradeError(err) => err.status(),
        }
    }
}

/// A possible error value when upgrading connection.
#[derive(Debug, thiserror::Error)]
pub enum UpgradeError {
    /// No upgrade
    #[error("no upgrade")]
    NoUpgrade,

    /// Other error
    #[error("{0}")]
    Other(String),
}

impl ResponseError for UpgradeError {
    fn status(&self) -> StatusCode {
        match self {
            UpgradeError::NoUpgrade => StatusCode::INTERNAL_SERVER_ERROR,
            UpgradeError::Other(_) => StatusCode::BAD_REQUEST,
        }
    }
}

/// A possible error value when processing static files.
#[derive(Debug, thiserror::Error)]
pub enum StaticFileError {
    /// Method not allow
    #[error("method not found")]
    MethodNotAllowed(Method),

    /// Invalid path
    #[error("invalid path")]
    InvalidPath,

    /// Forbidden
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// File not found
    #[error("not found: {0}")]
    NotFound(String),

    /// Io error
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl ResponseError for StaticFileError {
    fn status(&self) -> StatusCode {
        match self {
            StaticFileError::MethodNotAllowed(_) => StatusCode::METHOD_NOT_ALLOWED,
            StaticFileError::InvalidPath => StatusCode::BAD_REQUEST,
            StaticFileError::Forbidden(_) => StatusCode::FORBIDDEN,
            StaticFileError::NotFound(_) => StatusCode::NOT_FOUND,
            StaticFileError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// A possible error value occurred in the `SizeLimit` middleware.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum SizedLimitError {
    /// Missing `Content-Length` header
    #[error("missing `Content-Length` header")]
    MissingContentLength,

    /// Payload too large
    #[error("payload too large")]
    PayloadTooLarge,
}

impl ResponseError for SizedLimitError {
    fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

/// A possible error value occurred when adding a route.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum RouteError {
    /// Invalid path
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Duplicate path
    #[error("duplicate path: {0}")]
    Duplicate(String),

    /// Invalid regex in path
    #[error("invalid regex in path: {path}")]
    InvalidRegex {
        /// Path
        path: String,

        /// Regex
        regex: String,
    },
}

impl ResponseError for RouteError {
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error as IoError, ErrorKind};

    use super::*;

    #[test]
    fn test_into_result() {
        assert!(matches!("hello".into_result(), Ok("hello")));
        assert!(matches!(Ok::<_, Error>("hello").into_result(), Ok("hello")));
        assert!(matches!(
            Ok::<_, NotFoundError>("hello").into_result(),
            Ok("hello")
        ));
        assert!(Err::<String, _>(NotFoundError)
            .into_result()
            .unwrap_err()
            .is::<NotFoundError>());
    }

    #[test]
    fn test_error() {
        let err = Error::new(
            IoError::new(ErrorKind::AlreadyExists, "aaa"),
            StatusCode::BAD_GATEWAY,
        );
        assert_eq!(err.status(), StatusCode::BAD_GATEWAY);
        assert!(err.is::<IoError>());
        assert_eq!(
            err.downcast_ref::<IoError>().unwrap().kind(),
            ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn test_box_error() {
        let boxed_err: Box<dyn StdError + Send + Sync> =
            Box::new(IoError::new(ErrorKind::AlreadyExists, "aaa"));
        let err: Error = Error::from((StatusCode::BAD_GATEWAY, boxed_err));
        assert_eq!(err.status(), StatusCode::BAD_GATEWAY);
        assert!(err.is::<IoError>());
        assert_eq!(
            err.downcast_ref::<IoError>().unwrap().kind(),
            ErrorKind::AlreadyExists
        );
    }

    #[cfg(feature = "anyhow")]
    #[test]
    fn test_anyhow_error() {
        let anyhow_err: anyhow::Error = IoError::new(ErrorKind::AlreadyExists, "aaa").into();
        let err: Error = Error::from((StatusCode::BAD_GATEWAY, anyhow_err));
        assert_eq!(err.status(), StatusCode::BAD_GATEWAY);
        assert!(err.is::<IoError>());
        assert_eq!(
            err.downcast_ref::<IoError>().unwrap().kind(),
            ErrorKind::AlreadyExists
        );
    }
}
