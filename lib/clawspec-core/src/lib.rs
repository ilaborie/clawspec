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
//!         .with_host("api.example.com")
//!         .with_base_path("/v1")?
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
//!         .with_query(query)
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
//!     .with_scheme(Scheme::HTTPS)
//!     .with_host("api.github.com")
//!     .with_port(443)
//!     .with_base_path("/api/v3")?
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
//!     .with_query(query)
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
//! Flexible status code validation with ranges and specific codes using the convenient
//! `expected_status_codes!` macro or the builder pattern:
//!
//! ```rust
//! use clawspec_core::{ApiClient, expected_status_codes};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Using the convenient macro syntax
//! let result = client
//!     .post("/users")?
//!     .with_expected_status_codes(expected_status_codes!(201, 202))
//!     .exchange()
//!     .await?;
//!
//! // Accept a range of codes
//! let result = client
//!     .get("/health")?
//!     .with_expected_status_codes(expected_status_codes!(200-299))
//!     .exchange()
//!     .await?;
//!
//! // Complex patterns
//! let result = client
//!     .patch("/users/123")?
//!     .with_expected_status_codes(expected_status_codes!(200-204, 400-404, 422))
//!     .exchange()
//!     .await?;
//!
//! // Traditional builder pattern is still available
//! let result = client
//!     .get("/legacy")?
//!     .with_status_range_inclusive(200..=299)
//!     .add_expected_status(404)
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

/// Macro for creating `ExpectedStatusCodes` with convenient range syntax.
///
/// This macro supports:
/// - Single status codes: `200`, `404`, `418`
/// - Inclusive ranges: `200-204` (equivalent to `200..=204`)
/// - Comma-separated combinations: `200-204, 215, 400-403, 418`
///
/// Status codes must be valid HTTP status codes (100-599).
///
/// # Examples
///
/// ```rust
/// use clawspec_core::expected_status_codes;
///
/// // Single status code
/// let codes = expected_status_codes!(200);
/// assert!(codes.contains(200));
/// assert!(!codes.contains(201));
///
/// // Inclusive range
/// let codes = expected_status_codes!(200-204);
/// assert!(codes.contains(200));
/// assert!(codes.contains(202));
/// assert!(codes.contains(204));
/// assert!(!codes.contains(199));
/// assert!(!codes.contains(205));
///
/// // Multiple elements
/// let codes = expected_status_codes!(200-204, 215, 400-403, 418);
/// assert!(codes.contains(200));   // first range
/// assert!(codes.contains(204));   // first range
/// assert!(codes.contains(215));   // single code
/// assert!(codes.contains(400));   // second range
/// assert!(codes.contains(403));   // second range
/// assert!(codes.contains(418));   // single code
/// assert!(!codes.contains(205));  // outside ranges
/// assert!(!codes.contains(404));  // outside ranges
/// ```
///
/// # Integration with ApiCall
///
/// The macro seamlessly integrates with the `ApiCall::with_expected_status_codes()` method:
///
/// ```rust
/// use clawspec_core::{ApiClient, expected_status_codes};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
///
/// // Simple patterns
/// let call = client.get("/users")?
///     .with_expected_status_codes(expected_status_codes!(200-299));
///
/// // Complex patterns  
/// let call = client.post("/users")?
///     .with_expected_status_codes(expected_status_codes!(200-204, 215, 400-403, 418));
///
/// // Real-world REST API patterns
/// let call = client.get("/users/123")?
///     .with_expected_status_codes(expected_status_codes!(200, 404));
///
/// let call = client.post("/users")?
///     .with_expected_status_codes(expected_status_codes!(201, 409));
///
/// let call = client.delete("/users/123")?
///     .with_expected_status_codes(expected_status_codes!(204, 404));
/// # Ok(())
/// # }
/// ```
///
/// # Benefits
///
/// - **Concise Syntax**: More readable than verbose builder patterns
/// - **Familiar Pattern**: Similar to HTTP status code ranges in web servers
/// - **Type Safety**: Compile-time validation through Rust's type system
/// - **No Runtime Overhead**: Expands to the existing builder pattern at compile time
#[macro_export]
macro_rules! expected_status_codes {
    // Single status code
    ($code:literal) => {
        $crate::ExpectedStatusCodes::from_single($code)
    };

    // Range pattern: start-end
    ($start:literal - $end:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start..=$end)
    };

    // Two elements: range, single
    ($start1:literal - $end1:literal, $code2:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start1..=$end1)
            .add_expected_status($code2)
    };

    // Two elements: single, range
    ($code1:literal, $start2:literal - $end2:literal) => {
        $crate::ExpectedStatusCodes::from_single($code1).add_expected_range($start2..=$end2)
    };

    // Two elements: range, range
    ($start1:literal - $end1:literal, $start2:literal - $end2:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start1..=$end1)
            .add_expected_range($start2..=$end2)
    };

    // Two elements: single, single
    ($code1:literal, $code2:literal) => {
        $crate::ExpectedStatusCodes::from_single($code1).add_expected_status($code2)
    };

    // Three elements: range, range, single
    ($start1:literal - $end1:literal, $start2:literal - $end2:literal, $code3:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start1..=$end1)
            .add_expected_range($start2..=$end2)
            .add_expected_status($code3)
    };

    // Three elements: range, single, range
    ($start1:literal - $end1:literal, $code2:literal, $start3:literal - $end3:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start1..=$end1)
            .add_expected_status($code2)
            .add_expected_range($start3..=$end3)
    };

    // Three elements: single, single, single
    ($code1:literal, $code2:literal, $code3:literal) => {
        $crate::ExpectedStatusCodes::from_single($code1)
            .add_expected_status($code2)
            .add_expected_status($code3)
    };

    // Four elements: range, single, range, single (most complex case from examples)
    ($start1:literal - $end1:literal, $code2:literal, $start3:literal - $end3:literal, $code4:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start1..=$end1)
            .add_expected_status($code2)
            .add_expected_range($start3..=$end3)
            .add_expected_status($code4)
    };

    // Handle trailing comma for two elements
    ($code1:literal, $code2:literal,) => {
        expected_status_codes!($code1, $code2)
    };

    // Handle trailing comma for range + single
    ($start:literal - $end:literal, $code:literal,) => {
        expected_status_codes!($start - $end, $code)
    };
}

#[cfg(test)]
mod macro_tests {
    use super::*;

    #[test]
    fn test_expected_status_codes_single() {
        let codes = expected_status_codes!(200);

        assert!(codes.contains(200));
        assert!(!codes.contains(199));
        assert!(!codes.contains(201));
    }

    #[test]
    fn test_expected_status_codes_range() {
        let codes = expected_status_codes!(200 - 204);

        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(204));
        assert!(!codes.contains(199));
        assert!(!codes.contains(205));
    }

    #[test]
    fn test_expected_status_codes_multiple_single() {
        let codes = expected_status_codes!(200, 404, 418);

        assert!(codes.contains(200));
        assert!(codes.contains(404));
        assert!(codes.contains(418));
        assert!(!codes.contains(201));
        assert!(!codes.contains(403));
        assert!(!codes.contains(419));
    }

    #[test]
    fn test_expected_status_codes_multiple_ranges() {
        let codes = expected_status_codes!(200 - 204, 400 - 403);

        // First range
        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(204));

        // Second range
        assert!(codes.contains(400));
        assert!(codes.contains(401));
        assert!(codes.contains(403));

        // Outside ranges
        assert!(!codes.contains(199));
        assert!(!codes.contains(205));
        assert!(!codes.contains(399));
        assert!(!codes.contains(404));
    }

    #[test]
    fn test_expected_status_codes_mixed() {
        let codes = expected_status_codes!(200 - 204, 215, 400 - 403, 418);

        // First range
        assert!(codes.contains(200));
        assert!(codes.contains(204));

        // Single code
        assert!(codes.contains(215));

        // Second range
        assert!(codes.contains(400));
        assert!(codes.contains(403));

        // Another single code
        assert!(codes.contains(418));

        // Outside
        assert!(!codes.contains(205));
        assert!(!codes.contains(214));
        assert!(!codes.contains(216));
        assert!(!codes.contains(404));
        assert!(!codes.contains(417));
        assert!(!codes.contains(419));
    }

    #[test]
    fn test_expected_status_codes_trailing_comma() {
        let codes = expected_status_codes!(200, 404,);

        assert!(codes.contains(200));
        assert!(codes.contains(404));
        assert!(!codes.contains(201));
        assert!(!codes.contains(403));
    }

    #[test]
    fn test_expected_status_codes_range_trailing_comma() {
        let codes = expected_status_codes!(200 - 204, 404,);

        assert!(codes.contains(200));
        assert!(codes.contains(204));
        assert!(codes.contains(404));
        assert!(!codes.contains(205));
        assert!(!codes.contains(403));
    }

    #[test]
    fn test_expected_status_codes_edge_cases() {
        // Test edge HTTP status codes
        let codes = expected_status_codes!(100, 599);

        assert!(codes.contains(100));
        assert!(codes.contains(599));
        assert!(!codes.contains(99));
        assert!(!codes.contains(600));
    }

    #[test]
    fn test_expected_status_codes_common_patterns() {
        // Common 2xx pattern
        let success_codes = expected_status_codes!(200 - 299);
        assert!(success_codes.contains(200));
        assert!(success_codes.contains(201));
        assert!(success_codes.contains(299));
        assert!(!success_codes.contains(300));

        // Common error handling pattern
        let error_codes = expected_status_codes!(200 - 204, 400 - 404, 500);
        assert!(error_codes.contains(200));
        assert!(error_codes.contains(204));
        assert!(error_codes.contains(400));
        assert!(error_codes.contains(404));
        assert!(error_codes.contains(500));
        assert!(!error_codes.contains(205));
        assert!(!error_codes.contains(405));
        assert!(!error_codes.contains(501));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_expected_status_codes_with_api_call() -> Result<(), ApiClientError> {
        let client = ApiClient::builder().build()?;

        // Test that the macro works with with_expected_status_codes
        let _call = client
            .get("/test")?
            .with_expected_status_codes(expected_status_codes!(200));

        let _call = client
            .get("/test")?
            .with_expected_status_codes(expected_status_codes!(200 - 299));

        let _call = client
            .get("/test")?
            .with_expected_status_codes(expected_status_codes!(200, 404));

        let _call = client
            .get("/test")?
            .with_expected_status_codes(expected_status_codes!(200 - 204, 400 - 404));

        let _call = client
            .get("/test")?
            .with_expected_status_codes(expected_status_codes!(200 - 204, 215, 400 - 403, 418));

        Ok(())
    }

    #[test]
    fn test_expected_status_codes_method_chaining() -> Result<(), ApiClientError> {
        let client = ApiClient::builder().build()?;

        // Test that it works in method chains
        let _call = client
            .post("/users")?
            .with_expected_status_codes(expected_status_codes!(201, 409))
            .with_header("Content-Type", "application/json");

        let _call = client
            .delete("/users/123")?
            .with_expected_status_codes(expected_status_codes!(204, 404))
            .with_header("Authorization", "Bearer token");

        Ok(())
    }

    #[test]
    fn test_expected_status_codes_real_world_patterns() -> Result<(), ApiClientError> {
        let client = ApiClient::builder().build()?;

        // Common REST API patterns

        // GET - accept success and not found
        let _call = client
            .get("/users/123")?
            .with_expected_status_codes(expected_status_codes!(200, 404));

        // POST - accept created or conflict
        let _call = client
            .post("/users")?
            .with_expected_status_codes(expected_status_codes!(201, 409));

        // PUT - accept success, created, or not found
        let _call = client
            .put("/users/123")?
            .with_expected_status_codes(expected_status_codes!(200, 201, 404));

        // DELETE - accept no content or not found
        let _call = client
            .delete("/users/123")?
            .with_expected_status_codes(expected_status_codes!(204, 404));

        // Health check - accept any 2xx
        let _call = client
            .get("/health")?
            .with_expected_status_codes(expected_status_codes!(200 - 299));

        // Complex API with multiple acceptable responses
        let _call = client
            .post("/batch")?
            .with_expected_status_codes(expected_status_codes!(200 - 202, 207, 400 - 404, 422));

        Ok(())
    }

    #[test]
    fn test_expected_status_codes_vs_manual_creation() {
        // Test that macro produces equivalent results to manual creation

        // Single code
        let macro_codes = expected_status_codes!(200);
        let manual_codes = ExpectedStatusCodes::from_single(200);
        assert_eq!(macro_codes.contains(200), manual_codes.contains(200));
        assert_eq!(macro_codes.contains(201), manual_codes.contains(201));

        // Range
        let macro_codes = expected_status_codes!(200 - 204);
        let manual_codes = ExpectedStatusCodes::from_inclusive_range(200..=204);
        for status in 190..220 {
            assert_eq!(macro_codes.contains(status), manual_codes.contains(status));
        }

        // Complex pattern
        let macro_codes = expected_status_codes!(200 - 204, 215, 400 - 403, 418);
        let manual_codes = ExpectedStatusCodes::from_inclusive_range(200..=204)
            .add_expected_status(215)
            .add_expected_range(400..=403)
            .add_expected_status(418);

        for status in 190..430 {
            assert_eq!(
                macro_codes.contains(status),
                manual_codes.contains(status),
                "Status {status} should match between macro and manual creation"
            );
        }
    }
}
