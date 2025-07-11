//! Path parameter handling for HTTP requests with OpenAPI 3.1 support.
//!
//! This module provides a type-safe system for building HTTP paths with parameterized
//! segments that comply with the OpenAPI 3.1 specification. It supports parameter
//! substitution, URL encoding, and automatic OpenAPI schema generation.
//!
//! # Key Features
//!
//! - **Type Safety**: Compile-time guarantees that parameters implement required traits
//! - **OpenAPI Compliance**: Supports different parameter styles for arrays
//! - **URL Encoding**: Proper percent-encoding of parameter values
//! - **Parameter Substitution**: Template-based path building with `{param}` syntax
//! - **Duplicate Parameter Support**: Handles same parameter appearing multiple times in path
//! - **Automatic Schema Generation**: Integrates with utoipa for OpenAPI documentation
//! - **Error Handling**: Robust validation and error reporting
//!
//! # Quick Start
//!
//! ```rust
//! use clawspec_utoipa::{CallPath, ParamValue, ParamStyle};
//!
//! // Basic path with single parameter
//! let mut path = CallPath::from("/users/{user_id}");
//! path.add_param("user_id", ParamValue::new(123));
//!
//! // Path with multiple parameters
//! let mut path = CallPath::from("/users/{user_id}/posts/{post_id}");
//! path.add_param("user_id", ParamValue::new(456));
//! path.add_param("post_id", ParamValue::new("hello-world"));
//!
//! // Array parameter with custom style
//! let mut path = CallPath::from("/search/{tags}");
//! path.add_param("tags", ParamValue::with_style(
//!     vec!["rust", "web", "api"],
//!     ParamStyle::PipeDelimited
//! ));
//! // Results in: /search/rust%7Cweb%7Capi
//!
//! // Duplicate parameters (same parameter appears multiple times)
//! let mut path = CallPath::from("/api/{version}/users/{id}/posts/{id}/comments/{version}");
//! path.add_param("version", ParamValue::new("v1"));
//! path.add_param("id", ParamValue::new(123));
//! // Results in: /api/v1/users/123/posts/123/comments/v1
//! ```
//!
//! # Parameter Types
//!
//! ## ParamValue
//!
//! Use [`ParamValue`] for any type that implements `Serialize` and `ToSchema`.
//! The parameter value is automatically converted to a string representation
//! suitable for URL path substitution.
//!
//! ```rust
//! # use clawspec_utoipa::{CallPath, ParamValue, ParamStyle};
//! let mut path = CallPath::from("/api/items/{id}");
//!
//! // String parameter
//! path.add_param("id", ParamValue::new("item-123"));
//!
//! // Numeric parameter
//! path.add_param("id", ParamValue::new(42));
//!
//! // Boolean parameter
//! path.add_param("id", ParamValue::new(true));
//!
//! // Array parameter (comma-separated by default)
//! path.add_param("id", ParamValue::new(vec![1, 2, 3]));
//! ```
//!
//! # Array Parameter Styles
//!
//! Path parameters support different array serialization styles:
//!
//! - **Simple/Default**: `value1,value2,value3` (comma-separated)
//! - **SpaceDelimited**: `value1 value2 value3` (space-separated)
//! - **PipeDelimited**: `value1|value2|value3` (pipe-separated)
//!
//! ```rust
//! # use clawspec_utoipa::{CallPath, ParamValue, ParamStyle};
//! let mut path = CallPath::from("/filter/{categories}");
//!
//! // Simple style (default): tech,programming,rust
//! path.add_param("categories", ParamValue::new(vec!["tech", "programming", "rust"]));
//!
//! // Space delimited: tech%20programming%20rust
//! path.add_param("categories", ParamValue::with_style(
//!     vec!["tech", "programming", "rust"],
//!     ParamStyle::SpaceDelimited
//! ));
//!
//! // Pipe delimited: tech%7Cprogramming%7Crust
//! path.add_param("categories", ParamValue::with_style(
//!     vec!["tech", "programming", "rust"],
//!     ParamStyle::PipeDelimited
//! ));
//! ```
//!
//! # URL Encoding
//!
//! All parameter values are automatically percent-encoded according to RFC 3986
//! to ensure URL safety:
//!
//! ```rust
//! # use clawspec_utoipa::{CallPath, ParamValue};
//! let mut path = CallPath::from("/search/{query}");
//! path.add_param("query", ParamValue::new("hello world & more"));
//! // Results in: /search/hello%20world%20%26%20more
//! ```
//!
//! # Error Handling
//!
//! The path system provides robust error handling for common issues:
//!
//! - **Missing Parameters**: Paths with unresolved `{param}` placeholders
//! - **Invalid Values**: Object or nested array parameters
//! - **Type Errors**: Parameters that don't implement required traits
//!
//! Path resolution occurs when converting `CallPath` to `PathResolved`,
//! which validates that all parameters have been provided.

use std::fmt::Debug;
use std::sync::LazyLock;

use indexmap::IndexMap;
use percent_encoding::NON_ALPHANUMERIC;
use regex::Regex;
use tracing::warn;

use super::param::ParameterValue;
use super::param::ResolvedParamValue;
use super::schema::Schemas;
use super::{ApiClientError, ParamValue};
use utoipa::openapi::Required;
use utoipa::openapi::path::{Parameter, ParameterIn};

/// Regular expression for matching path parameters in the format `{param_name}`.
static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(?<name>\w*)}").expect("a valid regex"));

/// Optimized string replacement that avoids format! macro allocations
fn replace_path_param(path: &str, param_name: &str, value: &str) -> String {
    // Pre-allocate capacity based on the likely size difference
    let mut result = String::with_capacity(path.len() + value.len());
    let pattern = ["{", param_name, "}"].concat();
    
    let mut last_end = 0;
    while let Some(start) = path[last_end..].find(&pattern) {
        let actual_start = last_end + start;
        result.push_str(&path[last_end..actual_start]);
        result.push_str(value);
        last_end = actual_start + pattern.len();
    }
    result.push_str(&path[last_end..]);
    result
}

/// A parameterized HTTP path with type-safe parameter substitution.
///
/// `CallPath` represents an HTTP path template with named parameters that can be
/// substituted with typed values. It supports OpenAPI 3.1 parameter styles and
/// automatic schema generation.
///
/// # Examples
///
/// ```rust
/// use clawspec_utoipa::{CallPath, ParamValue};
///
/// // Create a path template
/// let mut path = CallPath::from("/users/{user_id}/posts/{post_id}");
///
/// // Add typed parameters
/// path.add_param("user_id", ParamValue::new(123));
/// path.add_param("post_id", ParamValue::new("my-post"));
///
/// // Path is now ready for resolution to: /users/123/posts/my-post
/// ```
///
/// # Path Template Syntax
///
/// Path templates use `{parameter_name}` syntax for parameter placeholders.
/// Parameter names must be valid identifiers (alphanumeric + underscore).
/// The same parameter can appear multiple times in a single path.
///
/// ```rust
/// # use clawspec_utoipa::{CallPath, ParamValue};
/// let mut path = CallPath::from("/api/v1/users/{user_id}/documents/{doc_id}");
/// path.add_param("user_id", ParamValue::new(456));
/// path.add_param("doc_id", ParamValue::new("report-2023"));
///
/// // Duplicate parameters are supported
/// let mut path = CallPath::from("/test/{id}/{id}");
/// path.add_param("id", ParamValue::new(123));
/// // Results in: /test/123/123
/// ```
#[derive(Debug, Clone, Default, derive_more::Display)]
#[display("{path}")]
pub struct CallPath {
    /// The path template with parameter placeholders
    pub(super) path: String,
    /// Resolved parameter values indexed by parameter name
    args: IndexMap<String, ResolvedParamValue>,
    /// OpenAPI schemas for the parameters
    schemas: Schemas,
}

impl CallPath {
    /// Adds a path parameter with the given name and value.
    ///
    /// This method accepts any value that can be converted into a `ParamValue<T>`,
    /// allowing for ergonomic usage where you can pass values directly or use
    /// explicit `ParamValue` wrappers for custom styles.
    ///
    /// # Parameters
    ///
    /// - `name`: The parameter name (will be converted to `String`)
    /// - `param`: The parameter value that can be converted into `ParamValue<T>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{CallPath, ParamValue, ParamStyle};
    ///
    /// let mut path = CallPath::from("/users/{id}");
    ///
    /// // Ergonomic usage - pass values directly
    /// path.add_param("id", 123);
    ///
    /// // Explicit ParamValue usage for custom styles
    /// path.add_param("id", ParamValue::with_style(456, ParamStyle::Simple));
    /// ```
    pub fn add_param<T: ParameterValue>(
        &mut self,
        name: impl Into<String>,
        param: impl Into<ParamValue<T>>,
    ) {
        let name = name.into();
        let param = param.into();
        if let Some(resolved) = param.resolve(|value| self.schemas.add_example::<T>(value)) {
            self.args.insert(name, resolved);
        }
    }

    /// Creates an iterator over OpenAPI parameters for path parameters.
    ///
    /// This method converts the internal path parameter representation into
    /// OpenAPI Parameter objects suitable for inclusion in the OpenAPI specification.
    /// All path parameters are marked as required according to the OpenAPI specification.
    ///
    /// # Returns
    ///
    /// An iterator over `Parameter` objects representing path parameters.
    pub(super) fn to_parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        self.args.iter().map(|(name, value)| {
            Parameter::builder()
                .name(name)
                .parameter_in(ParameterIn::Path)
                .required(Required::True) // Path parameters are always required
                .schema(Some(value.schema.clone()))
                .style(value.style.into())
                .build()
        })
    }

    /// Get the schemas collected from path parameters.
    pub(super) fn schemas(&self) -> &Schemas {
        &self.schemas
    }
}

impl From<&str> for CallPath {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for CallPath {
    fn from(value: String) -> Self {
        let path = value;
        let args = Default::default();
        let schemas = Schemas::default();
        Self {
            path,
            args,
            schemas,
        }
    }
}

#[derive(Debug)]
pub(super) struct PathResolved {
    pub(super) path: String,
}

// Build concrete
impl TryFrom<CallPath> for PathResolved {
    type Error = ApiClientError;

    fn try_from(value: CallPath) -> Result<Self, Self::Error> {
        let CallPath {
            mut path,
            args,
            schemas: _,
        } = value;

        // Optimized: Extract all parameter names once using a HashSet for efficient lookup
        let mut names: std::collections::HashSet<String> = RE
            .captures_iter(&path)
            .filter_map(|caps| caps.name("name"))
            .map(|m| m.as_str().to_string())
            .collect();

        if names.is_empty() {
            return Ok(Self { path });
        }

        // Optimized: Process all parameters in a single pass
        for (name, resolved) in args {
            if !names.remove(&name) {
                warn!(?name, "argument name not found");
                continue;
            }

            // Convert JSON value to string for path substitution
            let path_value: String = match resolved.to_string_value() {
                Ok(value) => value,
                Err(err) => {
                    warn!(?resolved.value, error = %err, "failed to serialize path parameter value");
                    continue;
                }
            };

            // TODO explore [URI template](https://datatracker.ietf.org/doc/html/rfc6570) - https://github.com/ilaborie/clawspec/issues/21
            // See <https://crates.io/crates/iri-string>, <https://crates.io/crates/uri-template-system>
            let encoded_value =
                percent_encoding::utf8_percent_encode(&path_value, NON_ALPHANUMERIC).to_string();
            
            // Optimized: Use custom replacement function that avoids string allocations
            path = replace_path_param(&path, &name, &encoded_value);

            if names.is_empty() {
                return Ok(Self { path });
            }
        }

        Err(ApiClientError::PathUnresolved {
            path,
            missings: names.into_iter().collect(),
        })
    }
}

// TODO dsl path!(""/ object / ""...) - https://github.com/ilaborie/clawspec/issues/21

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamStyle;

    #[test]
    fn should_build_call_path() {
        let mut path = CallPath::from("/breed/{breed}/images");
        path.add_param("breed", ParamValue::new("hound"));

        insta::assert_debug_snapshot!(path, @r#"
        CallPath {
            path: "/breed/{breed}/images",
            args: {
                "breed": ResolvedParamValue {
                    value: String("hound"),
                    schema: T(
                        Object(
                            Object {
                                schema_type: Type(
                                    String,
                                ),
                                title: None,
                                format: None,
                                description: None,
                                default: None,
                                enum_values: None,
                                required: [],
                                properties: {},
                                additional_properties: None,
                                property_names: None,
                                deprecated: None,
                                example: None,
                                examples: [],
                                write_only: None,
                                read_only: None,
                                xml: None,
                                multiple_of: None,
                                maximum: None,
                                minimum: None,
                                exclusive_maximum: None,
                                exclusive_minimum: None,
                                max_length: None,
                                min_length: None,
                                pattern: None,
                                max_properties: None,
                                min_properties: None,
                                extensions: None,
                                content_encoding: "",
                                content_media_type: "",
                            },
                        ),
                    ),
                    style: Default,
                },
            },
            schemas: Schemas(
                [
                    "&str",
                ],
            ),
        }
        "#);

        let path_resolved = PathResolved::try_from(path).expect("full resolve");

        insta::assert_debug_snapshot!(path_resolved, @r#"
        PathResolved {
            path: "/breed/hound/images",
        }
        "#);
    }

    #[test]
    fn test_path_resolved_with_multiple_parameters() {
        let mut path = CallPath::from("/users/{user_id}/posts/{post_id}");
        path.add_param("user_id", ParamValue::new(123));
        path.add_param("post_id", ParamValue::new("abc"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        insta::assert_debug_snapshot!(resolved, @r#"
        PathResolved {
            path: "/users/123/posts/abc",
        }
        "#);
    }

    #[test]
    fn test_path_resolved_with_missing_parameters() {
        let mut path = CallPath::from("/users/{user_id}/posts/{post_id}");
        path.add_param("user_id", ParamValue::new(123));
        // Missing post_id parameter

        let result = PathResolved::try_from(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_resolved_with_url_encoding() {
        let mut path = CallPath::from("/search/{query}");
        path.add_param("query", ParamValue::new("hello world"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        assert_eq!(resolved.path, "/search/hello%20world");
    }

    #[test]
    fn test_path_resolved_with_special_characters() {
        let mut path = CallPath::from("/items/{name}");
        path.add_param("name", ParamValue::new("test@example.com"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        insta::assert_snapshot!(resolved.path, @"/items/test%40example%2Ecom");
    }

    #[test]
    fn test_path_with_duplicate_parameter_names() {
        let mut path = CallPath::from("/test/{id}/{id}");
        path.add_param("id", ParamValue::new(123));

        // The algorithm now properly handles duplicates using names.retain()
        // It removes all occurrences of the parameter name from the list
        let result = PathResolved::try_from(path);

        // Should now succeed - duplicate parameters are handled correctly
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.path, "/test/123/123");
    }

    #[test]
    fn test_path_with_multiple_duplicate_parameters() {
        let mut path = CallPath::from("/api/{version}/users/{id}/posts/{id}/comments/{version}");
        path.add_param("version", ParamValue::new("v1"));
        path.add_param("id", ParamValue::new(456));

        // Test with multiple parameters that appear multiple times
        let result = PathResolved::try_from(path);

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.path, "/api/v1/users/456/posts/456/comments/v1");
    }

    #[test]
    fn test_add_param_overwrites_existing() {
        let mut path = CallPath::from("/test/{id}");
        path.add_param("id", ParamValue::new(123));
        path.add_param("id", ParamValue::new(456)); // Overwrite

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/test/456");
    }

    #[test]
    fn test_path_with_array_simple_style() {
        let mut path = CallPath::from("/search/{tags}");
        path.add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::Simple),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%2Cweb%2Capi");
    }

    #[test]
    fn test_path_with_array_default_style() {
        let mut path = CallPath::from("/search/{tags}");
        path.add_param("tags", ParamValue::new(vec!["rust", "web", "api"])); // Default style

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%2Cweb%2Capi"); // Default is Simple for paths
    }

    #[test]
    fn test_path_with_array_space_delimited_style() {
        let mut path = CallPath::from("/search/{tags}");
        path.add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::SpaceDelimited),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%20web%20api");
    }

    #[test]
    fn test_path_with_array_pipe_delimited_style() {
        let mut path = CallPath::from("/search/{tags}");
        path.add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::PipeDelimited),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%7Cweb%7Capi");
    }

    #[test]
    fn test_path_with_mixed_array_types() {
        let mut path = CallPath::from("/items/{values}");
        path.add_param(
            "values",
            ParamValue::with_style(vec![1, 2, 3], ParamStyle::Simple),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/items/1%2C2%2C3");
    }
}
