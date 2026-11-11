//! OpenAPI schema collection and result handling.
//!
//! This module provides:
//!
//! - [`CallResult`] - Response wrapper for deserializing and collecting schemas
//! - [`RawResult`] - Raw response access before schema collection
//! - [`RawBody`] - Raw response body content
//!
//! Internal types for schema collection are not exported.

pub(in crate::client) mod channel;
pub(in crate::client) mod schema;

mod result;
// CallResult, RawResult, and RawBody are public API
pub use self::result::{CallResult, RawBody, RawResult};

mod operation;
pub(in crate::client) use self::operation::CalledOperation;

mod collectors;

mod compat;
pub(crate) use self::compat::iter_operations;
pub(in crate::client) use self::compat::{downgrade_paths_to_31, downgrade_to_31};
