//! Path selector supporting both JSON Pointer (RFC 6901) and JSONPath (RFC 9535) syntax.
//!
//! This module provides a unified way to specify paths for redaction, automatically
//! detecting the syntax based on the path prefix:
//! - Paths starting with `$` are parsed as JSONPath (supports wildcards)
//! - Paths starting with `/` are parsed as JSON Pointer (exact paths)

use serde_json_path::JsonPath;

use crate::client::error::ApiClientError;

/// A path selector that supports both JSON Pointer and JSONPath syntax.
///
/// The syntax is auto-detected based on the path prefix:
/// - `$...` → JSONPath (RFC 9535) - supports wildcards like `[*]`, `..`
/// - `/...` → JSON Pointer (RFC 6901) - exact path only
#[derive(Debug, Clone)]
pub enum PathSelector {
    /// JSON Pointer (RFC 6901) - exact path.
    Pointer(String),
    /// JSONPath (RFC 9535) - supports wildcards.
    JsonPath(JsonPath),
}

impl PathSelector {
    /// Parse a path string, auto-detecting the syntax.
    ///
    /// # Syntax Detection
    ///
    /// - Paths starting with `$` are parsed as JSONPath
    /// - Paths starting with `/` are parsed as JSON Pointer
    /// - Other paths return an error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path doesn't start with `$` or `/`
    /// - JSONPath parsing fails for `$` prefixed paths
    pub fn parse(path: &str) -> Result<Self, ApiClientError> {
        if path.starts_with('$') {
            let json_path = JsonPath::parse(path).map_err(|e| ApiClientError::RedactionError {
                message: format!("Invalid JSONPath '{path}': {e}"),
            })?;
            Ok(Self::JsonPath(json_path))
        } else if path.starts_with('/') || path.is_empty() {
            // Empty string is valid JSON Pointer (root)
            Ok(Self::Pointer(path.to_string()))
        } else {
            Err(ApiClientError::RedactionError {
                message: format!(
                    "Invalid path '{path}': must start with '$' (JSONPath) or '/' (JSON Pointer)"
                ),
            })
        }
    }

    /// Resolve the selector to concrete JSON Pointer paths against a JSON value.
    ///
    /// For JSON Pointer, this returns a single-element vector with the pointer itself.
    /// For JSONPath, this queries the JSON and returns all matching paths as JSON Pointers.
    pub fn resolve(&self, json: &serde_json::Value) -> Vec<String> {
        match self {
            Self::Pointer(pointer) => vec![pointer.clone()],
            Self::JsonPath(path) => path
                .query_located(json)
                .locations()
                .map(|loc| loc.to_json_pointer())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn should_parse_json_pointer() {
        let selector = PathSelector::parse("/foo/bar").expect("should parse");
        assert!(matches!(selector, PathSelector::Pointer(_)));
    }

    #[test]
    fn should_parse_empty_pointer() {
        let selector = PathSelector::parse("").expect("should parse");
        assert!(matches!(selector, PathSelector::Pointer(_)));
    }

    #[test]
    fn should_parse_jsonpath() {
        let selector = PathSelector::parse("$.foo.bar").expect("should parse");
        assert!(matches!(selector, PathSelector::JsonPath(_)));
    }

    #[test]
    fn should_parse_jsonpath_with_wildcard() {
        let selector = PathSelector::parse("$.items[*].id").expect("should parse");
        assert!(matches!(selector, PathSelector::JsonPath(_)));
    }

    #[test]
    fn should_reject_invalid_path() {
        let result = PathSelector::parse("foo/bar");
        assert!(result.is_err());
    }

    #[test]
    fn should_resolve_json_pointer() {
        let selector = PathSelector::parse("/foo/0/bar").expect("should parse");
        let json = json!({"foo": [{"bar": 1}]});
        let paths = selector.resolve(&json);
        assert_eq!(paths, vec!["/foo/0/bar"]);
    }

    #[test]
    fn should_resolve_jsonpath_single() {
        let selector = PathSelector::parse("$.foo.bar").expect("should parse");
        let json = json!({"foo": {"bar": 1}});
        let paths = selector.resolve(&json);
        assert_eq!(paths, vec!["/foo/bar"]);
    }

    #[test]
    fn should_resolve_jsonpath_wildcard() {
        let selector = PathSelector::parse("$.items[*].id").expect("should parse");
        let json = json!({
            "items": [
                {"id": "a", "name": "first"},
                {"id": "b", "name": "second"},
                {"id": "c", "name": "third"}
            ]
        });
        let paths = selector.resolve(&json);
        assert_eq!(paths, vec!["/items/0/id", "/items/1/id", "/items/2/id"]);
    }

    #[test]
    fn should_resolve_jsonpath_recursive_descent() {
        let selector = PathSelector::parse("$..id").expect("should parse");
        let json = json!({
            "id": "root",
            "nested": {
                "id": "nested"
            }
        });
        let paths = selector.resolve(&json);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"/id".to_string()));
        assert!(paths.contains(&"/nested/id".to_string()));
    }

    #[test]
    fn should_resolve_empty_for_no_matches() {
        let selector = PathSelector::parse("$.nonexistent").expect("should parse");
        let json = json!({"foo": "bar"});
        let paths = selector.resolve(&json);
        assert!(paths.is_empty());
    }
}
