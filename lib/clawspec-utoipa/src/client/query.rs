//! Query parameter handling for HTTP requests with OpenAPI 3.1 support.
//!
//! This module provides a type-safe system for building and serializing HTTP query parameters
//! that comply with the OpenAPI 3.1 specification. It supports different parameter styles
//! and automatically generates OpenAPI schemas for documentation and client generation.
//!
//! # Key Features
//!
//! - **Type Safety**: Compile-time guarantees that parameters implement required traits
//! - **OpenAPI 3.1 Compliance**: Supports Form, SpaceDelimited, and PipeDelimited styles
//! - **Builder Pattern**: Fluent API for constructing query parameter collections
//! - **Automatic Schema Generation**: Integrates with utoipa for OpenAPI documentation
//! - **URL Encoding**: Proper percent-encoding of parameter values
//!
//! # Quick Start
//!
//! ```rust
//! use clawspec_utoipa::{CallQuery, ParamValue, ParamStyle};
//!
//! // Ergonomic usage - pass values directly (uses From<T> trait)
//! let query = CallQuery::new()
//!     .add_param("search", "hello world")
//!     .add_param("limit", 10)
//!     .add_param("active", true)
//!     .add_param("tags", vec!["rust", "web", "api"]);
//!
//! // Explicit ParamValue usage for custom styles
//! let query = CallQuery::new()
//!     .add_param("categories", ParamValue::with_style(
//!         vec!["tech", "programming"],
//!         ParamStyle::SpaceDelimited
//!     ));
//!
//! // This would generate a query string like:
//! // ?search=hello+world&limit=10&active=true&tags=rust&tags=web&tags=api&categories=tech+programming
//! ```
//!
//! # Parameter Types
//!
//! ## ParamValue
//!
//! Use [`ParamValue`] for any type that implements `Serialize` and `ToSchema`.
//! You can pass values directly or use explicit `ParamValue` wrappers:
//!
//! ```rust
//! # use clawspec_utoipa::{CallQuery, ParamValue, ParamStyle};
//! // Ergonomic usage - values are automatically wrapped
//! let query = CallQuery::new()
//!     .add_param("name", "John Doe")
//!     .add_param("age", 30)
//!     .add_param("active", true);
//!
//! // Explicit ParamValue for custom styles
//! let query = CallQuery::new()
//!     .add_param("tags", vec!["rust", "web"])  // Uses default Form style
//!     .add_param("ids", ParamValue::with_style(vec![1, 2, 3], ParamStyle::PipeDelimited));
//! ```
//!
//! # Query Styles
//!
//! The module supports three OpenAPI 3.1 query parameter styles:
//!
//! - **Form** (default): Arrays are repeated `?tags=a&tags=b&tags=c`
//! - **SpaceDelimited**: Arrays are joined with spaces `?tags=a%20b%20c`
//! - **PipeDelimited**: Arrays are joined with pipes `?tags=a|b|c`

use std::fmt::Debug;

use indexmap::IndexMap;
use utoipa::openapi::Required;
use utoipa::openapi::path::{Parameter, ParameterIn};

use super::param::ParameterValue;
use super::param::ResolvedParamValue;
use super::schema::Schemas;
use super::{ApiClientError, ParamStyle, ParamValue};

/// A collection of query parameters for HTTP requests with OpenAPI 3.1 support.
///
/// `CallQuery` provides a type-safe way to build and serialize query parameters
/// for HTTP requests. It supports different parameter styles as defined by the
/// OpenAPI 3.1 specification and automatically generates OpenAPI parameter schemas.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use clawspec_utoipa::{CallQuery, ParamValue, ParamStyle};
///
/// let query = CallQuery::new()
///     .add_param("search", ParamValue::new("hello world"))
///     .add_param("limit", ParamValue::new(10))
///     .add_param("active", ParamValue::new(true));
///
/// // This would generate: ?search=hello+world&limit=10&active=true
/// ```
///
/// ## Array Parameters with Different Styles
///
/// ```rust
/// use clawspec_utoipa::{CallQuery, ParamValue, ParamStyle};
/// let query = CallQuery::new()
///     // Form style (default): ?tags=rust&tags=web&tags=api
///     .add_param("tags", ParamValue::new(vec!["rust", "web", "api"]))
///     // Space delimited: ?categories=tech%20programming
///     .add_param("categories", ParamValue::with_style(
///         vec!["tech", "programming"],
///         ParamStyle::SpaceDelimited
///     ))
///     // Pipe delimited: ?ids=1|2|3
///     .add_param("ids", ParamValue::with_style(
///         vec![1, 2, 3],
///         ParamStyle::PipeDelimited
///     ));
/// ```
///
/// # Type Safety
///
/// The query system is type-safe and will prevent invalid parameter types:
/// - Objects are not supported as query parameters (will return an error)
/// - All parameters must implement `Serialize` and `ToSchema` traits
/// - Parameters are automatically converted to appropriate string representations
#[derive(Debug, Default, Clone)]
pub struct CallQuery {
    params: IndexMap<String, ResolvedParamValue>,
    pub(super) schemas: Schemas,
}

impl CallQuery {
    /// Creates a new empty query parameter collection.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::CallQuery;
    ///
    /// let query = CallQuery::new();
    /// // Query is initially empty
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a query parameter with the given name and value using the builder pattern.
    ///
    /// This method consumes `self` and returns a new `CallQuery` with the parameter added,
    /// allowing for method chaining. The parameter must implement both `Serialize` and
    /// `ToSchema` traits for proper serialization and OpenAPI schema generation.
    ///
    /// # Parameters
    ///
    /// - `name`: The parameter name (will be converted to `String`)
    /// - `param`: The parameter value that can be converted into `ParamValue<T>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{CallQuery, ParamValue, ParamStyle};
    ///
    /// // Ergonomic usage - pass values directly
    /// let query = CallQuery::new()
    ///     .add_param("search", "hello")
    ///     .add_param("limit", 10)
    ///     .add_param("active", true);
    ///
    /// // Explicit ParamValue usage for custom styles
    /// let query = CallQuery::new()
    ///     .add_param("tags", ParamValue::with_style(
    ///         vec!["rust", "web"],
    ///         ParamStyle::SpaceDelimited
    ///     ));
    /// ```
    pub fn add_param<T: ParameterValue>(
        mut self,
        name: impl Into<String>,
        param: impl Into<ParamValue<T>>,
    ) -> Self {
        let name = name.into();
        let param = param.into();
        if let Some(resolved) = param.resolve(|value| self.schemas.add_example::<T>(value)) {
            self.params.insert(name, resolved);
        }
        self
    }

    /// Check if the query is empty
    pub(super) fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Convert query parameters to OpenAPI Parameters
    pub(super) fn to_parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        // For query parameters, we need to create a schema for each parameter
        self.params.iter().map(|(name, resolved)| {
            Parameter::builder()
                .name(name)
                .parameter_in(ParameterIn::Query)
                .required(Required::False) // Query parameters are typically optional
                .schema(Some(resolved.schema.clone()))
                .style(resolved.style.into())
                .build()
        })
    }

    /// Serialize query parameters to URL-encoded string
    pub(super) fn to_query_string(&self) -> Result<String, ApiClientError> {
        let mut pairs = Vec::new();

        for (name, resolved) in &self.params {
            match resolved.style {
                ParamStyle::Default | ParamStyle::Form => {
                    self.encode_form_style(name, resolved, &mut pairs)?;
                }
                ParamStyle::SpaceDelimited | ParamStyle::PipeDelimited | ParamStyle::Simple => {
                    self.encode_delimited_style(name, resolved, &mut pairs)?;
                }
            }
        }

        serde_urlencoded::to_string(&pairs).map_err(ApiClientError::from)
    }

    /// Encode a parameter using form style (default)
    fn encode_form_style(
        &self,
        name: &str,
        resolved: &ResolvedParamValue,
        pairs: &mut Vec<(String, String)>,
    ) -> Result<(), ApiClientError> {
        match resolved.to_query_values() {
            Ok(values) => {
                for value in values {
                    pairs.push((name.to_string(), value));
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    /// Encode a parameter using delimited style (space or pipe)
    fn encode_delimited_style(
        &self,
        name: &str,
        resolved: &ResolvedParamValue,
        pairs: &mut Vec<(String, String)>,
    ) -> Result<(), ApiClientError> {
        match resolved.to_string_value() {
            Ok(value) => {
                pairs.push((name.to_string(), value));
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_query_basic_usage() {
        let query = CallQuery::new();

        assert!(query.is_empty());

        let query = query
            .add_param("name", ParamValue::new("test"))
            .add_param("age", ParamValue::new(25));

        assert!(!query.is_empty());
    }

    #[test]
    fn test_ergonomic_api_with_direct_values() {
        // Test that we can pass values directly without wrapping in ParamValue::new()
        let query = CallQuery::new()
            .add_param("name", "test")
            .add_param("age", 25)
            .add_param("active", true)
            .add_param("tags", vec!["rust", "web"]);

        assert!(!query.is_empty());

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=test&age=25&active=true&tags=rust&tags=web""#);
    }

    #[test]
    fn test_mixed_ergonomic_and_explicit_api() {
        // Test mixing direct values with explicit ParamValue wrappers
        let query = CallQuery::new()
            .add_param("name", "test") // Direct value
            .add_param("limit", 10) // Direct value
            .add_param(
                "tags",
                ParamValue::with_style(
                    // Explicit ParamValue with custom style
                    vec!["rust", "web"],
                    ParamStyle::SpaceDelimited,
                ),
            );

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=test&limit=10&tags=rust+web""#);
    }

    #[test]
    fn test_query_param_as_query_value() {
        let query = ParamValue::new("hello world");
        let value = query.as_query_value().expect("should have value");

        insta::assert_debug_snapshot!(value, @r#"String("hello world")"#);
    }

    #[test]
    fn test_query_param_with_different_styles() {
        let default_query = ParamValue::new("test");
        assert_eq!(default_query.style, ParamStyle::Default);

        let form_query = ParamValue::with_style("test", ParamStyle::Form);
        assert_eq!(form_query.style, ParamStyle::Form);

        let space_query = ParamValue::with_style("test", ParamStyle::SpaceDelimited);
        assert_eq!(space_query.style, ParamStyle::SpaceDelimited);

        let pipe_query = ParamValue::with_style("test", ParamStyle::PipeDelimited);
        assert_eq!(pipe_query.style, ParamStyle::PipeDelimited);
    }

    #[test]
    fn test_query_string_serialization_form_style() {
        let query = CallQuery::new()
            .add_param("name", ParamValue::new("john"))
            .add_param("age", ParamValue::new(25));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=john&age=25""#);
    }

    #[test]
    fn test_query_string_serialization_with_arrays() {
        let query = CallQuery::new().add_param("tags", ParamValue::new(vec!["rust", "web", "api"]));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust&tags=web&tags=api""#);
    }

    #[test]
    fn test_query_string_serialization_space_delimited() {
        let query = CallQuery::new().add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::SpaceDelimited),
        );

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust+web+api""#);
    }

    #[test]
    fn test_query_string_serialization_pipe_delimited() {
        let query = CallQuery::new().add_param(
            "tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::PipeDelimited),
        );

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust%7Cweb%7Capi""#);
    }

    #[test]
    fn test_empty_query_serialization() {
        let query = CallQuery::new();
        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""""#);
    }

    #[test]
    fn test_mixed_parameter_types() {
        let query = CallQuery::new()
            .add_param("name", ParamValue::new("john"))
            .add_param("active", ParamValue::new(true))
            .add_param("scores", ParamValue::new(vec![10, 20, 30]));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=john&active=true&scores=10&scores=20&scores=30""#);
    }

    #[test]
    fn test_object_query_parameter_error() {
        use serde_json::json;

        let query = CallQuery::new().add_param("config", ParamValue::new(json!({"key": "value"})));

        let result = query.to_query_string();
        assert!(matches!(
            result,
            Err(ApiClientError::UnsupportedParameterValue { .. })
        ));
    }

    #[test]
    fn test_nested_object_in_array_error() {
        use serde_json::json;

        let query = CallQuery::new().add_param(
            "items",
            ParamValue::new(json!(["valid", {"nested": "object"}])),
        );

        let result = query.to_query_string();
        assert!(matches!(
            result,
            Err(ApiClientError::UnsupportedParameterValue { .. })
        ));
    }

    #[test]
    fn test_to_parameters_generates_correct_openapi_parameters() {
        let query = CallQuery::new()
            .add_param("name", ParamValue::new("test"))
            .add_param(
                "tags",
                ParamValue::with_style(vec!["a", "b"], ParamStyle::SpaceDelimited),
            )
            .add_param(
                "limit",
                ParamValue::with_style(10, ParamStyle::PipeDelimited),
            );

        let parameters: Vec<_> = query.to_parameters().collect();

        assert_eq!(parameters.len(), 3);

        // Check that all parameters are query parameters and not required
        for param in &parameters {
            assert_eq!(param.parameter_in, ParameterIn::Query);
            assert_eq!(param.required, Required::False);
            assert!(param.schema.is_some());
            // Style can be None for default parameters
        }

        // Check parameter names
        let param_names: std::collections::HashSet<_> =
            parameters.iter().map(|p| p.name.as_str()).collect();
        assert!(param_names.contains("name"));
        assert!(param_names.contains("tags"));
        assert!(param_names.contains("limit"));
    }

    #[test]
    fn test_comprehensive_query_serialization_snapshot() {
        // Test various data types with different styles
        let query = CallQuery::new()
            .add_param("search", ParamValue::new("hello world"))
            .add_param("active", ParamValue::new(true))
            .add_param("count", ParamValue::new(42))
            .add_param("tags", ParamValue::new(vec!["rust", "api", "web"]))
            .add_param(
                "categories",
                ParamValue::with_style(vec!["tech", "programming"], ParamStyle::SpaceDelimited),
            )
            .add_param(
                "ids",
                ParamValue::with_style(vec![1, 2, 3], ParamStyle::PipeDelimited),
            );

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""search=hello+world&active=true&count=42&tags=rust&tags=api&tags=web&categories=tech+programming&ids=1%7C2%7C3""#);
    }

    #[test]
    fn test_query_parameters_openapi_generation_snapshot() {
        let query = CallQuery::new()
            .add_param("q", ParamValue::new("search term"))
            .add_param(
                "filters",
                ParamValue::with_style(vec!["active", "verified"], ParamStyle::SpaceDelimited),
            )
            .add_param(
                "sort",
                ParamValue::with_style(vec!["name", "date"], ParamStyle::PipeDelimited),
            );

        let parameters: Vec<_> = query.to_parameters().collect();
        let debug_params: Vec<_> = parameters
            .iter()
            .map(|p| {
                format!(
                    "{}({:?})",
                    p.name,
                    p.style
                        .as_ref()
                        .unwrap_or(&utoipa::openapi::path::ParameterStyle::Form)
                )
            })
            .collect();

        insta::assert_debug_snapshot!(debug_params, @r#"
        [
            "q(Form)",
            "filters(SpaceDelimited)",
            "sort(PipeDelimited)",
        ]
        "#);
    }

    #[test]
    fn test_empty_and_null_values_snapshot() {
        let query = CallQuery::new()
            .add_param("empty", ParamValue::new(""))
            .add_param("nullable", ParamValue::new(serde_json::Value::Null));

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""empty=&nullable=""#);
    }

    #[test]
    fn test_special_characters_encoding_snapshot() {
        let query = CallQuery::new()
            .add_param("special", ParamValue::new("hello & goodbye"))
            .add_param("unicode", ParamValue::new("café résumé"))
            .add_param("symbols", ParamValue::new("100% guaranteed!"));

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""special=hello+%26+goodbye&unicode=caf%C3%A9+r%C3%A9sum%C3%A9&symbols=100%25+guaranteed%21""#);
    }
}
