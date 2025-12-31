//! Response handling, status validation, and redaction.
//!
//! This module provides:
//!
//! - [`ExpectedStatusCodes`] - Define valid status codes for API calls
//! - Redaction utilities (with `redaction` feature) for stable examples

mod status;
pub use self::status::ExpectedStatusCodes;

pub(in crate::client) mod output;

#[cfg(feature = "redaction")]
mod redaction;
#[cfg(feature = "redaction")]
pub use self::redaction::{
    RedactOptions, RedactedResult, RedactionBuilder, Redactor, ValueRedactionBuilder, redact_value,
};
