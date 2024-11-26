#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[doc(hidden)]
pub mod codegen;

pub use twurst_error::{TwirpError, TwirpErrorCode};
