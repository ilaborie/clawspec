//! Request body redaction support for OpenAPI documentation.
//!
//! This module provides functionality to redact sensitive values in JSON request bodies
//! before they are stored as examples in the OpenAPI specification. The key principle is:
//!
//! - **Original value for HTTP**: The actual serialized data is sent in the HTTP request
//! - **Redacted value for OpenAPI**: The redacted value is used for documentation examples
//!
//! This allows you to test with real data while keeping your OpenAPI examples clean,
//! stable, and free of sensitive information.
//!
//! # Path Syntax
//!
//! The path syntax is auto-detected based on the prefix:
//! - Paths starting with `$` use JSONPath (RFC 9535) - supports wildcards
//! - Paths starting with `/` use JSON Pointer (RFC 6901) - exact paths only
//!
//! # Example
//!
//! ```ignore
//! use clawspec_core::ApiClient;
//! use serde::Serialize;
//! use utoipa::ToSchema;
//!
//! #[derive(Clone, Serialize, ToSchema)]
//! struct CreateUser {
//!     username: String,
//!     password: String,
//!     api_key: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ApiClient::builder().build()?;
//!
//! let user = CreateUser {
//!     username: "alice".to_string(),
//!     password: "secret123".to_string(),
//!     api_key: "sk-live-abc123".to_string(),
//! };
//!
//! // The HTTP request will contain the real password and API key,
//! // but the OpenAPI example will show the redacted values.
//! client
//!     .post("/users")?
//!     .json_redacted(&user)?
//!     .redact("/password", "[REDACTED]")?
//!     .redact("/api_key", "[REDACTED]")?
//!     .await?;  // IntoFuture - no .finish() needed
//! # Ok(())
//! # }
//! ```

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use utoipa::ToSchema;

use super::RedactOptions;
use super::apply::{apply_redaction, apply_remove};
use super::redactor::Redactor;
use crate::client::call::ApiCall;
use crate::client::error::ApiClientError;
use crate::client::{CallBody, CallResult};

/// Builder for redacting sensitive values in JSON request bodies.
///
/// This builder allows you to apply redactions to a JSON request body before
/// it's used in the OpenAPI documentation. The original (unredacted) value
/// is sent in the actual HTTP request.
///
/// # Key Principle
///
/// - **HTTP Request**: Uses the original value with real data for testing
/// - **OpenAPI Example**: Uses the redacted value with stable placeholders
///
/// This separation allows you to:
/// - Test with realistic data (passwords, tokens, API keys)
/// - Generate stable OpenAPI documentation (no dynamic values)
/// - Hide sensitive information from documentation
///
/// # Path Syntax
///
/// Paths are auto-detected based on their prefix:
/// - `/...` → JSON Pointer (RFC 6901) for exact paths
/// - `$...` → JSONPath (RFC 9535) for wildcards
///
/// # Example
///
/// ```ignore
/// use clawspec_core::ApiClient;
/// use serde::Serialize;
/// use utoipa::ToSchema;
///
/// #[derive(Clone, Serialize, ToSchema)]
/// struct LoginRequest {
///     email: String,
///     password: String,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
///
/// let request = LoginRequest {
///     email: "user@example.com".to_string(),
///     password: "my-secret-password".to_string(),
/// };
///
/// client
///     .post("/auth/login")?
///     .json_redacted(&request)?
///     .redact("/password", "[REDACTED]")?
///     .await?;  // IntoFuture - no .finish() needed
/// # Ok(())
/// # }
/// ```
#[derive(derive_more::Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "redaction")))]
pub struct RequestBodyRedactionBuilder<T> {
    /// The original value (kept for reference, used in HTTP request via body.data)
    #[debug(skip)]
    value: T,
    /// The JSON representation for redaction operations
    redacted: serde_json::Value,
    /// The body being built (contains serialized data for HTTP)
    body: CallBody,
    /// The ApiCall to return when finished
    #[debug(skip)]
    api_call: ApiCall,
}

impl<T> RequestBodyRedactionBuilder<T> {
    /// Creates a new request body redaction builder.
    pub(crate) fn new(
        value: T,
        redacted: serde_json::Value,
        body: CallBody,
        api_call: ApiCall,
    ) -> Self {
        Self {
            value,
            redacted,
            body,
            api_call,
        }
    }

    /// Redacts values at the specified path using a redactor.
    ///
    /// The path can be either JSON Pointer (RFC 6901) or JSONPath (RFC 9535).
    /// The syntax is auto-detected based on the prefix:
    /// - `$...` → JSONPath (supports wildcards)
    /// - `/...` → JSON Pointer (exact path)
    ///
    /// The redactor can be:
    /// - A static value: `"replacement"` or `serde_json::json!(...)`
    /// - A closure: `|path, val| transform(path, val)`
    ///
    /// # Arguments
    ///
    /// * `path` - Path expression (e.g., `/password`, `$.users[*].token`)
    /// * `redactor` - The redactor to apply (static value or closure)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is invalid
    /// - The path matches no values
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use clawspec_core::ApiClient;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # #[derive(Clone, Serialize, ToSchema)]
    /// # struct Request { token: String }
    /// # async fn example(client: &mut ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    /// // Static value
    /// client.post("/api")?
    ///     .json_redacted(&Request { token: "secret".into() })?
    ///     .redact("/token", "[REDACTED]")?
    ///     .await?;  // IntoFuture - no .finish() needed
    /// # Ok(())
    /// # }
    /// ```
    pub fn redact<R: Redactor>(self, path: &str, redactor: R) -> Result<Self, ApiClientError> {
        self.redact_with_options(path, redactor, RedactOptions::default())
    }

    /// Redacts values at the specified path with configurable options.
    ///
    /// This is like [`redact`](Self::redact) but allows customizing
    /// behavior through [`RedactOptions`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path expression (e.g., `/password`, `$.users[*].token`)
    /// * `redactor` - The redactor to apply
    /// * `options` - Configuration options
    ///
    /// # Example
    ///
    /// ```ignore
    /// use clawspec_core::RedactOptions;
    ///
    /// // Allow empty matches for optional fields
    /// let options = RedactOptions { allow_empty_match: true };
    ///
    /// builder
    ///     .redact_with_options("$.optional_field", "value", options)?
    ///     .await?;  // IntoFuture - no .finish() needed
    /// ```
    pub fn redact_with_options<R: Redactor>(
        mut self,
        path: &str,
        redactor: R,
        options: RedactOptions,
    ) -> Result<Self, ApiClientError> {
        apply_redaction(&mut self.redacted, path, redactor, options)?;
        Ok(self)
    }

    /// Removes values at the specified path from the OpenAPI example.
    ///
    /// This completely removes the field from the OpenAPI documentation example,
    /// unlike setting it to `null`. The original value is still sent in the HTTP request.
    ///
    /// The path can be either JSON Pointer (RFC 6901) or JSONPath (RFC 9535).
    ///
    /// # Arguments
    ///
    /// * `path` - Path expression to remove
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is invalid
    /// - The path matches no values
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use clawspec_core::ApiClient;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # #[derive(Clone, Serialize, ToSchema)]
    /// # struct Request { password: String, internal_id: String }
    /// # async fn example(client: &mut ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    /// client.post("/api")?
    ///     .json_redacted(&Request {
    ///         password: "secret".into(),
    ///         internal_id: "internal-123".into(),
    ///     })?
    ///     .redact("/password", "[REDACTED]")?
    ///     .redact_remove("/internal_id")?  // Remove entirely from docs
    ///     .await?;  // IntoFuture - no .finish() needed
    /// # Ok(())
    /// # }
    /// ```
    pub fn redact_remove(self, path: &str) -> Result<Self, ApiClientError> {
        self.redact_remove_with(path, RedactOptions::default())
    }

    /// Removes values at the specified path with configurable options.
    ///
    /// This is like [`redact_remove`](Self::redact_remove) but allows customizing
    /// behavior through [`RedactOptions`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path expression to remove
    /// * `options` - Configuration options
    ///
    /// # Example
    ///
    /// ```ignore
    /// use clawspec_core::RedactOptions;
    ///
    /// // Allow empty matches for optional fields
    /// let options = RedactOptions { allow_empty_match: true };
    ///
    /// builder
    ///     .redact_remove_with("$.optional_field", options)?
    ///     .await?;  // IntoFuture - no .finish() needed
    /// ```
    pub fn redact_remove_with(
        mut self,
        path: &str,
        options: RedactOptions,
    ) -> Result<Self, ApiClientError> {
        apply_remove(&mut self.redacted, path, options)?;
        Ok(self)
    }

    /// Finalizes the redaction and returns the configured ApiCall.
    ///
    /// This consumes the builder and returns the `ApiCall` with the request body
    /// configured. The body will contain:
    /// - **HTTP data**: The original (unredacted) serialized value
    /// - **OpenAPI example**: The redacted value for documentation
    ///
    /// After calling `finish()`, you can `.await` the `ApiCall` to execute
    /// the HTTP request.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use clawspec_core::ApiClient;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # #[derive(Clone, Serialize, ToSchema)]
    /// # struct Request { password: String }
    /// # async fn example(client: &mut ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let response = client
    ///     .post("/api")?
    ///     .json_redacted(&Request { password: "secret".into() })?
    ///     .redact("/password", "[REDACTED]")?
    ///     .finish()?  // Returns ApiCall
    ///     .await?;    // Executes the HTTP request
    /// # Ok(())
    /// # }
    /// ```
    pub fn finish(mut self) -> Result<ApiCall, ApiClientError>
    where
        T: ToSchema + 'static,
    {
        // Set the redacted example on the body
        self.body.set_example(self.redacted);

        // Set the body on the ApiCall
        self.api_call.body = Some(self.body);

        Ok(self.api_call)
    }

    /// Returns a reference to the original (unredacted) value.
    ///
    /// This can be useful if you need to inspect the original value
    /// while building the redactions.
    pub fn original_value(&self) -> &T {
        &self.value
    }

    /// Returns a reference to the current redacted JSON value.
    ///
    /// This can be useful if you need to inspect the redacted state
    /// while building the redactions.
    pub fn redacted_value(&self) -> &serde_json::Value {
        &self.redacted
    }
}

/// Implements `IntoFuture` to allow direct `.await` on the builder.
///
/// This enables a more ergonomic API where you can write:
///
/// ```ignore
/// client
///     .post("/users")?
///     .json_redacted(&user)?
///     .redact("/password", "[REDACTED]")?
///     .await?;  // No need for .finish()?
/// ```
///
/// Instead of:
///
/// ```ignore
/// client
///     .post("/users")?
///     .json_redacted(&user)?
///     .redact("/password", "[REDACTED]")?
///     .finish()?
///     .await?;
/// ```
impl<T> IntoFuture for RequestBodyRedactionBuilder<T>
where
    T: ToSchema + 'static,
{
    type Output = Result<CallResult, ApiClientError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        // Call finish() synchronously to avoid capturing self in the async block
        match self.finish() {
            Ok(api_call) => api_call.into_future(),
            Err(e) => Box::pin(async move { Err(e) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use utoipa::ToSchema;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    struct TestRequest {
        username: String,
        password: String,
        api_key: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    struct NestedRequest {
        user: UserInfo,
        items: Vec<Item>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    struct UserInfo {
        id: String,
        token: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    struct Item {
        id: String,
        secret: String,
    }

    fn create_test_builder() -> RequestBodyRedactionBuilder<TestRequest> {
        let value = TestRequest {
            username: "alice".to_string(),
            password: "secret123".to_string(),
            api_key: "sk-live-abc123".to_string(),
        };

        let redacted = json!({
            "username": "alice",
            "password": "secret123",
            "api_key": "sk-live-abc123"
        });

        let body = CallBody::json_without_example(&value).expect("should create body");

        // Create a minimal ApiCall for testing
        let client = reqwest::Client::new();
        let base_uri = "http://localhost:8080".parse().expect("valid URI");
        let collector_sender = crate::client::openapi::channel::CollectorSender::dummy();
        let path = crate::client::CallPath::from("/test");
        let query = crate::client::CallQuery::new();
        let expected_status_codes = crate::client::response::ExpectedStatusCodes::default();
        let metadata = crate::client::call_parameters::OperationMetadata::default();

        let api_call = ApiCall {
            client,
            base_uri,
            collector_sender,
            method: http::Method::POST,
            path,
            query,
            headers: None,
            body: None,
            authentication: None,
            cookies: None,
            expected_status_codes,
            metadata,
            response_description: None,
            skip_collection: false,
            security: None,
        };

        RequestBodyRedactionBuilder::new(value.clone(), redacted, body, api_call)
    }

    #[test]
    fn should_redact_single_field() {
        let builder = create_test_builder();
        let result = builder.redact("/password", "[REDACTED]");

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        assert_eq!(
            builder.redacted.get("password").and_then(|v| v.as_str()),
            Some("[REDACTED]")
        );
        // Other fields unchanged
        assert_eq!(
            builder.redacted.get("username").and_then(|v| v.as_str()),
            Some("alice")
        );
        assert_eq!(
            builder.redacted.get("api_key").and_then(|v| v.as_str()),
            Some("sk-live-abc123")
        );
    }

    #[test]
    fn should_redact_multiple_fields() {
        let builder = create_test_builder();
        let result = builder
            .redact("/password", "[REDACTED]")
            .and_then(|b| b.redact("/api_key", "[REDACTED]"));

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        assert_eq!(
            builder.redacted.get("password").and_then(|v| v.as_str()),
            Some("[REDACTED]")
        );
        assert_eq!(
            builder.redacted.get("api_key").and_then(|v| v.as_str()),
            Some("[REDACTED]")
        );
        // Username unchanged
        assert_eq!(
            builder.redacted.get("username").and_then(|v| v.as_str()),
            Some("alice")
        );
    }

    #[test]
    fn should_redact_with_jsonpath_wildcards() {
        let value = NestedRequest {
            user: UserInfo {
                id: "user-123".to_string(),
                token: "token-abc".to_string(),
            },
            items: vec![
                Item {
                    id: "item-1".to_string(),
                    secret: "secret-1".to_string(),
                },
                Item {
                    id: "item-2".to_string(),
                    secret: "secret-2".to_string(),
                },
            ],
        };

        let redacted = serde_json::to_value(&value).expect("should serialize");
        let body = CallBody::json_without_example(&value).expect("should create body");

        let client = reqwest::Client::new();
        let base_uri = "http://localhost:8080".parse().expect("valid URI");
        let collector_sender = crate::client::openapi::channel::CollectorSender::dummy();
        let path = crate::client::CallPath::from("/test");
        let query = crate::client::CallQuery::new();
        let expected_status_codes = crate::client::response::ExpectedStatusCodes::default();
        let metadata = crate::client::call_parameters::OperationMetadata::default();

        let api_call = ApiCall {
            client,
            base_uri,
            collector_sender,
            method: http::Method::POST,
            path,
            query,
            headers: None,
            body: None,
            authentication: None,
            cookies: None,
            expected_status_codes,
            metadata,
            response_description: None,
            skip_collection: false,
            security: None,
        };

        let builder: RequestBodyRedactionBuilder<NestedRequest> =
            RequestBodyRedactionBuilder::new(value, redacted, body, api_call);

        let result = builder.redact("$.items[*].secret", "[REDACTED]");

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        let items = builder
            .redacted
            .get("items")
            .and_then(|v| v.as_array())
            .expect("should have items");

        for item in items {
            assert_eq!(
                item.get("secret").and_then(|v| v.as_str()),
                Some("[REDACTED]")
            );
        }
    }

    #[test]
    fn should_redact_with_closure() {
        let builder = create_test_builder();
        let result = builder.redact("/password", |_path: &str, _val: &serde_json::Value| {
            json!("redacted-by-closure")
        });

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        assert_eq!(
            builder.redacted.get("password").and_then(|v| v.as_str()),
            Some("redacted-by-closure")
        );
    }

    #[test]
    fn should_remove_fields() {
        let builder = create_test_builder();
        let result = builder.redact_remove("/password");

        assert!(result.is_ok());
        let builder = result.expect("removal should succeed");
        assert!(builder.redacted.get("password").is_none());
        // Other fields still present
        assert!(builder.redacted.get("username").is_some());
        assert!(builder.redacted.get("api_key").is_some());
    }

    #[test]
    fn should_preserve_original_value() {
        let builder = create_test_builder();
        let original_password = builder.value.password.clone();

        let result = builder.redact("/password", "[REDACTED]");
        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");

        // Original value is preserved
        assert_eq!(builder.value.password, original_password);
        // Redacted value is different
        assert_eq!(
            builder.redacted.get("password").and_then(|v| v.as_str()),
            Some("[REDACTED]")
        );
    }

    #[test]
    fn should_fail_on_invalid_path() {
        let builder = create_test_builder();
        let result = builder.redact("$.nonexistent", "[REDACTED]");

        assert!(result.is_err());
    }

    #[test]
    fn should_allow_empty_match_with_option() {
        let builder = create_test_builder();
        let options = RedactOptions {
            allow_empty_match: true,
        };
        let result = builder.redact_with_options("$.nonexistent", "[REDACTED]", options);

        assert!(result.is_ok());
    }

    #[test]
    fn should_access_original_and_redacted_values() {
        let builder = create_test_builder();

        // Check original value access
        assert_eq!(builder.original_value().username, "alice");
        assert_eq!(builder.original_value().password, "secret123");

        // Check redacted value access before redaction
        assert_eq!(
            builder
                .redacted_value()
                .get("password")
                .and_then(|v| v.as_str()),
            Some("secret123")
        );

        // Redact and check again
        let builder = builder
            .redact("/password", "[REDACTED]")
            .expect("should redact");
        assert_eq!(
            builder
                .redacted_value()
                .get("password")
                .and_then(|v| v.as_str()),
            Some("[REDACTED]")
        );
        // Original unchanged
        assert_eq!(builder.original_value().password, "secret123");
    }

    #[test]
    fn should_finish_and_return_api_call() {
        let builder = create_test_builder();
        let result = builder
            .redact("/password", "[REDACTED]")
            .and_then(|b| b.finish());

        assert!(result.is_ok());
        let api_call = result.expect("should finish");
        assert!(api_call.body.is_some());
    }

    #[test]
    fn test_debug_impl() {
        let builder = create_test_builder();
        let debug_str = format!("{builder:?}");

        assert!(debug_str.contains("RequestBodyRedactionBuilder"));
    }
}
