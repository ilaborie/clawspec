pub(in crate::client) mod channel;
pub(in crate::client) mod schema;

mod result;
// CallResult, RawResult, and RawBody are public API
pub use self::result::{CallResult, RawBody, RawResult};

mod operation;
pub(in crate::client) use self::operation::CalledOperation;

mod collectors;
// Collectors is internal to the client module
pub(in crate::client) use self::collectors::Collectors;
