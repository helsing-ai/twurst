#![doc = include_str!("../README.md")]
#![doc(
    test(attr(deny(warnings))),
    html_favicon_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png",
    html_logo_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// A Twirp [error](https://twitchtv.github.io/twirp/docs/spec_v7.html#errors)
///
/// It is composed of three elements:
/// - An error `code` that is member of a fixed list [`TwirpErrorCode`]
/// - A human error `message` describing the error as a string
/// - A set of "`meta`" key-value pairs as strings holding arbitrary metadata describing the error.
///
/// ```
/// # use twurst_error::{TwirpError, TwirpErrorCode};
/// let error = TwirpError::not_found("Object foo not found").with_meta("id", "foo");
/// assert_eq!(error.code(), TwirpErrorCode::NotFound);
/// assert_eq!(error.message(), "Object foo not found");
/// assert_eq!(error.meta("id"), Some("foo"));
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TwirpError {
    /// The [error code](https://twitchtv.github.io/twirp/docs/spec_v7.html#error-codes)
    code: TwirpErrorCode,
    /// The error message (human description of the error)
    msg: String,
    /// Some metadata describing the error
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "HashMap::is_empty")
    )]
    meta: HashMap<String, String>,
    #[cfg_attr(feature = "serde", serde(default, skip))]
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl TwirpError {
    #[inline]
    pub fn code(&self) -> TwirpErrorCode {
        self.code
    }

    #[inline]
    pub fn message(&self) -> &str {
        &self.msg
    }

    #[inline]
    pub fn into_message(self) -> String {
        self.msg
    }

    /// Get an associated metadata
    #[inline]
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(|s| s.as_str())
    }

    /// Get all associated metadata
    #[inline]
    pub fn meta_iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.meta.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    #[inline]
    pub fn new(code: TwirpErrorCode, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
            meta: HashMap::new(),
            source: None,
        }
    }

    #[inline]
    pub fn wrap(
        code: TwirpErrorCode,
        msg: impl Into<String>,
        e: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            msg: msg.into(),
            meta: HashMap::new(),
            source: Some(Arc::new(e)),
        }
    }

    /// Set an associated metadata
    #[inline]
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }

    #[inline]
    pub fn aborted(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Aborted, msg)
    }

    #[inline]
    pub fn already_exists(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::AlreadyExists, msg)
    }

    #[inline]
    pub fn canceled(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Canceled, msg)
    }

    #[inline]
    pub fn dataloss(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Dataloss, msg)
    }

    #[inline]
    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::InvalidArgument, msg)
    }

    #[inline]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Internal, msg)
    }

    #[inline]
    pub fn deadline_exceeded(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::DeadlineExceeded, msg)
    }

    #[inline]
    pub fn failed_precondition(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::FailedPrecondition, msg)
    }

    #[inline]
    pub fn malformed(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Malformed, msg)
    }

    #[inline]
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::NotFound, msg)
    }

    #[inline]
    pub fn out_of_range(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::OutOfRange, msg)
    }

    #[inline]
    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::PermissionDenied, msg)
    }

    #[inline]
    pub fn required_argument(msg: impl Into<String>) -> Self {
        Self::invalid_argument(msg)
    }

    #[inline]
    pub fn resource_exhausted(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::ResourceExhausted, msg)
    }

    #[inline]
    pub fn unauthenticated(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Unauthenticated, msg)
    }

    #[inline]
    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Unavailable, msg)
    }

    #[inline]
    pub fn unimplemented(msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::Unimplemented, msg)
    }
}

impl fmt::Display for TwirpError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Twirp {:?} error: {}", self.code, self.msg)
    }
}

impl Error for TwirpError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref()?)
    }
}

impl PartialEq for TwirpError {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.msg == other.msg && self.meta == other.meta
    }
}

impl Eq for TwirpError {}

/// A Twirp [error code](https://twitchtv.github.io/twirp/docs/spec_v7.html#error-codes)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TwirpErrorCode {
    /// The operation was cancelled.
    Canceled,
    /// An unknown error occurred. For example, this can be used when handling errors raised by APIs that do not return any error information.
    Unknown,
    /// The client specified an invalid argument. This indicates arguments that are invalid regardless of the state of the system (i.e. a malformed file name, required argument, number out of range, etc.).
    InvalidArgument,
    /// The client sent a message which could not be decoded. This may mean that the message was encoded improperly or that the client and server have incompatible message definitions.
    Malformed,
    /// Operation expired before completion. For operations that change the state of the system, this error may be returned even if the operation has completed successfully (timeout).
    DeadlineExceeded,
    /// Some requested entity was not found.
    NotFound,
    /// The requested URL path wasn't routable to a Twirp service and method. This is returned by generated server code and should not be returned by application code (use "not_found" or "unimplemented" instead).
    BadRoute,
    /// An attempt to create an entity failed because one already exists.
    AlreadyExists,
    /// The caller does not have permission to execute the specified operation. It must not be used if the caller cannot be identified (use "unauthenticated" instead).
    PermissionDenied,
    /// The request does not have valid authentication credentials for the operation.
    Unauthenticated,
    /// Some resource has been exhausted or rate-limited, perhaps a per-user quota, or perhaps the entire file system is out of space.
    ResourceExhausted,
    /// The operation was rejected because the system is not in a state required for the operation's execution. For example, doing an rmdir operation on a directory that is non-empty, or on a non-directory object, or when having conflicting read-modify-write on the same resource.
    FailedPrecondition,
    /// The operation was aborted, typically due to a concurrency issue like sequencer check failures, transaction aborts, etc.
    Aborted,
    /// The operation was attempted past the valid range. For example, seeking or reading past end of a paginated collection. Unlike "invalid_argument", this error indicates a problem that may be fixed if the system state changes (i.e. adding more items to the collection). There is a fair bit of overlap between "failed_precondition" and "out_of_range". We recommend using "out_of_range" (the more specific error) when it applies so that callers who are iterating through a space can easily look for an "out_of_range" error to detect when they are done.
    OutOfRange,
    /// The operation is not implemented or not supported/enabled in this service.
    Unimplemented,
    /// When some invariants expected by the underlying system have been broken. In other words, something bad happened in the library or backend service. Twirp specific issues like wire and serialization problems are also reported as "internal" errors.
    Internal,
    /// The service is currently unavailable. This is most likely a transient condition and may be corrected by retrying with a backoff.
    Unavailable,
    /// The operation resulted in unrecoverable data loss or corruption.
    Dataloss,
}

/// Applies the mapping defined in [Twirp spec](https://twitchtv.github.io/twirp/docs/spec_v7.html#error-codes)
#[cfg(feature = "http")]
impl From<TwirpErrorCode> for http::StatusCode {
    #[inline]
    fn from(code: TwirpErrorCode) -> Self {
        match code {
            TwirpErrorCode::Canceled => Self::REQUEST_TIMEOUT,
            TwirpErrorCode::Unknown => Self::INTERNAL_SERVER_ERROR,
            TwirpErrorCode::InvalidArgument => Self::BAD_REQUEST,
            TwirpErrorCode::Malformed => Self::BAD_REQUEST,
            TwirpErrorCode::DeadlineExceeded => Self::REQUEST_TIMEOUT,
            TwirpErrorCode::NotFound => Self::NOT_FOUND,
            TwirpErrorCode::BadRoute => Self::NOT_FOUND,
            TwirpErrorCode::AlreadyExists => Self::CONFLICT,
            TwirpErrorCode::PermissionDenied => Self::FORBIDDEN,
            TwirpErrorCode::Unauthenticated => Self::UNAUTHORIZED,
            TwirpErrorCode::ResourceExhausted => Self::TOO_MANY_REQUESTS,
            TwirpErrorCode::FailedPrecondition => Self::PRECONDITION_FAILED,
            TwirpErrorCode::Aborted => Self::CONFLICT,
            TwirpErrorCode::OutOfRange => Self::BAD_REQUEST,
            TwirpErrorCode::Unimplemented => Self::NOT_IMPLEMENTED,
            TwirpErrorCode::Internal => Self::INTERNAL_SERVER_ERROR,
            TwirpErrorCode::Unavailable => Self::SERVICE_UNAVAILABLE,
            TwirpErrorCode::Dataloss => Self::SERVICE_UNAVAILABLE,
        }
    }
}

#[cfg(feature = "http")]
impl<B: From<String>> From<TwirpError> for http::Response<B> {
    fn from(error: TwirpError) -> Self {
        let json = serde_json::to_string(&error).unwrap();
        http::Response::builder()
            .status(error.code)
            .header(http::header::CONTENT_TYPE, "application/json")
            .extension(error)
            .body(json.into())
            .unwrap()
    }
}

#[cfg(feature = "http")]
impl<B: AsRef<[u8]>> From<http::Response<B>> for TwirpError {
    fn from(response: http::Response<B>) -> Self {
        if let Some(error) = response.extensions().get::<Self>() {
            // We got a ready to use error in the extensions, let's use it
            return error.clone();
        }
        // We are lenient here, a bad error is better than no error at all
        let status = response.status();
        let body = response.into_body();
        if let Ok(error) = serde_json::from_slice::<TwirpError>(body.as_ref()) {
            // The body is an error, we use it
            return error;
        }
        // We don't have a Twirp error, we build a fallback
        let code = if status == http::StatusCode::REQUEST_TIMEOUT {
            TwirpErrorCode::DeadlineExceeded
        } else if status == http::StatusCode::FORBIDDEN {
            TwirpErrorCode::PermissionDenied
        } else if status == http::StatusCode::UNAUTHORIZED {
            TwirpErrorCode::Unauthenticated
        } else if status == http::StatusCode::TOO_MANY_REQUESTS {
            TwirpErrorCode::ResourceExhausted
        } else if status == http::StatusCode::PRECONDITION_FAILED {
            TwirpErrorCode::FailedPrecondition
        } else if status == http::StatusCode::NOT_IMPLEMENTED {
            TwirpErrorCode::Unimplemented
        } else if status == http::StatusCode::TOO_MANY_REQUESTS
            || status == http::StatusCode::BAD_GATEWAY
            || status == http::StatusCode::SERVICE_UNAVAILABLE
            || status == http::StatusCode::GATEWAY_TIMEOUT
        {
            TwirpErrorCode::Unavailable
        } else if status == http::StatusCode::NOT_FOUND {
            TwirpErrorCode::NotFound
        } else if status.is_server_error() {
            TwirpErrorCode::Internal
        } else if status.is_client_error() {
            TwirpErrorCode::Malformed
        } else {
            TwirpErrorCode::Unknown
        };
        TwirpError::new(code, String::from_utf8_lossy(body.as_ref()))
    }
}

#[cfg(feature = "axum-08")]
impl axum_core_05::response::IntoResponse for TwirpError {
    #[inline]
    fn into_response(self) -> axum_core_05::response::Response {
        self.into()
    }
}

#[cfg(feature = "tonic-014")]
impl From<TwirpErrorCode> for tonic_014::Code {
    #[inline]
    fn from(code: TwirpErrorCode) -> Self {
        match code {
            TwirpErrorCode::Canceled => Self::Cancelled,
            TwirpErrorCode::Unknown => Self::Unknown,
            TwirpErrorCode::InvalidArgument => Self::InvalidArgument,
            TwirpErrorCode::Malformed => Self::InvalidArgument,
            TwirpErrorCode::DeadlineExceeded => Self::DeadlineExceeded,
            TwirpErrorCode::NotFound => Self::NotFound,
            TwirpErrorCode::BadRoute => Self::NotFound,
            TwirpErrorCode::AlreadyExists => Self::AlreadyExists,
            TwirpErrorCode::PermissionDenied => Self::PermissionDenied,
            TwirpErrorCode::Unauthenticated => Self::Unauthenticated,
            TwirpErrorCode::ResourceExhausted => Self::ResourceExhausted,
            TwirpErrorCode::FailedPrecondition => Self::FailedPrecondition,
            TwirpErrorCode::Aborted => Self::Aborted,
            TwirpErrorCode::OutOfRange => Self::OutOfRange,
            TwirpErrorCode::Unimplemented => Self::Unimplemented,
            TwirpErrorCode::Internal => Self::Internal,
            TwirpErrorCode::Unavailable => Self::Unavailable,
            TwirpErrorCode::Dataloss => Self::DataLoss,
        }
    }
}

#[cfg(feature = "tonic-014")]
impl From<TwirpError> for tonic_014::Status {
    #[inline]
    fn from(error: TwirpError) -> Self {
        if let Some(source) = &error.source {
            if let Some(status) = source.downcast_ref::<tonic_014::Status>() {
                if status.code() == error.code().into() && status.message() == error.message() {
                    // This is a status wrapped as a Twirp error, we reuse the status to keep the details
                    return status.clone();
                }
            }
        }
        Self::new(error.code().into(), error.into_message())
    }
}

#[cfg(feature = "tonic-014")]
impl From<tonic_014::Code> for TwirpErrorCode {
    #[inline]
    fn from(code: tonic_014::Code) -> TwirpErrorCode {
        match code {
            tonic_014::Code::Cancelled => Self::Canceled,
            tonic_014::Code::Unknown => Self::Unknown,
            tonic_014::Code::InvalidArgument => Self::InvalidArgument,
            tonic_014::Code::DeadlineExceeded => Self::DeadlineExceeded,
            tonic_014::Code::NotFound => Self::NotFound,
            tonic_014::Code::AlreadyExists => Self::AlreadyExists,
            tonic_014::Code::PermissionDenied => Self::PermissionDenied,
            tonic_014::Code::Unauthenticated => Self::Unauthenticated,
            tonic_014::Code::ResourceExhausted => Self::ResourceExhausted,
            tonic_014::Code::FailedPrecondition => Self::FailedPrecondition,
            tonic_014::Code::Aborted => Self::Aborted,
            tonic_014::Code::OutOfRange => Self::OutOfRange,
            tonic_014::Code::Unimplemented => Self::Unimplemented,
            tonic_014::Code::Internal => Self::Internal,
            tonic_014::Code::Unavailable => Self::Unavailable,
            tonic_014::Code::DataLoss => Self::Dataloss,
            tonic_014::Code::Ok => Self::Unknown,
        }
    }
}

#[cfg(feature = "tonic-014")]
impl From<tonic_014::Status> for TwirpError {
    #[inline]
    fn from(status: tonic_014::Status) -> TwirpError {
        Self::wrap(status.code().into(), status.message().to_string(), status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "http")]
    use std::error::Error;

    #[test]
    fn test_accessors() {
        let error = TwirpError::invalid_argument("foo is wrong").with_meta("foo", "bar");
        assert_eq!(error.code(), TwirpErrorCode::InvalidArgument);
        assert_eq!(error.message(), "foo is wrong");
        assert_eq!(error.meta("foo"), Some("bar"));
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_to_response() -> Result<(), Box<dyn Error>> {
        let object =
            TwirpError::permission_denied("Thou shall not pass").with_meta("target", "Balrog");
        let response = http::Response::<Vec<u8>>::from(object);
        assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE),
            Some(&http::HeaderValue::from_static("application/json"))
        );
        assert_eq!(
            response.into_body(), b"{\"code\":\"permission_denied\",\"msg\":\"Thou shall not pass\",\"meta\":{\"target\":\"Balrog\"}}"
        );
        Ok(())
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_from_valid_response() -> Result<(), Box<dyn Error>> {
        let response = http::Response::builder()
            .header(http::header::CONTENT_TYPE, "application/json")
            .body("{\"code\":\"permission_denied\",\"msg\":\"Thou shall not pass\",\"meta\":{\"target\":\"Balrog\"}}")?;
        assert_eq!(
            TwirpError::from(response),
            TwirpError::permission_denied("Thou shall not pass").with_meta("target", "Balrog")
        );
        Ok(())
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_from_plain_response() -> Result<(), Box<dyn Error>> {
        let response = http::Response::builder()
            .status(http::StatusCode::FORBIDDEN)
            .body("Thou shall not pass")?;
        assert_eq!(
            TwirpError::from(response),
            TwirpError::permission_denied("Thou shall not pass")
        );
        Ok(())
    }

    #[cfg(feature = "tonic-014")]
    #[test]
    fn test_from_tonic_014_status_simple() {
        assert_eq!(
            TwirpError::from(tonic_014::Status::not_found("Not found")),
            TwirpError::not_found("Not found")
        );
    }

    #[cfg(feature = "tonic-014")]
    #[test]
    fn test_to_tonic_014_status_simple() {
        let error = TwirpError::not_found("Not found");
        let status = tonic_014::Status::from(error);
        assert_eq!(status.code(), tonic_014::Code::NotFound);
        assert_eq!(status.message(), "Not found");
    }

    #[cfg(feature = "tonic-014")]
    #[test]
    fn test_from_to_tonic_014_status_roundtrip() {
        let status = tonic_014::Status::with_details(
            tonic_014::Code::NotFound,
            "Not found",
            b"some_dummy_details".to_vec().into(),
        );
        let new_status = tonic_014::Status::from(TwirpError::from(status.clone()));
        assert_eq!(status.code(), new_status.code());
        assert_eq!(status.message(), new_status.message());
        assert_eq!(status.details(), new_status.details());
    }
}
