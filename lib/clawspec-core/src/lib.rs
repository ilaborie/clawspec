//! # Clawspec Core
//!
//! A Rust library for generating OpenAPI specifications from HTTP client interactions.
//! This crate provides a type-safe HTTP client that automatically captures request/response
//! schemas and generates comprehensive OpenAPI documentation.
//!
//! ## Key Features
//!
//! - **Test-Driven Documentation**: Generate OpenAPI specs by writing tests
//! - **Type Safety**: Compile-time guarantees for API parameters and responses
//! - **Automatic Schema Generation**: Extract schemas from Rust types using utoipa
//! - **Multiple Parameter Styles**: Support for path, query, and header parameters
//! - **Comprehensive Status Code Handling**: Flexible validation and error handling
//! - **Builder Pattern APIs**: Ergonomic client and request configuration
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallPath, CallQuery, CallHeaders, ParamValue};
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Debug, Deserialize, ToSchema)]
//! struct User {
//!     id: u32,
//!     name: String,
//!     email: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create an API client
//!     let mut client = ApiClient::builder()
//!         .host("api.example.com")
//!         .base_path("/v1")?
//!         .build()?;
//!
//!     // Create path with parameters
//!     let mut path = CallPath::from("/users/{id}");
//!     path.add_param("id", ParamValue::new(123));
//!
//!     // Create query parameters
//!     let query = CallQuery::new()
//!         .add_param("include_profile", ParamValue::new(true));
//!
//!     // Create headers
//!     let headers = CallHeaders::new()
//!         .add_header("Authorization", "Bearer token");
//!
//!     // Make a request and capture the schema
//!     let user: User = client
//!         .get(path)?
//!         .query(query)
//!         .with_headers(headers)
//!         .exchange()
//!         .await?
//!         .as_json()
//!         .await?;
//!
//!     // Generate OpenAPI specification
//!     let openapi_spec = client.collected_openapi().await;
//!     let yaml = serde_yaml::to_string(&openapi_spec)?;
//!     println!("{yaml}");
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Core Concepts
//!
//! ### ApiClient
//!
//! The [`ApiClient`] is the main entry point for making HTTP requests. It automatically
//! collects request/response information to build OpenAPI specifications.
//!
//! ```rust
//! use clawspec_core::ApiClient;
//! use http::uri::Scheme;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = ApiClient::builder()
//!     .scheme(Scheme::HTTPS)
//!     .host("api.github.com")
//!     .port(443)
//!     .base_path("/api/v3")?
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Parameter Handling
//!
//! The library supports three types of parameters with full OpenAPI 3.1 compliance:
//!
//! - **Path Parameters**: URL path segments like `/users/{id}`
//! - **Query Parameters**: URL query string parameters like `?page=1&limit=10`
//! - **Header Parameters**: HTTP headers like `Authorization: Bearer token`
//!
//! ```rust
//! use clawspec_core::{ApiClient, CallPath, CallQuery, CallHeaders, ParamValue, ParamStyle};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Create path with parameter
//! let mut path = CallPath::from("/search/{category}");
//! path.add_param("category", ParamValue::new("electronics"));
//!
//! // Create query with space-delimited array
//! let query = CallQuery::new()
//!     .add_param("tags", ParamValue::with_style(
//!         vec!["phone", "android"],
//!         ParamStyle::SpaceDelimited
//!     ));
//!
//! // Create headers
//! let headers = CallHeaders::new()
//!     .add_header("Accept-Language", "en-US,en;q=0.9");
//!
//! let result = client
//!     .get(path)?
//!     .query(query)
//!     .with_headers(headers)
//!     .exchange()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Request Bodies
//!
//! Support for various request body formats:
//!
//! ```rust
//! use clawspec_core::ApiClient;
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Serialize, ToSchema)]
//! struct CreateUser {
//!     name: String,
//!     email: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let new_user = CreateUser {
//!     name: "John Doe".to_string(),
//!     email: "john@example.com".to_string(),
//! };
//!
//! // JSON request body
//! let response = client
//!     .post("/users")?
//!     .json(&new_user)?
//!     .exchange()
//!     .await?;
//!
//! // Form-encoded request body
//! let response = client
//!     .post("/users")?
//!     .form(&new_user)?
//!     .exchange()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Response Handling
//!
//! Flexible response processing with automatic schema extraction:
//!
//! ```rust
//! use clawspec_core::ApiClient;
//! use serde::Deserialize;
//! use utoipa::ToSchema;
//!
//! #[derive(Deserialize, ToSchema)]
//! struct ApiResponse<T> {
//!     data: T,
//!     status: String,
//! }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User {
//!     id: u32,
//!     name: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let mut result = client
//!     .get("/users/123")?
//!     .exchange()
//!     .await?;
//!
//! // Different response formats
//! let user: ApiResponse<User> = result.as_json().await?;
//! let raw_text: &str = result.as_text().await?;
//! let raw_bytes: &[u8] = result.as_bytes().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Status Code Validation
//!
//! Flexible status code validation with ranges and specific codes:
//!
//! ```rust
//! use clawspec_core::{ApiClient, ExpectedStatusCodes};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Accept multiple success codes
//! let result = client
//!     .post("/users")?
//!     .set_expected_status(201)
//!     .add_expected_status(202)
//!     .exchange()
//!     .await?;
//!
//! // Accept a range of codes
//! let result = client
//!     .get("/health")?
//!     .set_status_range_inclusive(200..=299)
//!     .exchange()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error handling through [`ApiClientError`]:
//!
//! ```rust
//! use clawspec_core::{ApiClient, ApiClientError};
//!
//! # async fn example() -> Result<(), ApiClientError> {
//! # let mut client = ApiClient::builder().build()?;
//! match client.get("/invalid-endpoint")?.exchange().await {
//!     Ok(mut response) => {
//!         // Handle successful response
//!         println!("Success!");
//!     }
//!     Err(ApiClientError::UnexpectedStatusCode { status_code, body }) => {
//!         // Handle specific HTTP errors
//!         println!("HTTP {} error: {}", status_code, body);
//!     }
//!     Err(ApiClientError::ReqwestError(source)) => {
//!         // Handle network/request errors
//!         println!("Request failed: {}", source);
//!     }
//!     Err(err) => {
//!         // Handle other errors
//!         println!("Other error: {}", err);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Testing Integration
//!
//! Perfect for integration testing and documentation generation:
//!
//! ```rust
//! #[cfg(test)]
//! mod tests {
//!     use super::*;
//!     use clawspec_core::{ApiClient, register_schemas};
//!
//!     #[derive(serde::Deserialize, utoipa::ToSchema)]
//!     struct TestResponse {
//!         message: String,
//!     }
//!
//!     #[tokio::test]
//!     async fn test_api_and_generate_docs() -> Result<(), Box<dyn std::error::Error>> {
//!         let mut client = ApiClient::builder()
//!             .host("httpbin.org")
//!             .build()?;
//!
//!         // Register schemas for better documentation
//!         register_schemas!(client, TestResponse);
//!
//!         // Test the API endpoint
//!         let response: TestResponse = client
//!             .get("/json")?
//!             .exchange()
//!             .await?
//!             .as_json()
//!             .await?;
//!
//!         assert!(!response.message.is_empty());
//!
//!         // Generate OpenAPI documentation
//!         let openapi = client.collected_openapi().await;
//!         
//!         // Save to file for documentation
//!         let yaml = serde_yaml::to_string(&openapi)?;
//!         std::fs::write("api-docs.yml", yaml)?;
//!
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## Re-exports
//!
//! All commonly used types are re-exported from the crate root for convenience.

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
/// use clawspec_core::{ApiClient, register_schemas};
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
