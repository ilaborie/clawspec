// TODO: Add comprehensive documentation for all public APIs - https://github.com/ilaborie/clawspec/issues/34
// TODO: Add comprehensive unit tests for all modules - https://github.com/ilaborie/clawspec/issues/30

mod client;

// Public API - only expose user-facing types and functions
pub use self::client::{
    ApiCall, ApiClient, ApiClientBuilder, ApiClientError, CallBody, CallHeaders, CallPath,
    CallQuery, CallResult, ParamStyle, ParamValue, ParameterValue,
};
