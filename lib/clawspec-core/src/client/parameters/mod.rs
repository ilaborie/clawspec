//! Request parameter types for building API calls.
//!
//! This module provides types for handling different kinds of HTTP request parameters:
//!
//! - [`CallPath`] - Path parameters (e.g., `/users/{id}`)
//! - [`CallQuery`] - Query string parameters
//! - [`CallHeaders`] - HTTP headers
//! - [`CallCookies`] - Cookie parameters
//! - [`CallBody`] - Request body content
//!
//! All parameter types support automatic OpenAPI schema generation.
//!
//! See the [Tutorial](crate::_tutorial) for usage examples.

mod param;
pub use self::param::{ParamStyle, ParamValue, ParameterValue};

mod path;
pub use self::path::CallPath;
pub(in crate::client) use self::path::PathResolved;

mod query;
pub use self::query::CallQuery;

mod headers;
pub use self::headers::CallHeaders;

mod cookies;
pub use self::cookies::CallCookies;

mod body;
pub use self::body::CallBody;
