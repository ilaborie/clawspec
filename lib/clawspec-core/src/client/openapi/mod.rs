pub(in crate::client) mod schema;

mod collectors;
// CallResult, RawResult, and RawBody are public API
pub use self::collectors::{CallResult, RawBody, RawResult};
// CalledOperation and Collectors are internal to the client module
pub(in crate::client) use self::collectors::{CalledOperation, Collectors};
