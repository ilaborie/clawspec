//! Response handling, status validation, and redaction.
//!
//! This module provides:
//!
//! - [`ExpectedStatusCodes`] - Define valid status codes for API calls
//! - [`SseEvent`] - A parsed server-sent event
//! - Redaction utilities (with `redaction` feature) for stable examples

mod status;
pub use self::status::ExpectedStatusCodes;

pub(in crate::client) mod output;

pub(in crate::client) mod sequential;
pub use self::sequential::SseEvent;

#[cfg(feature = "redaction")]
pub(in crate::client) mod redaction;
#[cfg(feature = "redaction")]
pub use self::redaction::{
    RedactOptions, RedactedResult, RedactionBuilder, Redactor, RequestBodyRedactionBuilder,
    ValueRedactionBuilder, redact_value,
};
