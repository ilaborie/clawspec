// TODO: Add comprehensive documentation for all public APIs - https://github.com/ilaborie/clawspec/issues/34
// TODO: Add comprehensive unit tests for all modules - https://github.com/ilaborie/clawspec/issues/30

mod client;

// Public API - only expose user-facing types and functions
pub use self::client::{
    ApiCall, ApiClient, ApiClientBuilder, ApiClientError, CallBody, CallHeaders, CallPath,
    CallQuery, CallResult, ExpectedStatusCodes, ParamStyle, ParamValue, ParameterValue,
};

/// Macro for registering multiple schemas at once in an ApiClient.
///
/// This macro provides a convenient way to register multiple types that implement
/// `ToSchema` in a single call. It's more convenient than calling `register_schema`
/// multiple times.
///
/// # Example
///
/// ```rust
/// use clawspec_utoipa::{ApiClient, register_schemas};
/// # use utoipa::ToSchema;
/// # use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// struct ErrorType { message: String }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// struct DataType { value: i32 }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// struct ResponseType { success: bool }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
///
/// // Register multiple schemas at once
/// register_schemas!(client, ErrorType, DataType, ResponseType);
///
/// let openapi = client.collected_openapi().await;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! register_schemas {
    ($client:expr, $($schema_type:ty),+ $(,)?) => {
        {
            $(
                $client.register_schema::<$schema_type>().await;
            )+
        }
    };
}
