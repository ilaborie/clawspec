//! # Clawspec Core
//!
//! Generate OpenAPI specifications from your HTTP client test code.
//!
//! This crate provides two main ways to generate OpenAPI documentation:
//! - **[`ApiClient`]** - Direct HTTP client for fine-grained control
//! - **[`TestClient`](test_client::TestClient)** - Test server integration with automatic lifecycle management
//!
//! ## Quick Start
//!
//! ### Using ApiClient directly
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: u32, name: String }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .build()?;
//!
//! // Make requests - schemas are captured automatically
//! let user: User = client
//!     .get("/users/123")?
//!     .exchange()
//!     .await?
//!     .as_json()  // ← Important: Must consume result for OpenAPI generation!
//!     .await?;
//!
//! // Generate OpenAPI specification
//! let spec = client.collected_openapi().await;
//! # Ok(())
//! # }
//! ```
//!
//! ### Using TestClient with a test server
//!
//! For a complete working example, see the [axum example](https://github.com/ilaborie/clawspec/tree/main/examples/axum-example).
//!
//! ```rust,no_run
//! use clawspec_core::test_client::{TestClient, TestServer};
//! use std::net::TcpListener;
//!
//! # #[derive(Debug)]
//! # struct MyServer;
//! # impl TestServer for MyServer {
//! #     type Error = std::io::Error;
//! #     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//! #         Ok(())
//! #     }
//! # }
//! #[tokio::test]
//! async fn test_api() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = TestClient::start(MyServer).await?;
//!     
//!     // Test your API
//!     let response = client.get("/users")?.exchange().await?;
//!     
//!     // Write OpenAPI spec
//!     client.write_openapi("api.yml").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Working with Parameters
//!
//! ```rust
//! use clawspec_core::{ApiClient, CallPath, CallQuery, CallHeaders, ParamValue};
//!
//! # async fn example(client: &mut ApiClient) -> Result<(), Box<dyn std::error::Error>> {
//! // Path parameters
//! let mut path = CallPath::from("/users/{id}");
//! path.add_param("id", ParamValue::new(123));
//!
//! // Query parameters
//! let query = CallQuery::new()
//!     .add_param("page", ParamValue::new(1))
//!     .add_param("limit", ParamValue::new(10));
//!
//! // Headers
//! let headers = CallHeaders::new()
//!     .add_header("Authorization", "Bearer token");
//!
//! let response = client
//!     .get(path)?
//!     .with_query(query)
//!     .with_headers(headers)
//!     .exchange()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Status Code Validation
//!
//! By default, requests expect status codes in the range 200-499 (inclusive of 200, exclusive of 500).
//! You can customize this behavior:
//!
//! ```rust
//! use clawspec_core::{ApiClient, expected_status_codes};
//!
//! # async fn example(client: &mut ApiClient) -> Result<(), Box<dyn std::error::Error>> {
//! // Single codes
//! client.post("/users")?
//!     .with_expected_status_codes(expected_status_codes!(201, 202))
//!     .exchange()
//!     .await?;
//!
//! // Ranges
//! client.get("/health")?
//!     .with_expected_status_codes(expected_status_codes!(200-299))
//!     .exchange()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Schema Registration
//!
//! ```rust
//! use clawspec_core::{ApiClient, register_schemas};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//!
//! #[derive(Deserialize, ToSchema)]
//! struct CreateUser { name: String, email: String }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct ErrorResponse { code: String, message: String }
//!
//! # fn example(client: &mut ApiClient) {
//! // Register schemas for complete documentation
//! register_schemas!(client, CreateUser, ErrorResponse);
//! # }
//! ```
//!
//! ## Error Handling
//!
//! The library provides two main error types:
//! - [`ApiClientError`] - HTTP client errors (network, parsing, validation)
//! - [`TestAppError`](test_client::TestAppError) - Test server lifecycle errors
//!
//! ## See Also
//!
//! - [`ApiClient`] - HTTP client with OpenAPI collection
//! - [`ApiCall`] - Request builder with parameter support
//! - [`test_client`] - Test server integration module
//! - [`ExpectedStatusCodes`] - Status code validation
//!
//! ## Re-exports
//!
//! All commonly used types are re-exported from the crate root for convenience.

// TODO: Add comprehensive unit tests for all modules - https://github.com/ilaborie/clawspec/issues/30

mod client;

pub mod test_client;

// Public API - only expose user-facing types and functions
pub use self::client::{
    ApiCall, ApiClient, ApiClientBuilder, ApiClientError, CallBody, CallHeaders, CallPath,
    CallQuery, CallResult, ExpectedStatusCodes, ParamStyle, ParamValue, ParameterValue,
};

// Convenience macro re-exports are handled by the macro_rules! definitions below

/// Creates an [`ExpectedStatusCodes`] instance with the specified status codes and ranges.
///
/// This macro provides a convenient syntax for defining expected HTTP status codes
/// with support for individual codes, inclusive ranges, and exclusive ranges.
///
/// # Syntax
///
/// - Single codes: `200`, `201`, `404`
/// - Inclusive ranges: `200-299` (includes both endpoints)
/// - Exclusive ranges: `200..300` (excludes 300)
/// - Mixed: `200, 201-204, 400..500`
///
/// # Examples
///
/// ```rust
/// use clawspec_core::expected_status_codes;
///
/// // Single status codes
/// let codes = expected_status_codes!(200, 201, 204);
///
/// // Ranges
/// let success_codes = expected_status_codes!(200-299);
/// let client_errors = expected_status_codes!(400..500);
///
/// // Mixed
/// let mixed = expected_status_codes!(200-204, 301, 302, 400-404);
/// ```
#[macro_export]
macro_rules! expected_status_codes {
    // Empty case
    () => {
        $crate::ExpectedStatusCodes::default()
    };

    // Single element
    ($single:literal) => {
        $crate::ExpectedStatusCodes::from_single($single)
    };

    // Single range (inclusive)
    ($start:literal - $end:literal) => {
        $crate::ExpectedStatusCodes::from_inclusive_range($start..=$end)
    };

    // Single range (exclusive)
    ($start:literal .. $end:literal) => {
        $crate::ExpectedStatusCodes::from_exclusive_range($start..$end)
    };

    // Multiple elements - single code followed by more
    ($first:literal, $($rest:tt)*) => {{
        #[allow(unused_mut)]
        let mut codes = $crate::ExpectedStatusCodes::from_single($first);
        $crate::expected_status_codes!(@accumulate codes, $($rest)*);
        codes
    }};

    // Multiple elements - inclusive range followed by more
    ($start:literal - $end:literal, $($rest:tt)*) => {{
        #[allow(unused_mut)]
        let mut codes = $crate::ExpectedStatusCodes::from_inclusive_range($start..=$end);
        $crate::expected_status_codes!(@accumulate codes, $($rest)*);
        codes
    }};

    // Multiple elements - exclusive range followed by more
    ($start:literal .. $end:literal, $($rest:tt)*) => {{
        #[allow(unused_mut)]
        let mut codes = $crate::ExpectedStatusCodes::from_exclusive_range($start..$end);
        $crate::expected_status_codes!(@accumulate codes, $($rest)*);
        codes
    }};

    // Internal accumulator - empty (base case for trailing commas)
    (@accumulate $codes:ident,) => {
        // Do nothing for trailing commas
    };

    // Internal accumulator - single code
    (@accumulate $codes:ident, $single:literal) => {
        $codes = $codes.add_single($single);
    };

    // Internal accumulator - single code followed by more
    (@accumulate $codes:ident, $single:literal, $($rest:tt)*) => {
        $codes = $codes.add_single($single);
        $crate::expected_status_codes!(@accumulate $codes, $($rest)*);
    };

    // Internal accumulator - inclusive range
    (@accumulate $codes:ident, $start:literal - $end:literal) => {
        $codes = $codes.add_inclusive_range($start..=$end);
    };

    // Internal accumulator - inclusive range followed by more
    (@accumulate $codes:ident, $start:literal - $end:literal, $($rest:tt)*) => {
        $codes = $codes.add_inclusive_range($start..=$end);
        $crate::expected_status_codes!(@accumulate $codes, $($rest)*);
    };

    // Internal accumulator - exclusive range
    (@accumulate $codes:ident, $start:literal .. $end:literal) => {
        $codes = $codes.add_exclusive_range($start..$end);
    };

    // Internal accumulator - exclusive range followed by more
    (@accumulate $codes:ident, $start:literal .. $end:literal, $($rest:tt)*) => {
        $codes = $codes.add_exclusive_range($start..$end);
        $crate::expected_status_codes!(@accumulate $codes, $($rest)*);
    };

    // Internal accumulator - empty (catch all for trailing cases)
    (@accumulate $codes:ident) => {
        // Base case - do nothing
    };
}

/// Registers multiple schema types with the ApiClient for OpenAPI documentation.
///
/// This macro simplifies the process of registering multiple types that implement
/// [`utoipa::ToSchema`] with an [`ApiClient`] instance.
///
/// # Examples
///
/// ```rust
/// use clawspec_core::{ApiClient, register_schemas};
/// use serde::{Serialize, Deserialize};
/// use utoipa::ToSchema;
///
/// #[derive(Serialize, Deserialize, ToSchema)]
/// struct User {
///     id: u64,
///     name: String,
/// }
///
/// #[derive(Serialize, Deserialize, ToSchema)]
/// struct Post {
///     id: u64,
///     title: String,
///     author_id: u64,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
///
/// // Register multiple schemas at once
/// register_schemas!(client, User, Post).await;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! register_schemas {
    ($client:expr, $($schema:ty),+ $(,)?) => {
        async {
            $(
                $client.register_schema::<$schema>().await;
            )+
        }
    };
}

#[cfg(test)]
mod macro_tests {
    use super::*;

    #[test]
    fn test_expected_status_codes_single() {
        let codes = expected_status_codes!(200);
        assert!(codes.contains(200));
        assert!(!codes.contains(201));
    }

    #[test]
    fn test_expected_status_codes_multiple_single() {
        let codes = expected_status_codes!(200, 201, 204);
        assert!(codes.contains(200));
        assert!(codes.contains(201));
        assert!(codes.contains(204));
        assert!(!codes.contains(202));
    }

    #[test]
    fn test_expected_status_codes_range() {
        let codes = expected_status_codes!(200 - 204);
        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(204));
        assert!(!codes.contains(205));
    }

    #[test]
    fn test_expected_status_codes_mixed() {
        let codes = expected_status_codes!(200, 201 - 204, 301, 400 - 404);
        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(301));
        assert!(codes.contains(402));
        assert!(!codes.contains(305));
    }

    #[test]
    fn test_expected_status_codes_trailing_comma() {
        let codes = expected_status_codes!(200, 201,);
        assert!(codes.contains(200));
        assert!(codes.contains(201));
    }

    #[test]
    fn test_expected_status_codes_range_trailing_comma() {
        let codes = expected_status_codes!(200 - 204,);
        assert!(codes.contains(202));
    }

    #[test]
    fn test_expected_status_codes_five_elements() {
        let codes = expected_status_codes!(200, 201, 202, 203, 204);
        assert!(codes.contains(200));
        assert!(codes.contains(201));
        assert!(codes.contains(202));
        assert!(codes.contains(203));
        assert!(codes.contains(204));
    }

    #[test]
    fn test_expected_status_codes_eight_elements() {
        let codes = expected_status_codes!(200, 201, 202, 203, 204, 205, 206, 207);
        assert!(codes.contains(200));
        assert!(codes.contains(204));
        assert!(codes.contains(207));
    }

    #[test]
    fn test_expected_status_codes_multiple_ranges() {
        let codes = expected_status_codes!(200 - 204, 300 - 304, 400 - 404);
        assert!(codes.contains(202));
        assert!(codes.contains(302));
        assert!(codes.contains(402));
        assert!(!codes.contains(205));
        assert!(!codes.contains(305));
    }

    #[test]
    fn test_expected_status_codes_edge_cases() {
        // Empty should work
        let _codes = expected_status_codes!();

        // Single range should work
        let codes = expected_status_codes!(200 - 299);
        assert!(codes.contains(250));
    }

    #[test]
    fn test_expected_status_codes_common_patterns() {
        // Success codes
        let success = expected_status_codes!(200 - 299);
        assert!(success.contains(200));
        assert!(success.contains(201));
        assert!(success.contains(204));

        // Client errors
        let client_errors = expected_status_codes!(400 - 499);
        assert!(client_errors.contains(400));
        assert!(client_errors.contains(404));
        assert!(client_errors.contains(422));

        // Specific success codes
        let specific = expected_status_codes!(200, 201, 204);
        assert!(specific.contains(200));
        assert!(!specific.contains(202));
    }

    #[test]
    fn test_expected_status_codes_builder_alternative() {
        // Using macro
        let macro_codes = expected_status_codes!(200 - 204, 301, 302, 400 - 404);

        // Using builder (should be equivalent)
        let builder_codes = ExpectedStatusCodes::default()
            .add_inclusive_range(200..=204)
            .add_single(301)
            .add_single(302)
            .add_inclusive_range(400..=404);

        // Both should have same results
        for code in [200, 202, 204, 301, 302, 400, 402, 404] {
            assert_eq!(macro_codes.contains(code), builder_codes.contains(code));
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_expected_status_codes_real_world_patterns() {
        // REST API common patterns
        let rest_success = expected_status_codes!(200, 201, 204);
        assert!(rest_success.contains(200)); // GET success
        assert!(rest_success.contains(201)); // POST created
        assert!(rest_success.contains(204)); // DELETE success

        // GraphQL typically uses 200 for everything
        let graphql = expected_status_codes!(200);
        assert!(graphql.contains(200));
        assert!(!graphql.contains(201));

        // Health check endpoints
        let health = expected_status_codes!(200, 503);
        assert!(health.contains(200)); // Healthy
        assert!(health.contains(503)); // Unhealthy

        // Authentication endpoints
        let auth = expected_status_codes!(200, 201, 401, 403);
        assert!(auth.contains(200)); // Login success
        assert!(auth.contains(401)); // Unauthorized
        assert!(auth.contains(403)); // Forbidden
    }

    #[test]
    fn test_expected_status_codes_with_api_call() {
        // This tests that the macro works correctly with actual API calls
        let client = ApiClient::builder().build().unwrap();
        let codes = expected_status_codes!(200 - 299, 404);

        // Should compile and be usable
        let _call = client
            .get("/test")
            .unwrap()
            .with_expected_status_codes(codes);
    }

    #[test]
    fn test_expected_status_codes_method_chaining() {
        let codes = expected_status_codes!(200)
            .add_single(201)
            .add_inclusive_range(300..=304);

        assert!(codes.contains(200));
        assert!(codes.contains(201));
        assert!(codes.contains(302));
    }

    #[test]
    fn test_expected_status_codes_vs_manual_creation() {
        // Macro version
        let macro_version = expected_status_codes!(200 - 204, 301, 400);

        // Manual version
        let manual_version = ExpectedStatusCodes::from_inclusive_range(200..=204)
            .add_single(301)
            .add_single(400);

        // Should behave identically
        for code in 100..600 {
            assert_eq!(
                macro_version.contains(code),
                manual_version.contains(code),
                "Mismatch for status code {code}"
            );
        }
    }
}
