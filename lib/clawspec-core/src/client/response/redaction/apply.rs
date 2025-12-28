//! Core redaction application logic.
//!
//! This module provides the internal functions for applying redactions to JSON values.
//! These functions are used by both `RedactionBuilder` (for response bodies) and
//! `ValueRedactionBuilder` (for arbitrary JSON values).

use jsonptr::{Pointer, assign::Assign, delete::Delete, resolve::Resolve};
use serde_json::Value;

use super::RedactOptions;
use super::path_selector::PathSelector;
use super::redactor::Redactor;
use crate::client::error::ApiClientError;

/// Apply a redactor to all paths matching the selector in the given JSON value.
///
/// This function:
/// 1. Parses the path (auto-detecting JSON Pointer vs JSONPath syntax)
/// 2. Resolves to concrete JSON Pointer paths
/// 3. Applies the redactor to each matched path
///
/// # Arguments
///
/// * `json` - The JSON value to modify in place
/// * `path` - Path expression (e.g., `/id`, `$.items[*].id`)
/// * `redactor` - The redactor to apply (static value or closure)
/// * `options` - Configuration options (e.g., allow empty matches)
///
/// # Errors
///
/// Returns an error if:
/// - The path is invalid
/// - The path matches no values (unless `allow_empty_match` is set)
/// - JSON Pointer operations fail
pub(crate) fn apply_redaction<R: Redactor>(
    json: &mut Value,
    path: &str,
    redactor: R,
    options: RedactOptions,
) -> Result<(), ApiClientError> {
    // Parse the path (auto-detect JSONPath vs JSON Pointer)
    let selector = PathSelector::parse(path)?;

    // Resolve to concrete JSON Pointer paths
    let concrete_paths = selector.resolve(json);

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
        let current_value = json
            .resolve(ptr)
            .map_err(|e| ApiClientError::RedactionError {
                message: format!("Failed to resolve path '{pointer}': {e}"),
            })?;

        // Apply redactor transformation
        let new_value = redactor.apply(&pointer, current_value);

        // Assign new value
        json.assign(ptr, new_value)
            .map_err(|e| ApiClientError::RedactionError {
                message: format!("Failed to assign value at path '{pointer}': {e}"),
            })?;
    }

    Ok(())
}

/// Remove values at all paths matching the selector in the given JSON value.
///
/// This function:
/// 1. Parses the path (auto-detecting JSON Pointer vs JSONPath syntax)
/// 2. Resolves to concrete JSON Pointer paths
/// 3. Deletes the value at each matched path
///
/// # Arguments
///
/// * `json` - The JSON value to modify in place
/// * `path` - Path expression (e.g., `/id`, `$.items[*].id`)
/// * `options` - Configuration options (e.g., allow empty matches)
///
/// # Errors
///
/// Returns an error if:
/// - The path is invalid
/// - The path matches no values (unless `allow_empty_match` is set)
pub(crate) fn apply_remove(
    json: &mut Value,
    path: &str,
    options: RedactOptions,
) -> Result<(), ApiClientError> {
    // Parse the path (auto-detect JSONPath vs JSON Pointer)
    let selector = PathSelector::parse(path)?;

    // Resolve to concrete JSON Pointer paths
    let concrete_paths = selector.resolve(json);

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
        let _ = json.delete(ptr);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn should_apply_redaction_with_json_pointer() {
        let mut json = json!({"id": "uuid-123", "name": "Test"});

        apply_redaction(&mut json, "/id", "REDACTED", RedactOptions::default())
            .expect("redaction should succeed");

        assert_eq!(json.get("id").and_then(|v| v.as_str()), Some("REDACTED"));
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Test"));
    }

    #[test]
    fn should_apply_redaction_with_jsonpath() {
        let mut json = json!({
            "items": [
                {"id": "a", "value": 1},
                {"id": "b", "value": 2}
            ]
        });

        apply_redaction(
            &mut json,
            "$.items[*].id",
            "REDACTED",
            RedactOptions::default(),
        )
        .expect("redaction should succeed");

        let items = json.get("items").and_then(|v| v.as_array()).expect("items");
        assert_eq!(
            items[0].get("id").and_then(|v| v.as_str()),
            Some("REDACTED")
        );
        assert_eq!(
            items[1].get("id").and_then(|v| v.as_str()),
            Some("REDACTED")
        );
    }

    #[test]
    fn should_apply_redaction_with_closure() {
        let mut json = json!({
            "items": [
                {"id": "a"},
                {"id": "b"}
            ]
        });

        apply_redaction(
            &mut json,
            "$.items[*].id",
            |path: &str, _val: &Value| {
                let idx = path.split('/').nth(2).unwrap_or("?");
                json!(format!("item-{idx}"))
            },
            RedactOptions::default(),
        )
        .expect("redaction should succeed");

        let items = json.get("items").and_then(|v| v.as_array()).expect("items");
        assert_eq!(items[0].get("id").and_then(|v| v.as_str()), Some("item-0"));
        assert_eq!(items[1].get("id").and_then(|v| v.as_str()), Some("item-1"));
    }

    #[test]
    fn should_fail_on_no_match_by_default() {
        let mut json = json!({"id": "test"});

        let result = apply_redaction(
            &mut json,
            "$.nonexistent",
            "REDACTED",
            RedactOptions::default(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn should_allow_empty_match_with_option() {
        let mut json = json!({"id": "test"});

        let result = apply_redaction(
            &mut json,
            "$.nonexistent",
            "REDACTED",
            RedactOptions {
                allow_empty_match: true,
            },
        );

        assert!(result.is_ok());
    }

    #[test]
    fn should_remove_with_json_pointer() {
        let mut json = json!({"id": "uuid-123", "name": "Test"});

        apply_remove(&mut json, "/id", RedactOptions::default()).expect("removal should succeed");

        assert!(json.get("id").is_none());
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Test"));
    }

    #[test]
    fn should_remove_with_jsonpath() {
        let mut json = json!({
            "items": [
                {"id": "a", "value": 1},
                {"id": "b", "value": 2}
            ]
        });

        apply_remove(&mut json, "$.items[*].id", RedactOptions::default())
            .expect("removal should succeed");

        let items = json.get("items").and_then(|v| v.as_array()).expect("items");
        assert!(items[0].get("id").is_none());
        assert!(items[1].get("id").is_none());
        assert_eq!(items[0].get("value").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(items[1].get("value").and_then(|v| v.as_i64()), Some(2));
    }

    #[test]
    fn should_fail_remove_on_no_match_by_default() {
        let mut json = json!({"id": "test"});

        let result = apply_remove(&mut json, "$.nonexistent", RedactOptions::default());

        assert!(result.is_err());
    }

    #[test]
    fn should_allow_empty_match_on_remove_with_option() {
        let mut json = json!({"id": "test"});

        let result = apply_remove(
            &mut json,
            "$.nonexistent",
            RedactOptions {
                allow_empty_match: true,
            },
        );

        assert!(result.is_ok());
    }

    #[test]
    fn should_fail_on_invalid_path() {
        let mut json = json!({"id": "test"});

        let result = apply_redaction(
            &mut json,
            "invalid_path",
            "REDACTED",
            RedactOptions::default(),
        );

        assert!(result.is_err());
    }
}
