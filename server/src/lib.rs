#![doc = include_str!("../README.md")]
#![doc(
    test(attr(deny(warnings))),
    html_favicon_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png",
    html_logo_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![feature(async_fn_traits)]
#![feature(unboxed_closures)]

#[doc(hidden)]
pub mod codegen;

use axum::http::Uri;
use axum::response::IntoResponse;
pub use twurst_error::{TwirpError, TwirpErrorCode};

/// Fallback method to be used with a Twirp router
pub async fn twirp_fallback(uri: Uri) -> impl IntoResponse {
    TwirpError::new(
        TwirpErrorCode::BadRoute,
        format!("{} is not a supported Twirp method", uri.path()),
    )
}

/// Fallback method to be used with a gRPC router
#[cfg(feature = "grpc")]
pub async fn grpc_fallback(uri: Uri) -> impl IntoResponse {
    tonic::Status::new(
        tonic::Code::NotFound,
        format!("{} is not a supported gRPC method", uri.path()),
    )
    .into_http::<axum::body::Body>()
}
