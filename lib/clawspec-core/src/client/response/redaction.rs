//! JSON response redaction support using JSON Pointer (RFC 6901).
//!
//! This module provides functionality to redact sensitive or dynamic values
//! in JSON responses for snapshot testing. It allows you to replace or remove
//! values at specific JSON Pointer paths while preserving the original data
//! for test assertions.
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
//! let result = client
//!     .get("/api/users/123")?
//!     .await?
//!     .as_json_redacted::<User>().await?
//!     .redact_replace("/id", "550e8400-e29b-41d4-a716-446655440000")?
//!     .redact_replace("/created_at", "2024-01-01T00:00:00Z")?
//!     .finish()
//!     .await;
//!
//! // Test assertions use the real value
//! assert_eq!(result.value.name, "John Doe");
//!
//! // Snapshots use the redacted value (stable id and timestamp)
//! insta::assert_yaml_snapshot!(result.redacted);
//! # Ok(())
//! # }
//! ```

use std::any::{TypeId, type_name};

use headers::ContentType;
use http::StatusCode;
use jsonptr::{Pointer, assign::Assign, delete::Delete};
use serde::{Serialize, de::DeserializeOwned};
use utoipa::ToSchema;
use utoipa::openapi::{RefOr, Schema};

use super::output::Output;
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
    ///     .redact_replace("/id", "stable-uuid")?
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
    pub async fn as_json_redacted<T>(
        &mut self,
    ) -> Result<super::redaction::RedactionBuilder<T>, ApiClientError>
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

/// Builder for applying redactions to JSON responses.
///
/// This builder allows you to chain multiple redaction operations before
/// finalizing the result. Redactions are applied using JSON Pointer (RFC 6901)
/// syntax.
///
/// # JSON Pointer Syntax
///
/// - `/field` - top-level field
/// - `/field/subfield` - nested field
/// - `/array/0` - array index
/// - `/field~1with~1slashes` - `~1` escapes `/`
/// - `/field~0with~0tildes` - `~0` escapes `~`
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

    /// Replaces the value at the specified JSON Pointer path with a new value.
    ///
    /// The replacement value can be any type that implements [`Serialize`].
    ///
    /// # Arguments
    ///
    /// * `pointer` - JSON Pointer path (e.g., `/id`, `/user/email`, `/items/0`)
    /// * `replacement` - Value to replace with (will be serialized to JSON)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pointer path is invalid
    /// - The pointer path does not exist in the JSON
    /// - The replacement value cannot be serialized
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use clawspec_core::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = ApiClient::builder().with_base_path("http://localhost".parse()?).build()?;
    /// let result = client
    ///     .get("/api/users")?
    ///     .await?
    ///     .as_json_redacted::<Vec<serde_json::Value>>().await?
    ///     .redact_replace("/0/id", "test-uuid")?
    ///     .redact_replace("/0/age", 25)?
    ///     .finish()
    ///     .await;
    /// # Ok(())
    /// # }
    /// ```
    pub fn redact_replace<V>(
        mut self,
        pointer: &str,
        replacement: V,
    ) -> Result<Self, ApiClientError>
    where
        V: Serialize,
    {
        // Serialize the replacement value
        let replacement_value =
            serde_json::to_value(replacement).map_err(|e| ApiClientError::SerializationError {
                message: format!(
                    "Failed to serialize replacement value for pointer '{pointer}': {e}"
                ),
            })?;

        // Parse the JSON Pointer
        let ptr = Pointer::parse(pointer).map_err(|e| ApiClientError::RedactionError {
            message: format!("Invalid JSON Pointer '{pointer}': {e}"),
        })?;

        // Use jsonptr to assign the value
        self.redacted.assign(ptr, replacement_value).map_err(|e| {
            ApiClientError::RedactionError {
                message: format!("Failed to replace value at pointer '{pointer}': {e}"),
            }
        })?;

        Ok(self)
    }

    /// Removes the value at the specified JSON Pointer path.
    ///
    /// This completely removes the field from objects or the element from arrays,
    /// unlike setting it to `null`.
    ///
    /// # Arguments
    ///
    /// * `pointer` - JSON Pointer path to remove
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pointer path is invalid
    /// - The pointer path does not exist in the JSON
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use clawspec_core::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = ApiClient::builder().with_base_path("http://localhost".parse()?).build()?;
    /// let result = client
    ///     .get("/api/users/123")?
    ///     .await?
    ///     .as_json_redacted::<serde_json::Value>().await?
    ///     .redact_remove("/password")?
    ///     .redact_remove("/creditCard")?
    ///     .finish()
    ///     .await;
    /// # Ok(())
    /// # }
    /// ```
    pub fn redact_remove(mut self, pointer: &str) -> Result<Self, ApiClientError> {
        // Parse the JSON Pointer
        let ptr = Pointer::parse(pointer).map_err(|e| ApiClientError::RedactionError {
            message: format!("Invalid JSON Pointer '{pointer}': {e}"),
        })?;

        // Use jsonptr to delete the value
        // Delete returns None if the pointer doesn't exist, which is fine - we'll just continue
        let _ = self.redacted.delete(ptr);

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
    let deserializer = &mut serde_json::Deserializer::from_str(json);
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
