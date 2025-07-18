use std::fmt::Debug;
use std::sync::LazyLock;

use indexmap::IndexMap;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use tracing::warn;

use super::param::ParameterValue;
use super::param::ResolvedParamValue;
use super::schema::Schemas;
use super::{ApiClientError, ParamStyle, ParamValue};
use utoipa::openapi::Required;
use utoipa::openapi::path::{Parameter, ParameterIn};

/// Regular expression for matching path parameters in the format `{param_name}`.
static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(?<name>\w+)}").expect("a valid regex"));

fn replace_path_param(path: &str, param_name: &str, value: &str) -> String {
    // Use concat to avoid format! macro allocation, but keep str::replace for correctness
    let pattern = ["{", param_name, "}"].concat();
    path.replace(&pattern, value)
}

/// URL-encode a path parameter value using percent-encoding for proper path encoding.
/// This approach maintains the existing behavior while consolidating the encoding logic
/// in a single function that can be reused and tested independently.
fn encode_path_param_value(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
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
/// use clawspec_core::{CallPath, ParamValue};
///
/// // Create a path template with method chaining
/// let path = CallPath::from("/users/{user_id}/posts/{post_id}")
///     .add_param("user_id", ParamValue::new(123))
///     .add_param("post_id", ParamValue::new("my-post"));
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
/// # use clawspec_core::{CallPath, ParamValue};
/// let path = CallPath::from("/api/v1/users/{user_id}/documents/{doc_id}")
///     .add_param("user_id", ParamValue::new(456))
///     .add_param("doc_id", ParamValue::new("report-2023"));
///
/// // Duplicate parameters are supported
/// let path = CallPath::from("/test/{id}/{id}")
///     .add_param("id", ParamValue::new(123));
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
    /// use clawspec_core::{CallPath, ParamValue, ParamStyle};
    ///
    /// // Ergonomic usage - pass values directly with method chaining
    /// let path = CallPath::from("/users/{id}")
    ///     .add_param("id", 123);
    ///
    /// // Explicit ParamValue usage for custom styles
    /// let path = CallPath::from("/users/{id}")
    ///     .add_param("id", ParamValue::with_style(456, ParamStyle::Simple));
    /// ```
    pub fn add_param<T: ParameterValue>(
        mut self,
        name: impl Into<String>,
        param: impl Into<ParamValue<T>>,
    ) -> Self {
        let name = name.into();
        let param = param.into();
        if let Some(resolved) = param.resolve(|value| self.schemas.add_example::<T>(value)) {
            self.args.insert(name, resolved);
        }
        self
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

            // Apply path parameter style formatting
            let formatted_value = match resolved.style {
                ParamStyle::Label => {
                    // Label style: /users/.value
                    format!(".{path_value}")
                }
                ParamStyle::Matrix => {
                    // Matrix style: /users/;name=value
                    format!(";{name}={path_value}")
                }
                ParamStyle::DeepObject => {
                    warn!(?resolved.style, "DeepObject style not supported for path parameters");
                    continue;
                }
                _ => {
                    // Default, Simple, Form, SpaceDelimited, PipeDelimited
                    path_value
                }
            };

            // TODO explore [URI template](https://datatracker.ietf.org/doc/html/rfc6570) - https://github.com/ilaborie/clawspec/issues/21
            // See <https://crates.io/crates/iri-string>, <https://crates.io/crates/uri-template-system>
            let encoded_value = encode_path_param_value(&formatted_value);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamStyle;

    #[test]
    fn should_build_call_path() {
        let path =
            CallPath::from("/breed/{breed}/images").add_param("breed", ParamValue::new("hound"));

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
        let path = CallPath::from("/users/{user_id}/posts/{post_id}")
            .add_param("user_id", ParamValue::new(123))
            .add_param("post_id", ParamValue::new("abc"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        insta::assert_debug_snapshot!(resolved, @r#"
        PathResolved {
            path: "/users/123/posts/abc",
        }
        "#);
    }

    #[test]
    fn test_path_resolved_with_missing_parameters() {
        let path = CallPath::from("/users/{user_id}/posts/{post_id}")
            .add_param("user_id", ParamValue::new(123));
        // Missing post_id parameter

        let result = PathResolved::try_from(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_resolved_with_url_encoding() {
        let path =
            CallPath::from("/search/{query}").add_param("query", ParamValue::new("hello world"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        assert_eq!(resolved.path, "/search/hello%20world");
    }

    #[test]
    fn test_path_resolved_with_special_characters() {
        let path =
            CallPath::from("/items/{name}").add_param("name", ParamValue::new("test@example.com"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        insta::assert_snapshot!(resolved.path, @"/items/test%40example%2Ecom");
    }

    #[test]
    fn test_path_with_duplicate_parameter_names() {
        let path = CallPath::from("/test/{id}/{id}").add_param("id", ParamValue::new(123));

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
        let path = CallPath::from("/api/{version}/users/{id}/posts/{id}/comments/{version}")
            .add_param("version", ParamValue::new("v1"))
            .add_param("id", ParamValue::new(456));

        // Test with multiple parameters that appear multiple times
        let result = PathResolved::try_from(path);

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.path, "/api/v1/users/456/posts/456/comments/v1");
    }

    #[test]
    fn test_add_param_overwrites_existing() {
        let path = CallPath::from("/test/{id}")
            .add_param("id", ParamValue::new(123))
            .add_param("id", ParamValue::new(456)); // Overwrite

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/test/456");
    }

    #[test]
    fn test_path_with_array_simple_style() {
        let path = CallPath::from("/search/{tags}").add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::Simple),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%2Cweb%2Capi");
    }

    #[test]
    fn test_path_with_array_default_style() {
        let path = CallPath::from("/search/{tags}")
            .add_param("tags", ParamValue::new(vec!["rust", "web", "api"])); // Default style

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%2Cweb%2Capi"); // Default is Simple for paths
    }

    #[test]
    fn test_path_with_array_space_delimited_style() {
        let path = CallPath::from("/search/{tags}").add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::SpaceDelimited),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%20web%20api");
    }

    #[test]
    fn test_path_with_array_pipe_delimited_style() {
        let path = CallPath::from("/search/{tags}").add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::PipeDelimited),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/rust%7Cweb%7Capi");
    }

    #[test]
    fn test_path_with_label_style() {
        let path = CallPath::from("/users/{id}")
            .add_param("id", ParamValue::with_style(123, ParamStyle::Label));

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/users/%2E123");
    }

    #[test]
    fn test_path_with_label_style_array() {
        let path = CallPath::from("/search/{tags}").add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web"], ParamStyle::Label),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/%2Erust%2Cweb");
    }

    #[test]
    fn test_path_with_matrix_style() {
        let path = CallPath::from("/users/{id}")
            .add_param("id", ParamValue::with_style(123, ParamStyle::Matrix));

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/users/%3Bid%3D123");
    }

    #[test]
    fn test_path_with_matrix_style_array() {
        let path = CallPath::from("/search/{tags}").add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web"], ParamStyle::Matrix),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/search/%3Btags%3Drust%2Cweb");
    }

    #[test]
    fn test_path_with_mixed_array_types() {
        let path = CallPath::from("/items/{values}").add_param(
            "values",
            ParamValue::with_style(vec![1, 2, 3], ParamStyle::Simple),
        );

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/items/1%2C2%2C3");
    }

    #[test]
    fn test_replace_path_param_no_collision() {
        // Test that "id" doesn't match inside "user_id"
        let result = replace_path_param("/users/{user_id}/posts/{id}", "id", "123");
        assert_eq!(result, "/users/{user_id}/posts/123");
    }

    #[test]
    fn test_replace_path_param_substring_collision() {
        // Test parameter names that are substrings of each other
        let result = replace_path_param("/api/{user_id}/data/{id}", "id", "456");
        assert_eq!(result, "/api/{user_id}/data/456");

        let result = replace_path_param("/api/{user_id}/data/{id}", "user_id", "789");
        assert_eq!(result, "/api/789/data/{id}");
    }

    #[test]
    fn test_replace_path_param_exact_match_only() {
        // Test that only exact {param} matches are replaced
        let result = replace_path_param("/prefix{param}suffix/{param}", "param", "value");
        assert_eq!(result, "/prefixvaluesuffix/value");
    }

    #[test]
    fn test_replace_path_param_multiple_occurrences() {
        // Test that all occurrences of the same parameter are replaced
        let result = replace_path_param(
            "/api/{version}/users/{id}/posts/{id}/comments/{version}",
            "id",
            "123",
        );
        assert_eq!(
            result,
            "/api/{version}/users/123/posts/123/comments/{version}"
        );
    }

    #[test]
    fn test_replace_path_param_empty_cases() {
        // Test edge cases with empty values
        let result = replace_path_param("/users/{id}", "id", "");
        assert_eq!(result, "/users/");

        let result = replace_path_param("/users/{id}", "nonexistent", "123");
        assert_eq!(result, "/users/{id}");
    }

    #[test]
    fn test_replace_path_param_special_characters() {
        // Test with special characters in parameter values
        let result = replace_path_param("/users/{id}", "id", "user@example.com");
        assert_eq!(result, "/users/user@example.com");

        let result = replace_path_param("/search/{query}", "query", "hello world & more");
        assert_eq!(result, "/search/hello world & more");
    }
}
