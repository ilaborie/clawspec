//! JSON response redaction support using JSON Pointer (RFC 6901) and JSONPath (RFC 9535).
//!
//! This module provides functionality to redact sensitive or dynamic values
//! in JSON responses for snapshot testing. It allows you to replace or remove
//! values at specific paths while preserving the original data for test assertions.
//!
//! # Path Syntax
//!
//! The path syntax is auto-detected based on the prefix:
//! - Paths starting with `$` use JSONPath (RFC 9535) - supports wildcards
//! - Paths starting with `/` use JSON Pointer (RFC 6901) - exact paths only
//!
//! ## JSONPath Examples (wildcards)
//!
//! - `$.items[*].id` - all `id` fields in the `items` array
//! - `$..id` - all `id` fields anywhere in the document (recursive descent)
//! - `$.users[0:3].email` - `email` in first 3 users
//!
//! ## JSON Pointer Examples (exact paths)
//!
//! - `/id` - top-level `id` field
//! - `/user/email` - nested field
//! - `/items/0/id` - specific array index
//!
//! # Redactor Types
//!
//! The [`redact`](RedactionBuilder::redact) method accepts any type implementing [`Redactor`]:
//!
//! - **Static values**: `&str`, `String`, `serde_json::Value`
//! - **Functions**: `Fn(&str, &Value) -> Value` - transform based on path and/or value
//!
//! # Example
//!
//! ```ignore
//! use clawspec_core::ApiClient;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
//! struct User {
//!     id: String,
//!     name: String,
//!     created_at: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = ApiClient::builder().with_base_path("http://localhost:8080".parse()?).build()?;
//!
//! // Using static values
//! let result = client
//!     .get("/api/users/123")?
//!     .await?
//!     .as_json_redacted::<User>().await?
//!     .redact("/id", "stable-uuid")?
//!     .redact("/created_at", "2024-01-01T00:00:00Z")?
//!     .finish()
//!     .await;
//!
//! // Using JSONPath wildcards with static values
//! let result = client
//!     .get("/api/users")?
//!     .await?
//!     .as_json_redacted::<Vec<User>>().await?
//!     .redact("$[*].id", "redacted-uuid")?
//!     .redact("$[*].created_at", "2024-01-01T00:00:00Z")?
//!     .finish()
//!     .await;
//!
//! // Using closure for index-based IDs
//! let result = client
//!     .get("/api/users")?
//!     .await?
//!     .as_json_redacted::<Vec<User>>().await?
//!     .redact("$[*].id", |path, _val| {
//!         let idx = path.split('/').nth(1).unwrap_or("0");
//!         serde_json::json!(format!("user-{idx}"))
//!     })?
//!     .finish()
//!     .await;
//!
//! // Test assertions use the real value
//! assert!(!result.value.is_empty());
//!
//! // Snapshots use the redacted value (stable ids and timestamps)
//! insta::assert_yaml_snapshot!(result.redacted);
//! # Ok(())
//! # }
//! ```

use std::any::{TypeId, type_name};

use headers::ContentType;
use http::StatusCode;
use jsonptr::{Pointer, assign::Assign, delete::Delete, resolve::Resolve};
use serde::de::DeserializeOwned;
use serde_json::Deserializer;
use utoipa::ToSchema;
use utoipa::openapi::{RefOr, Schema};

use super::output::Output;

mod path_selector;
mod redactor;

use self::path_selector::PathSelector;
pub use self::redactor::Redactor;
use crate::client::CallResult;
use crate::client::error::ApiClientError;
use crate::client::openapi::channel::{CollectorMessage, CollectorSender};
use crate::client::openapi::schema::{SchemaEntry, compute_schema_ref};

impl CallResult {
    /// Deserializes the JSON response and returns a builder for applying redactions.
    ///
    /// This method is similar to [`as_json()`](CallResult::as_json) but returns a
    /// [`RedactionBuilder`](super::redaction::RedactionBuilder) that allows you to apply redactions
    /// to the JSON before finalizing the result.
    ///
    /// The original value is preserved for test assertions, while the redacted
    /// JSON can be used for snapshot testing with stable values.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize into. Must implement [`DeserializeOwned`],
    ///   [`ToSchema`], and have a `'static` lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The response is not JSON
    /// - JSON deserialization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use clawspec_core::ApiClient;
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
    /// # struct User { id: String, name: String }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = ApiClient::builder().with_base_path("http://localhost".parse()?).build()?;
    /// let result = client
    ///     .get("/api/users/123")?
    ///     .await?
    ///     .as_json_redacted::<User>().await?
    ///     .redact("/id", "stable-uuid")?
    ///     .finish()
    ///     .await;
    ///
    /// // Use real value for assertions
    /// assert!(!result.value.id.is_empty());
    ///
    /// // Use redacted value for snapshots
    /// insta::assert_yaml_snapshot!(result.redacted);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_json_redacted<T>(&mut self) -> Result<RedactionBuilder<T>, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
    {
        // Compute schema reference locally (no lock needed)
        let schema = compute_schema_ref::<T>();

        // Register the schema entry via channel
        let entry = SchemaEntry::of::<T>();
        self.collector_sender
            .send(CollectorMessage::AddSchemaEntry(entry))
            .await;

        // Access output directly without calling get_output() to defer response registration
        let Output::Json(json) = self.output() else {
            return Err(ApiClientError::UnsupportedJsonOutput {
                output: self.output().clone(),
                name: type_name::<T>(),
            });
        };

        // Delegate to redaction module with deferred registration data
        // Response will be registered in finish() with the redacted example
        let builder = super::redaction::create_redaction_builder::<T>(
            json,
            self.collector_sender.clone(),
            self.operation_id().to_string(),
            self.status(),
            self.content_type().cloned(),
            schema,
        )?;

        Ok(builder)
    }
}

/// Result of a redacted JSON response containing both the real and redacted values.
///
/// This struct is returned by [`CallResult::as_json_redacted()`] and provides
/// access to both the original deserialized value and the redacted JSON for
/// snapshot testing.
#[derive(Debug, Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "redaction")))]
pub struct RedactedResult<T> {
    /// The real deserialized value for test assertions.
    pub value: T,
    /// The redacted JSON value for snapshot testing.
    pub redacted: serde_json::Value,
}

/// Options for configuring redaction behavior.
///
/// Use this struct with [`RedactionBuilder::redact_with_options`] and
/// [`RedactionBuilder::redact_remove_with`] to customize how redaction
/// handles edge cases.
///
/// # Example
///
/// ```ignore
/// use clawspec_core::RedactOptions;
///
/// // Allow empty matches (useful for optional fields)
/// let options = RedactOptions { allow_empty_match: true };
///
/// builder
///     .redact_with_options("$.optional[*].field", "value", options)?
///     .finish()
///     .await;
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "redaction")))]
pub struct RedactOptions {
    /// If true, matching zero paths is not an error (silent no-op).
    ///
    /// By default (`false`), if a path matches nothing, an error is returned.
    /// This helps catch typos in path expressions. Set to `true` when redacting
    /// optional fields that may not always be present.
    pub allow_empty_match: bool,
}

/// Builder for applying redactions to JSON responses.
///
/// This builder allows you to chain multiple redaction operations before
/// finalizing the result. Paths can use either JSON Pointer (RFC 6901) or
/// JSONPath (RFC 9535) syntax.
///
/// # Path Syntax
///
/// The syntax is auto-detected based on the path prefix:
///
/// ## JSON Pointer (starts with `/`)
///
/// - `/field` - top-level field
/// - `/field/subfield` - nested field
/// - `/array/0` - array index
/// - `/field~1with~1slashes` - `~1` escapes `/`
/// - `/field~0with~0tildes` - `~0` escapes `~`
///
/// ## JSONPath (starts with `$`)
///
/// - `$.field` - top-level field
/// - `$.items[*].id` - all `id` fields in array
/// - `$..id` - all `id` fields anywhere (recursive)
/// - `$[0:3]` - array slice
#[derive(derive_more::Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "redaction")))]
pub struct RedactionBuilder<T> {
    value: T,
    redacted: serde_json::Value,
    #[debug(skip)]
    collector_sender: CollectorSender,
    // Deferred response registration data
    operation_id: String,
    status: StatusCode,
    content_type: Option<ContentType>,
    schema: RefOr<Schema>,
}

impl<T> RedactionBuilder<T> {
    /// Creates a new redaction builder with the original value and JSON.
    pub(in crate::client) fn new(
        value: T,
        json: serde_json::Value,
        collector_sender: CollectorSender,
        operation_id: String,
        status: StatusCode,
        content_type: Option<ContentType>,
        schema: RefOr<Schema>,
    ) -> Self {
        Self {
            value,
            redacted: json,
            collector_sender,
            operation_id,
            status,
            content_type,
            schema,
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
    /// * `path` - Path expression (e.g., `/id`, `$.items[*].id`)
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
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = ApiClient::builder().with_base_path("http://localhost".parse()?).build()?;
    /// // Static value
    /// let result = client
    ///     .get("/api/users/123")?
    ///     .await?
    ///     .as_json_redacted::<serde_json::Value>().await?
    ///     .redact("/id", "test-uuid")?
    ///     .finish()
    ///     .await;
    ///
    /// // Closure for index-based IDs
    /// let result = client
    ///     .get("/api/users")?
    ///     .await?
    ///     .as_json_redacted::<Vec<serde_json::Value>>().await?
    ///     .redact("$[*].id", |path, _val| {
    ///         let idx = path.split('/').nth(1).unwrap_or("0");
    ///         serde_json::json!(format!("user-{idx}"))
    ///     })?
    ///     .finish()
    ///     .await;
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
    /// * `path` - Path expression (e.g., `/id`, `$.items[*].id`)
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
    ///     .redact_with_options("$.optional[*].field", "value", options)?
    ///     .finish()
    ///     .await;
    /// ```
    pub fn redact_with_options<R: Redactor>(
        mut self,
        path: &str,
        redactor: R,
        options: RedactOptions,
    ) -> Result<Self, ApiClientError> {
        // Parse the path (auto-detect JSONPath vs JSON Pointer)
        let selector = PathSelector::parse(path)?;

        // Resolve to concrete JSON Pointer paths
        let concrete_paths = selector.resolve(&self.redacted);

        // Check for empty matches
        if concrete_paths.is_empty() && !options.allow_empty_match {
            return Err(ApiClientError::RedactionError {
                message: format!("Path '{path}' matched no values"),
            });
        }

        // Apply redactor to each matched path
        for pointer in concrete_paths {
            let ptr = Pointer::parse(&pointer).map_err(|e| ApiClientError::RedactionError {
                message: format!("Invalid JSON Pointer '{pointer}': {e}"),
            })?;

            // Get current value
            let current_value =
                self.redacted
                    .resolve(ptr)
                    .map_err(|e| ApiClientError::RedactionError {
                        message: format!("Failed to resolve path '{pointer}': {e}"),
                    })?;

            // Apply redactor transformation
            let new_value = redactor.apply(&pointer, current_value);

            // Assign new value
            self.redacted
                .assign(ptr, new_value)
                .map_err(|e| ApiClientError::RedactionError {
                    message: format!("Failed to assign value at path '{pointer}': {e}"),
                })?;
        }

        Ok(self)
    }

    /// Removes values at the specified path.
    ///
    /// This completely removes the field from objects or the element from arrays,
    /// unlike setting it to `null`.
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
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = ApiClient::builder().with_base_path("http://localhost".parse()?).build()?;
    /// // Remove specific field
    /// let result = client
    ///     .get("/api/users/123")?
    ///     .await?
    ///     .as_json_redacted::<serde_json::Value>().await?
    ///     .redact_remove("/password")?
    ///     .finish()
    ///     .await;
    ///
    /// // Remove field from all array elements
    /// let result = client
    ///     .get("/api/users")?
    ///     .await?
    ///     .as_json_redacted::<Vec<serde_json::Value>>().await?
    ///     .redact_remove("$[*].password")?
    ///     .finish()
    ///     .await;
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
    ///     .redact_remove_with("$.optional[*].field", options)?
    ///     .finish()
    ///     .await;
    /// ```
    pub fn redact_remove_with(
        mut self,
        path: &str,
        options: RedactOptions,
    ) -> Result<Self, ApiClientError> {
        // Parse the path (auto-detect JSONPath vs JSON Pointer)
        let selector = PathSelector::parse(path)?;

        // Resolve to concrete JSON Pointer paths
        let concrete_paths = selector.resolve(&self.redacted);

        // Check for empty matches
        if concrete_paths.is_empty() && !options.allow_empty_match {
            return Err(ApiClientError::RedactionError {
                message: format!("Path '{path}' matched no values"),
            });
        }

        // Apply deletion to each matched path
        for pointer in concrete_paths {
            let ptr = Pointer::parse(&pointer).map_err(|e| ApiClientError::RedactionError {
                message: format!("Invalid JSON Pointer '{pointer}': {e}"),
            })?;

            // Delete returns None if the pointer doesn't exist, which is fine
            let _ = self.redacted.delete(ptr);
        }

        Ok(self)
    }

    /// Finalizes the redaction and returns the result.
    ///
    /// This consumes the builder and returns a [`RedactedResult`] containing
    /// both the original value and the redacted JSON.
    ///
    /// The redacted JSON value is recorded as an example in both the OpenAPI
    /// schema for type `T` and in the response content for this operation.
    pub async fn finish(self) -> RedactedResult<T>
    where
        T: ToSchema + 'static,
    {
        // Add example to schemas via channel
        self.collector_sender
            .send(CollectorMessage::AddExample {
                type_id: TypeId::of::<T>(),
                type_name: type_name::<T>(),
                example: self.redacted.clone(),
            })
            .await;

        // Register response with the redacted example via channel
        self.collector_sender
            .send(CollectorMessage::RegisterResponseWithExample {
                operation_id: self.operation_id.clone(),
                status: self.status,
                content_type: self.content_type.clone(),
                schema: self.schema.clone(),
                example: self.redacted.clone(),
            })
            .await;

        RedactedResult {
            value: self.value,
            redacted: self.redacted,
        }
    }
}

/// Creates a RedactionBuilder from a JSON string.
///
/// This is a helper function used internally by `CallResult::as_json_redacted()`.
/// It deserializes the JSON into the target type and prepares it for redaction.
///
/// # Arguments
///
/// * `json` - The JSON string to deserialize and prepare for redaction
/// * `collector_sender` - The channel sender to record the redacted example to
/// * `operation_id` - The operation ID for deferred response registration
/// * `status` - The HTTP status code of the response
/// * `content_type` - The content type of the response
/// * `schema` - The OpenAPI schema reference for the response type
///
/// # Type Parameters
///
/// * `T` - The type to deserialize into. Must implement [`DeserializeOwned`],
///   [`ToSchema`], and have a `'static` lifetime.
///
/// # Errors
///
/// Returns an error if:
/// - JSON deserialization fails
/// - JSON parsing fails for the redaction copy
pub(crate) fn create_redaction_builder<T>(
    json: &str,
    collector_sender: CollectorSender,
    operation_id: String,
    status: StatusCode,
    content_type: Option<ContentType>,
    schema: RefOr<Schema>,
) -> Result<RedactionBuilder<T>, ApiClientError>
where
    T: DeserializeOwned + ToSchema + 'static,
{
    // Deserialize the original value
    let deserializer = &mut Deserializer::from_str(json);
    let value: T = serde_path_to_error::deserialize(deserializer).map_err(|err| {
        ApiClientError::JsonError {
            path: err.path().to_string(),
            error: err.into_inner(),
            body: json.to_string(),
        }
    })?;

    // Parse JSON for redaction
    let json_value = serde_json::from_str::<serde_json::Value>(json).map_err(|error| {
        ApiClientError::JsonError {
            path: String::new(),
            error,
            body: json.to_string(),
        }
    })?;

    Ok(RedactionBuilder::new(
        value,
        json_value,
        collector_sender,
        operation_id,
        status,
        content_type,
        schema,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use utoipa::openapi::RefOr;

    #[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
    struct TestStruct {
        id: String,
        name: String,
        items: Vec<TestItem>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
    struct TestItem {
        id: String,
        value: i32,
    }

    fn create_test_builder() -> RedactionBuilder<TestStruct> {
        let value = TestStruct {
            id: "real-uuid".to_string(),
            name: "Test".to_string(),
            items: vec![
                TestItem {
                    id: "item-1".to_string(),
                    value: 10,
                },
                TestItem {
                    id: "item-2".to_string(),
                    value: 20,
                },
            ],
        };

        let json = json!({
            "id": "real-uuid",
            "name": "Test",
            "items": [
                {"id": "item-1", "value": 10},
                {"id": "item-2", "value": 20}
            ]
        });

        let sender = CollectorSender::dummy();
        let schema = RefOr::T(utoipa::openapi::ObjectBuilder::new().build().into());

        RedactionBuilder::new(
            value,
            json,
            sender,
            "test_op".to_string(),
            StatusCode::OK,
            None,
            schema,
        )
    }

    #[test]
    fn test_redact_options_default() {
        let options = RedactOptions::default();

        assert!(!options.allow_empty_match);
    }

    #[test]
    fn test_redact_options_allow_empty_match() {
        let options = RedactOptions {
            allow_empty_match: true,
        };

        assert!(options.allow_empty_match);
    }

    #[test]
    fn test_redact_options_debug() {
        let options = RedactOptions {
            allow_empty_match: true,
        };
        let debug_str = format!("{options:?}");

        assert!(debug_str.contains("RedactOptions"));
        assert!(debug_str.contains("allow_empty_match"));
    }

    #[test]
    fn test_redact_options_clone() {
        let original = RedactOptions {
            allow_empty_match: true,
        };
        let cloned = original.clone();

        assert_eq!(original.allow_empty_match, cloned.allow_empty_match);
    }

    #[test]
    fn test_redaction_builder_redact_success() {
        let builder = create_test_builder();
        let result = builder.redact("/id", "stable-uuid");

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        assert_eq!(
            builder.redacted.get("id").and_then(|v| v.as_str()),
            Some("stable-uuid")
        );
    }

    #[test]
    fn test_redaction_builder_redact_jsonpath_wildcard() {
        let builder = create_test_builder();
        let result = builder.redact("$.items[*].id", "redacted-id");

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        let items = builder.redacted.get("items").expect("should have items");
        let items_array = items.as_array().expect("should be array");

        for item in items_array {
            assert_eq!(item.get("id").and_then(|v| v.as_str()), Some("redacted-id"));
        }
    }

    #[test]
    fn test_redaction_builder_redact_with_closure() {
        let builder = create_test_builder();
        let result = builder.redact("$.items[*].id", |path: &str, _val: &serde_json::Value| {
            let idx = path.split('/').nth(2).unwrap_or("?");
            json!(format!("item-idx-{idx}"))
        });

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        let items = builder.redacted.get("items").expect("should have items");
        let items_array = items.as_array().expect("should be array");

        assert_eq!(
            items_array[0].get("id").and_then(|v| v.as_str()),
            Some("item-idx-0")
        );
        assert_eq!(
            items_array[1].get("id").and_then(|v| v.as_str()),
            Some("item-idx-1")
        );
    }

    #[test]
    fn test_redaction_builder_redact_error_no_match() {
        let builder = create_test_builder();
        let result = builder.redact("$.nonexistent", "value");

        assert!(result.is_err());
    }

    #[test]
    fn test_redaction_builder_redact_with_options_allow_empty() {
        let builder = create_test_builder();
        let options = RedactOptions {
            allow_empty_match: true,
        };
        let result = builder.redact_with_options("$.nonexistent", "value", options);

        assert!(result.is_ok());
    }

    #[test]
    fn test_redaction_builder_redact_remove_success() {
        let builder = create_test_builder();
        let result = builder.redact_remove("/id");

        assert!(result.is_ok());
        let builder = result.expect("removal should succeed");
        assert!(builder.redacted.get("id").is_none());
    }

    #[test]
    fn test_redaction_builder_redact_remove_jsonpath() {
        let builder = create_test_builder();
        let result = builder.redact_remove("$.items[*].id");

        assert!(result.is_ok());
        let builder = result.expect("removal should succeed");
        let items = builder.redacted.get("items").expect("should have items");
        let items_array = items.as_array().expect("should be array");

        for item in items_array {
            assert!(item.get("id").is_none());
        }
    }

    #[test]
    fn test_redaction_builder_redact_remove_error_no_match() {
        let builder = create_test_builder();
        let result = builder.redact_remove("$.nonexistent");

        assert!(result.is_err());
    }

    #[test]
    fn test_redaction_builder_redact_remove_with_allow_empty() {
        let builder = create_test_builder();
        let options = RedactOptions {
            allow_empty_match: true,
        };
        let result = builder.redact_remove_with("$.nonexistent", options);

        assert!(result.is_ok());
    }

    #[test]
    fn test_redaction_builder_chained_redactions() {
        let builder = create_test_builder();
        let result = builder
            .redact("/id", "stable-id")
            .and_then(|b| b.redact("/name", "Redacted Name"))
            .and_then(|b| b.redact("$.items[*].id", "item-redacted"));

        assert!(result.is_ok());
        let builder = result.expect("chained redactions should succeed");
        assert_eq!(
            builder.redacted.get("id").and_then(|v| v.as_str()),
            Some("stable-id")
        );
        assert_eq!(
            builder.redacted.get("name").and_then(|v| v.as_str()),
            Some("Redacted Name")
        );
    }

    #[test]
    fn test_redaction_builder_preserves_original_value() {
        let builder = create_test_builder();
        let original_id = builder.value.id.clone();
        let result = builder.redact("/id", "stable-id");

        assert!(result.is_ok());
        let builder = result.expect("redaction should succeed");
        // Original value should be preserved
        assert_eq!(builder.value.id, original_id);
        // Redacted value should be different
        assert_eq!(
            builder.redacted.get("id").and_then(|v| v.as_str()),
            Some("stable-id")
        );
    }

    #[test]
    fn test_redacted_result_fields() {
        let result = RedactedResult {
            value: TestStruct {
                id: "real".to_string(),
                name: "Test".to_string(),
                items: vec![],
            },
            redacted: json!({"id": "fake", "name": "Test", "items": []}),
        };

        assert_eq!(result.value.id, "real");
        assert_eq!(
            result.redacted.get("id").and_then(|v| v.as_str()),
            Some("fake")
        );
    }

    #[test]
    fn test_redacted_result_debug() {
        let result = RedactedResult {
            value: "test".to_string(),
            redacted: json!("redacted"),
        };
        let debug_str = format!("{result:?}");

        assert!(debug_str.contains("RedactedResult"));
    }

    #[test]
    fn test_redacted_result_clone() {
        let original = RedactedResult {
            value: "test".to_string(),
            redacted: json!("redacted"),
        };
        let cloned = original.clone();

        assert_eq!(original.value, cloned.value);
        assert_eq!(original.redacted, cloned.redacted);
    }

    #[test]
    fn test_redaction_builder_debug() {
        let builder = create_test_builder();
        let debug_str = format!("{builder:?}");

        assert!(debug_str.contains("RedactionBuilder"));
        assert!(debug_str.contains("operation_id"));
    }

    #[test]
    fn test_create_redaction_builder_success() {
        let json = r#"{"id": "uuid", "name": "Test"}"#;
        let sender = CollectorSender::dummy();
        let schema = RefOr::T(utoipa::openapi::ObjectBuilder::new().build().into());

        let result = create_redaction_builder::<serde_json::Value>(
            json,
            sender,
            "op_id".to_string(),
            StatusCode::OK,
            None,
            schema,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_create_redaction_builder_invalid_json() {
        let json = r#"{"invalid json"#;
        let sender = CollectorSender::dummy();
        let schema = RefOr::T(utoipa::openapi::ObjectBuilder::new().build().into());

        let result = create_redaction_builder::<serde_json::Value>(
            json,
            sender,
            "op_id".to_string(),
            StatusCode::OK,
            None,
            schema,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_path_syntax_error() {
        let builder = create_test_builder();
        // Path without $ or / prefix should fail
        let result = builder.redact("invalid_path", "value");

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_jsonpath_syntax() {
        let builder = create_test_builder();
        // Invalid JSONPath syntax
        let result = builder.redact("$[[[invalid", "value");

        assert!(result.is_err());
    }
}
