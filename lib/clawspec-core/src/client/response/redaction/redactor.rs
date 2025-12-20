//! Redactor trait and implementations for flexible value transformation.
//!
//! This module provides the [`Redactor`] trait which allows different types to be used
//! for redaction operations. The trait is implemented for:
//!
//! - Static values (`&str`, `String`, `serde_json::Value`)
//! - Functions `Fn(&str, &Value) -> Value`
//!
//! # Examples
//!
//! ```ignore
//! // Static value
//! .redact("/id", "stable-uuid")?
//!
//! // Function transformation (path available, ignore if not needed)
//! .redact("$..timestamp", |_path, _val| {
//!     serde_json::json!("2024-01-01T00:00:00Z")
//! })?
//!
//! // Path-aware transformation
//! .redact("$.items[*].id", |path, _val| {
//!     let idx = path.split('/').nth(2).unwrap_or("0");
//!     serde_json::json!(format!("id-{idx}"))
//! })?
//! ```

use serde_json::Value;

/// Trait for types that can be used to redact values.
///
/// This trait defines how a redactor transforms a value at a given path.
/// Implementations receive both the concrete JSON Pointer path and the current
/// value, allowing for path-aware or value-based transformations.
///
/// # Implementations
///
/// - `&str` - Replace with a static string
/// - `String` - Replace with a string
/// - `serde_json::Value` - Replace with a JSON value
/// - `Fn(&str, &Value) -> Value` - Transform using a function
///
/// # Examples
///
/// ```ignore
/// // Static replacement
/// .redact("/id", "stable-uuid")?
///
/// // Value-based transformation (ignore path)
/// .redact("$..notes", |_path, val| {
///     if val.as_str().map(|s| s.len() > 50).unwrap_or(false) {
///         serde_json::json!("[REDACTED]")
///     } else {
///         val.clone()
///     }
/// })?
///
/// // Path-aware transformation
/// .redact("$.items[*].id", |path, _val| {
///     let idx = path.split('/').nth(2).unwrap_or("0");
///     serde_json::json!(format!("stable-id-{idx}"))
/// })?
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "redaction")))]
pub trait Redactor {
    /// Apply the redaction to a value at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - The concrete JSON Pointer path (e.g., `/items/0/id`)
    /// * `current` - The current value at that path
    ///
    /// # Returns
    ///
    /// The new value to replace the current one.
    fn apply(&self, path: &str, current: &Value) -> Value;
}

// Implementation for serde_json::Value (direct replacement)
impl Redactor for Value {
    fn apply(&self, _path: &str, _current: &Value) -> Value {
        self.clone()
    }
}

// Implementation for &str
impl Redactor for &str {
    fn apply(&self, _path: &str, _current: &Value) -> Value {
        Value::String((*self).to_string())
    }
}

// Implementation for String
impl Redactor for String {
    fn apply(&self, _path: &str, _current: &Value) -> Value {
        Value::String(self.clone())
    }
}

// Implementation for Fn(&str, &Value) -> Value
impl<F> Redactor for F
where
    F: Fn(&str, &Value) -> Value,
{
    fn apply(&self, path: &str, current: &Value) -> Value {
        self(path, current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn should_redact_with_static_str() {
        let redactor = "replacement";
        let result = redactor.apply("/id", &json!("original"));
        assert_eq!(result, json!("replacement"));
    }

    #[test]
    fn should_redact_with_static_string() {
        let redactor = String::from("replacement");
        let result = redactor.apply("/id", &json!("original"));
        assert_eq!(result, json!("replacement"));
    }

    #[test]
    fn should_redact_with_json_value() {
        let redactor = json!({"nested": "value"});
        let result = redactor.apply("/data", &json!("original"));
        assert_eq!(result, json!({"nested": "value"}));
    }

    #[test]
    fn should_redact_with_fn_ignoring_path() {
        let redactor =
            |_path: &str, val: &Value| json!(format!("masked-{}", val.as_str().unwrap_or("?")));
        let result = redactor.apply("/id", &json!("secret"));
        assert_eq!(result, json!("masked-secret"));
    }

    #[test]
    fn should_redact_with_fn_using_path() {
        let redactor = |path: &str, _val: &Value| {
            let idx = path.split('/').nth(2).unwrap_or("0");
            json!(format!("id-{idx}"))
        };
        let result = redactor.apply("/items/2/id", &json!("uuid-xxx"));
        assert_eq!(result, json!("id-2"));
    }

    #[test]
    fn should_redact_fn_clone_value() {
        let redactor = |_path: &str, val: &Value| val.clone();
        let result = redactor.apply("/any/path", &json!(42));
        assert_eq!(result, json!(42));
    }

    #[test]
    fn should_redact_fn_use_both_args() {
        let redactor = |path: &str, val: &Value| json!(format!("{}={}", path, val));
        let result = redactor.apply("/key", &json!("value"));
        assert_eq!(result, json!("/key=\"value\""));
    }
}
