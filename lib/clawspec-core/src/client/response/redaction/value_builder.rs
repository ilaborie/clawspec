//! Builder for redacting arbitrary JSON values.
//!
//! This module provides [`ValueRedactionBuilder`] for applying redactions to any
//! `serde_json::Value`, independent of HTTP response handling. This is useful for:
//!
//! - Stabilizing dynamic values in generated OpenAPI specifications
//! - Post-processing JSON before writing to files
//! - Applying consistent redaction patterns across different contexts
//!
//! # Example
//!
//! ```rust
//! use clawspec_core::redact_value;
//! use serde_json::json;
//!
//! let value = json!({
//!     "id": "550e8400-e29b-41d4-a716-446655440000",
//!     "created_at": "2024-12-28T10:30:00Z",
//!     "items": [
//!         {"entity_id": "uuid-1"},
//!         {"entity_id": "uuid-2"}
//!     ]
//! });
//!
//! let redacted = redact_value(value)
//!     .redact("/id", "ENTITY_ID").unwrap()
//!     .redact("/created_at", "TIMESTAMP").unwrap()
//!     .redact("$.items[*].entity_id", "NESTED_ID").unwrap()
//!     .finish();
//!
//! assert_eq!(redacted["id"], "ENTITY_ID");
//! assert_eq!(redacted["created_at"], "TIMESTAMP");
//! assert_eq!(redacted["items"][0]["entity_id"], "NESTED_ID");
//! ```

use serde_json::Value;

use super::RedactOptions;
use super::apply::{apply_redaction, apply_remove};
use super::redactor::Redactor;
use crate::client::error::ApiClientError;

/// Builder for redacting arbitrary JSON values.
///
/// Unlike [`RedactionBuilder`](super::RedactionBuilder), this builder works with any
/// `serde_json::Value` without requiring HTTP response context or OpenAPI collection.
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
///
/// # Example
///
/// ```rust
/// use clawspec_core::redact_value;
/// use serde_json::json;
///
/// let openapi_json = json!({
///     "paths": {
///         "/users": {
///             "get": {
///                 "responses": {
///                     "200": {
///                         "content": {
///                             "application/json": {
///                                 "example": {
///                                     "id": "real-uuid",
///                                     "created_at": "2024-12-28T10:30:00Z"
///                                 }
///                             }
///                         }
///                     }
///                 }
///             }
///         }
///     }
/// });
///
/// let stabilized = redact_value(openapi_json)
///     .redact("$..example.id", "ENTITY_ID").unwrap()
///     .redact("$..example.created_at", "TIMESTAMP").unwrap()
///     .finish();
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "redaction")))]
pub struct ValueRedactionBuilder {
    value: Value,
}

impl ValueRedactionBuilder {
    /// Create a new builder for the given JSON value.
    pub fn new(value: Value) -> Self {
        Self { value }
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
    /// ```rust
    /// use clawspec_core::redact_value;
    /// use serde_json::json;
    ///
    /// let value = json!({"id": "uuid-123", "name": "Test"});
    ///
    /// // Static value
    /// let redacted = redact_value(value)
    ///     .redact("/id", "stable-uuid").unwrap()
    ///     .finish();
    ///
    /// assert_eq!(redacted["id"], "stable-uuid");
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
    /// ```rust
    /// use clawspec_core::{redact_value, RedactOptions};
    /// use serde_json::json;
    ///
    /// let value = json!({"id": "test"});
    ///
    /// // Allow empty matches for optional fields
    /// let options = RedactOptions { allow_empty_match: true };
    ///
    /// let redacted = redact_value(value)
    ///     .redact_with_options("$.optional_field", "value", options).unwrap()
    ///     .finish();
    /// ```
    pub fn redact_with_options<R: Redactor>(
        mut self,
        path: &str,
        redactor: R,
        options: RedactOptions,
    ) -> Result<Self, ApiClientError> {
        apply_redaction(&mut self.value, path, redactor, options)?;
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
    /// ```rust
    /// use clawspec_core::redact_value;
    /// use serde_json::json;
    ///
    /// let value = json!({"id": "keep", "secret": "remove"});
    ///
    /// let redacted = redact_value(value)
    ///     .redact_remove("/secret").unwrap()
    ///     .finish();
    ///
    /// assert!(redacted.get("secret").is_none());
    /// assert_eq!(redacted["id"], "keep");
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
    /// ```rust
    /// use clawspec_core::{redact_value, RedactOptions};
    /// use serde_json::json;
    ///
    /// let value = json!({"id": "test"});
    ///
    /// // Allow empty matches for optional fields
    /// let options = RedactOptions { allow_empty_match: true };
    ///
    /// let redacted = redact_value(value)
    ///     .redact_remove_with("$.optional_field", options).unwrap()
    ///     .finish();
    /// ```
    pub fn redact_remove_with(
        mut self,
        path: &str,
        options: RedactOptions,
    ) -> Result<Self, ApiClientError> {
        apply_remove(&mut self.value, path, options)?;
        Ok(self)
    }

    /// Finalize and return the redacted value.
    ///
    /// This consumes the builder and returns the modified JSON value.
    pub fn finish(self) -> Value {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn should_redact_with_json_pointer() {
        let value = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "Test"
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact("/id", "REDACTED_ID")
            .expect("redaction should succeed")
            .finish();

        assert_eq!(redacted["id"], "REDACTED_ID");
        assert_eq!(redacted["name"], "Test");
    }

    #[test]
    fn should_redact_with_jsonpath_wildcards() {
        let value = json!({
            "items": [
                {"id": "uuid-1", "name": "Item 1"},
                {"id": "uuid-2", "name": "Item 2"}
            ]
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact("$.items[*].id", "REDACTED")
            .expect("redaction should succeed")
            .finish();

        let items = redacted["items"].as_array().expect("should be array");
        assert_eq!(items[0]["id"], "REDACTED");
        assert_eq!(items[0]["name"], "Item 1");
        assert_eq!(items[1]["id"], "REDACTED");
        assert_eq!(items[1]["name"], "Item 2");
    }

    #[test]
    fn should_redact_with_recursive_descent() {
        let value = json!({
            "id": "root-uuid",
            "nested": {
                "id": "nested-uuid",
                "deep": {
                    "id": "deep-uuid"
                }
            }
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact("$..id", "REDACTED")
            .expect("redaction should succeed")
            .finish();

        assert_eq!(redacted["id"], "REDACTED");
        assert_eq!(redacted["nested"]["id"], "REDACTED");
        assert_eq!(redacted["nested"]["deep"]["id"], "REDACTED");
    }

    #[test]
    fn should_redact_with_closure() {
        let value = json!({
            "price": 19.99,
            "tax": 1.234567
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact("$.*", |_path: &str, val: &Value| {
                if let Some(n) = val.as_f64() {
                    json!((n * 100.0).round() / 100.0)
                } else {
                    val.clone()
                }
            })
            .expect("redaction should succeed")
            .finish();

        assert_eq!(redacted["price"], 19.99);
        assert_eq!(redacted["tax"], 1.23);
    }

    #[test]
    fn should_redact_with_index_based_closure() {
        let value = json!({
            "items": [
                {"id": "uuid-a"},
                {"id": "uuid-b"},
                {"id": "uuid-c"}
            ]
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact("$.items[*].id", |path: &str, _val: &Value| {
                let idx = path.split('/').nth(2).unwrap_or("?");
                json!(format!("item-{idx}"))
            })
            .expect("redaction should succeed")
            .finish();

        let items = redacted["items"].as_array().expect("should be array");
        assert_eq!(items[0]["id"], "item-0");
        assert_eq!(items[1]["id"], "item-1");
        assert_eq!(items[2]["id"], "item-2");
    }

    #[test]
    fn should_chain_multiple_redactions() {
        let value = json!({
            "entity_id": "uuid-123",
            "created_at": "2024-12-28T10:30:00Z",
            "nested": {
                "entity_id": "uuid-456"
            }
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact("$..entity_id", "ENTITY_ID")
            .expect("redaction should succeed")
            .redact("$..created_at", "TIMESTAMP")
            .expect("redaction should succeed")
            .finish();

        assert_eq!(redacted["entity_id"], "ENTITY_ID");
        assert_eq!(redacted["created_at"], "TIMESTAMP");
        assert_eq!(redacted["nested"]["entity_id"], "ENTITY_ID");
    }

    #[test]
    fn should_handle_remove() {
        let value = json!({
            "id": "keep-this",
            "secret": "remove-this"
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact_remove("/secret")
            .expect("removal should succeed")
            .finish();

        assert_eq!(redacted["id"], "keep-this");
        assert!(redacted.get("secret").is_none());
    }

    #[test]
    fn should_handle_remove_with_jsonpath() {
        let value = json!({
            "items": [
                {"id": "a", "secret": "x"},
                {"id": "b", "secret": "y"}
            ]
        });

        let redacted = ValueRedactionBuilder::new(value)
            .redact_remove("$.items[*].secret")
            .expect("removal should succeed")
            .finish();

        let items = redacted["items"].as_array().expect("should be array");
        assert_eq!(items[0]["id"], "a");
        assert!(items[0].get("secret").is_none());
        assert_eq!(items[1]["id"], "b");
        assert!(items[1].get("secret").is_none());
    }

    #[test]
    fn should_fail_on_no_match_by_default() {
        let value = json!({"id": "test"});

        let err = ValueRedactionBuilder::new(value)
            .redact("/nonexistent", "REDACTED")
            .expect_err("should fail for missing path");

        assert!(matches!(err, ApiClientError::RedactionError { .. }));
    }

    #[test]
    fn should_respect_allow_empty_match_option() {
        let value = json!({"id": "test"});

        // allow_empty_match is for JSONPath patterns that might match nothing
        let options = RedactOptions {
            allow_empty_match: true,
        };
        let redacted = ValueRedactionBuilder::new(value)
            .redact_with_options("$.nonexistent", "REDACTED", options)
            .expect("should succeed with allow_empty_match")
            .finish();

        assert_eq!(redacted["id"], "test");
    }

    #[test]
    fn should_handle_json_value_redactor() {
        let value = json!({"data": "old"});

        let redacted = ValueRedactionBuilder::new(value)
            .redact("/data", json!({"nested": "value"}))
            .expect("redaction should succeed")
            .finish();

        assert_eq!(redacted["data"]["nested"], "value");
    }

    #[test]
    fn should_handle_openapi_example_redaction() {
        let openapi = json!({
            "paths": {
                "/users": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {
                                        "example": {
                                            "id": "real-uuid-here",
                                            "created_at": "2024-12-28T15:30:00Z"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let stabilized = ValueRedactionBuilder::new(openapi)
            .redact("$..example.id", "ENTITY_ID")
            .expect("redaction should succeed")
            .redact("$..example.created_at", "TIMESTAMP")
            .expect("redaction should succeed")
            .finish();

        let example = &stabilized["paths"]["/users"]["get"]["responses"]["200"]["content"]["application/json"]
            ["example"];
        assert_eq!(example["id"], "ENTITY_ID");
        assert_eq!(example["created_at"], "TIMESTAMP");
    }
}
