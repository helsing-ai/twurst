#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
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
pub struct TwirpError {
    /// The [error code](https://twitchtv.github.io/twirp/docs/spec_v7.html#error-codes)
    code: TwirpErrorCode,
    /// The error message (human description of the error)
    msg: String,
    /// Some metadata describing the error
    meta: HashMap<String, String>,
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
    pub fn invalid_argument(argument: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(TwirpErrorCode::InvalidArgument, msg).with_meta("argument", argument)
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
    pub fn required_argument(argument: &str) -> Self {
        Self::invalid_argument(argument, format!("{argument} is required"))
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
