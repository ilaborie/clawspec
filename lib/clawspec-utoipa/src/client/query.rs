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
//! use clawspec_utoipa::{CallQuery, QueryParam, QueryStyle};
//!
//! // Ergonomic usage - pass values directly (uses From<T> trait)
//! let query = CallQuery::new()
//!     .add_param("search", "hello world")
//!     .add_param("limit", 10)
//!     .add_param("active", true)
//!     .add_param("tags", vec!["rust", "web", "api"]);
//!
//! // Explicit QueryParam usage for custom styles
//! let query = CallQuery::new()
//!     .add_param("categories", QueryParam::with_style(
//!         vec!["tech", "programming"],
//!         QueryStyle::SpaceDelimited
//!     ));
//!
//! // This would generate a query string like:
//! // ?search=hello+world&limit=10&active=true&tags=rust&tags=web&tags=api&categories=tech+programming
//! ```
//!
//! # Parameter Types
//!
//! ## QueryParam
//!
//! Use [`QueryParam`] for any type that implements `Serialize` and `ToSchema`.
//! You can pass values directly or use explicit `QueryParam` wrappers:
//!
//! ```rust
//! # use clawspec_utoipa::{CallQuery, QueryParam, QueryStyle};
//! // Ergonomic usage - values are automatically wrapped
//! let query = CallQuery::new()
//!     .add_param("name", "John Doe")
//!     .add_param("age", 30)
//!     .add_param("active", true);
//!
//! // Explicit QueryParam for custom styles
//! let query = CallQuery::new()
//!     .add_param("tags", vec!["rust", "web"])  // Uses default Form style
//!     .add_param("ids", QueryParam::with_style(vec![1, 2, 3], QueryStyle::PipeDelimited));
//! ```
//!
//! # Query Styles
//!
//! The module supports three OpenAPI 3.1 query parameter styles:
//!
//! - **Form** (default): Arrays are repeated `?tags=a&tags=b&tags=c`
//! - **SpaceDelimited**: Arrays are joined with spaces `?tags=a%20b%20c`
//! - **PipeDelimited**: Arrays are joined with pipes `?tags=a|b|c`

use std::borrow::Cow;
use std::fmt::Debug;

use indexmap::IndexMap;
use serde::Serialize;
use utoipa::openapi::path::{Parameter, ParameterIn, ParameterStyle};
use utoipa::openapi::{RefOr, Required, Schema};
use utoipa::{PartialSchema, ToSchema};

use super::{ApiClientError, Schemas};

/// Query parameter styles supported by OpenAPI 3.1 specification.
///
/// These styles define how array values and complex parameters are serialized
/// in query strings according to the OpenAPI 3.1 standard.
///
/// # Examples
///
/// ```rust
/// use clawspec_utoipa::{QueryStyle, QueryParam, CallQuery};
///
/// // Form style (default) - arrays are repeated: ?tags=rust&tags=web&tags=api
/// let form_query = QueryParam::new(vec!["rust", "web", "api"]);
/// assert_eq!(form_query.query_style(), QueryStyle::Form);
///
/// // Space delimited - arrays are joined with spaces: ?tags=rust%20web%20api
/// let space_query = QueryParam::with_style(
///     vec!["rust", "web", "api"],
///     QueryStyle::SpaceDelimited
/// );
///
/// // Pipe delimited - arrays are joined with pipes: ?tags=rust|web|api
/// let pipe_query = QueryParam::with_style(
///     vec!["rust", "web", "api"],
///     QueryStyle::PipeDelimited
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryStyle {
    /// Form style (default): Arrays are repeated as separate parameters.
    ///
    /// Example: `?tags=rust&tags=web&tags=api`
    #[default]
    Form,
    /// Space delimited style: Array values are joined with spaces.
    ///
    /// Example: `?tags=rust%20web%20api` (spaces are URL-encoded as `%20` or `+`)
    SpaceDelimited,
    /// Pipe delimited style: Array values are joined with pipe characters.
    ///
    /// Example: `?tags=rust%7Cweb%7Capi` (pipes are URL-encoded as `%7C`)
    PipeDelimited,
}

impl From<QueryStyle> for ParameterStyle {
    fn from(value: QueryStyle) -> Self {
        match value {
            QueryStyle::Form => Self::Form,
            QueryStyle::SpaceDelimited => Self::SpaceDelimited,
            QueryStyle::PipeDelimited => Self::PipeDelimited,
        }
    }
}

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
/// use clawspec_utoipa::{CallQuery, QueryParam, QueryStyle};
///
/// let query = CallQuery::new()
///     .add_param("search", QueryParam::new("hello world"))
///     .add_param("limit", QueryParam::new(10))
///     .add_param("active", QueryParam::new(true));
///
/// // This would generate: ?search=hello+world&limit=10&active=true
/// ```
///
/// ## Array Parameters with Different Styles
///
/// ```rust
/// use clawspec_utoipa::{CallQuery, QueryParam, QueryStyle};
/// let query = CallQuery::new()
///     // Form style (default): ?tags=rust&tags=web&tags=api
///     .add_param("tags", QueryParam::new(vec!["rust", "web", "api"]))
///     // Space delimited: ?categories=tech%20programming
///     .add_param("categories", QueryParam::with_style(
///         vec!["tech", "programming"],
///         QueryStyle::SpaceDelimited
///     ))
///     // Pipe delimited: ?ids=1|2|3
///     .add_param("ids", QueryParam::with_style(
///         vec![1, 2, 3],
///         QueryStyle::PipeDelimited
///     ));
/// ```
///
/// # Type Safety
///
/// The query system is type-safe and will prevent invalid parameter types:
/// - Objects are not supported as query parameters (will return an error)
/// - All parameters must implement `Serialize` and `ToSchema` traits
/// - Parameters are automatically converted to appropriate string representations
#[derive(Debug, Default)]
pub struct CallQuery {
    params: IndexMap<String, (serde_json::Value, RefOr<Schema>, QueryStyle)>,
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
    /// - `param`: The parameter value that can be converted into `QueryParam<T>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{CallQuery, QueryParam, QueryStyle};
    ///
    /// // Ergonomic usage - pass values directly
    /// let query = CallQuery::new()
    ///     .add_param("search", "hello")
    ///     .add_param("limit", 10)
    ///     .add_param("active", true);
    ///
    /// // Explicit QueryParam usage for custom styles
    /// let query = CallQuery::new()
    ///     .add_param("tags", QueryParam::with_style(
    ///         vec!["rust", "web"],
    ///         QueryStyle::SpaceDelimited
    ///     ));
    /// ```
    pub fn add_param<T>(mut self, name: impl Into<String>, param: impl Into<QueryParam<T>>) -> Self
    where
        T: Serialize + ToSchema + Debug + Send + Sync + Clone + 'static,
    {
        let name = name.into();
        let param = param.into();
        if let Some(value) = param.as_query_value() {
            let schema_ref = self.schemas.add_example::<T>(value.clone());
            self.params.insert(name, (value, schema_ref, param.style));
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
        self.params.iter().map(|(name, (_, schema, style))| {
            // Create a simple string schema for the parameter

            Parameter::builder()
                .name(name)
                .parameter_in(ParameterIn::Query)
                .required(Required::False) // Query parameters are typically optional
                .schema(Some(schema.clone()))
                .style(Some((*style).into()))
                .build()
        })
    }

    /// Serialize query parameters to URL-encoded string
    pub(super) fn to_query_string(&self) -> Result<String, ApiClientError> {
        let mut pairs = Vec::new();

        for (name, (value, _, style)) in &self.params {
            match style {
                QueryStyle::Form => {
                    self.encode_form_style(name, value, &mut pairs)?;
                }
                QueryStyle::SpaceDelimited => {
                    self.encode_delimited_style(name, value, ' ', &mut pairs)?;
                }
                QueryStyle::PipeDelimited => {
                    self.encode_delimited_style(name, value, '|', &mut pairs)?;
                }
            }
        }

        serde_urlencoded::to_string(&pairs).map_err(ApiClientError::from)
    }

    /// Encode a parameter using form style (default)
    fn encode_form_style(
        &self,
        name: &str,
        value: &serde_json::Value,
        pairs: &mut Vec<(String, String)>,
    ) -> Result<(), ApiClientError> {
        match value {
            serde_json::Value::Array(arr) => {
                // For arrays in form style, repeat the parameter name
                for item in arr {
                    pairs.push((name.to_string(), self.value_to_string(item)?));
                }
            }
            serde_json::Value::String(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Bool(_)
            | serde_json::Value::Null => {
                pairs.push((name.to_string(), self.value_to_string(value)?));
            }
            serde_json::Value::Object(_) => {
                return Err(ApiClientError::UnsupportedQueryParameterValue {
                    value: value.clone(),
                });
            }
        }
        Ok(())
    }

    /// Encode a parameter using delimited style (space or pipe)
    fn encode_delimited_style(
        &self,
        name: &str,
        value: &serde_json::Value,
        delimiter: char,
        pairs: &mut Vec<(String, String)>,
    ) -> Result<(), ApiClientError> {
        match value {
            serde_json::Value::Array(arr) => {
                let mut values = Vec::new();
                for item in arr {
                    values.push(self.value_to_string(item)?);
                }
                let joined = values.join(&delimiter.to_string());
                pairs.push((name.to_string(), joined));
            }
            serde_json::Value::String(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Bool(_)
            | serde_json::Value::Null => {
                pairs.push((name.to_string(), self.value_to_string(value)?));
            }
            serde_json::Value::Object(_) => {
                return Err(ApiClientError::UnsupportedQueryParameterValue {
                    value: value.clone(),
                });
            }
        }
        Ok(())
    }

    /// Convert a JSON value to string representation
    fn value_to_string(&self, value: &serde_json::Value) -> Result<String, ApiClientError> {
        match value {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Null => Ok(String::new()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                Err(ApiClientError::UnsupportedQueryParameterValue {
                    value: value.clone(),
                })
            }
        }
    }
}

/// A query parameter with a typed value and configurable style.
///
/// This struct wraps any type that implements `Serialize` and `ToSchema` to be used
/// as a query parameter with OpenAPI 3.1 support.
///
/// # Type Parameters
///
/// - `T`: The underlying type that implements `Serialize` and `ToSchema`
///
/// # Examples
///
/// ```rust
/// use clawspec_utoipa::{QueryParam, QueryStyle};
/// use serde::Serialize;
/// use utoipa::ToSchema;
///
/// // Simple string parameter
/// let name_param = QueryParam::new("john");
///
/// // Array parameter with custom style
/// let tags_param = QueryParam::with_style(vec!["rust", "web"], QueryStyle::SpaceDelimited);
///
/// // Custom type parameter
/// #[derive(Debug, Clone, Serialize, ToSchema)]
/// struct User {
///     id: u32,
///     name: String,
/// }
///
/// let user = User { id: 1, name: "John".to_string() };
/// let user_param = QueryParam::new(user);
/// ```
#[derive(Debug, Clone)]
pub struct QueryParam<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    pub value: T,
    pub style: QueryStyle,
}

impl<T> QueryParam<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    /// Creates a new query parameter with the default form style.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{QueryParam, QueryStyle};
    ///
    /// let param = QueryParam::new("hello");
    /// assert_eq!(param.query_style(), QueryStyle::Form);
    /// ```
    pub fn new(value: T) -> Self {
        Self {
            value,
            style: QueryStyle::default(),
        }
    }

    /// Creates a new query parameter with the specified style.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{QueryParam, QueryStyle};
    ///
    /// let param = QueryParam::with_style(vec!["a", "b"], QueryStyle::PipeDelimited);
    /// assert_eq!(param.query_style(), QueryStyle::PipeDelimited);
    /// ```
    pub fn with_style(value: T, style: QueryStyle) -> Self {
        Self { value, style }
    }

    /// Returns the query style for this parameter.
    pub fn query_style(&self) -> QueryStyle {
        self.style
    }

    /// Converts the parameter to a JSON value for query string serialization.
    ///
    /// Returns `None` if the parameter should not be included in the query string.
    pub fn as_query_value(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.value).ok()
    }
}

impl<T> From<T> for QueryParam<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    /// Creates a `QueryParam` with the default form style from any compatible type.
    ///
    /// This allows for ergonomic usage where you can pass values directly to `add_param`
    /// without explicitly wrapping them in `QueryParam::new()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{CallQuery, QueryParam};
    ///
    /// // Both of these are equivalent:
    /// let query1 = CallQuery::new().add_param("name", "john");
    /// let query2 = CallQuery::new().add_param("name", QueryParam::new("john"));
    /// ```
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

// ToSchema implementations for generating OpenAPI schemas
impl<T: ToSchema> ToSchema for QueryParam<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    fn name() -> Cow<'static, str> {
        T::name()
    }
}

impl<T: ToSchema> PartialSchema for QueryParam<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    fn schema() -> RefOr<Schema> {
        T::schema()
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
            .add_param("name", QueryParam::new("test"))
            .add_param("age", QueryParam::new(25));

        assert!(!query.is_empty());
    }

    #[test]
    fn test_ergonomic_api_with_direct_values() {
        // Test that we can pass values directly without wrapping in QueryParam::new()
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
        // Test mixing direct values with explicit QueryParam wrappers
        let query = CallQuery::new()
            .add_param("name", "test") // Direct value
            .add_param("limit", 10) // Direct value
            .add_param(
                "tags",
                QueryParam::with_style(
                    // Explicit QueryParam with custom style
                    vec!["rust", "web"],
                    QueryStyle::SpaceDelimited,
                ),
            );

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=test&limit=10&tags=rust+web""#);
    }

    #[test]
    fn test_query_param_as_query_value() {
        let query = QueryParam::new("hello world");
        let value = query.as_query_value().expect("should have value");

        insta::assert_debug_snapshot!(value, @r#"String("hello world")"#);
    }

    #[test]
    fn test_query_param_with_different_styles() {
        let form_query = QueryParam::new("test");
        assert_eq!(form_query.style, QueryStyle::Form);

        let space_query = QueryParam::with_style("test", QueryStyle::SpaceDelimited);
        assert_eq!(space_query.style, QueryStyle::SpaceDelimited);

        let pipe_query = QueryParam::with_style("test", QueryStyle::PipeDelimited);
        assert_eq!(pipe_query.style, QueryStyle::PipeDelimited);
    }

    #[test]
    fn test_query_string_serialization_form_style() {
        let query = CallQuery::new()
            .add_param("name", QueryParam::new("john"))
            .add_param("age", QueryParam::new(25));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=john&age=25""#);
    }

    #[test]
    fn test_query_string_serialization_with_arrays() {
        let query = CallQuery::new().add_param("tags", QueryParam::new(vec!["rust", "web", "api"]));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust&tags=web&tags=api""#);
    }

    #[test]
    fn test_query_string_serialization_space_delimited() {
        let query = CallQuery::new().add_param(
            "tags",
            QueryParam::with_style(vec!["rust", "web", "api"], QueryStyle::SpaceDelimited),
        );

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust+web+api""#);
    }

    #[test]
    fn test_query_string_serialization_pipe_delimited() {
        let query = CallQuery::new().add_param(
            "tags",
            QueryParam::with_style(vec!["rust", "web", "api"], QueryStyle::PipeDelimited),
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
            .add_param("name", QueryParam::new("john"))
            .add_param("active", QueryParam::new(true))
            .add_param("scores", QueryParam::new(vec![10, 20, 30]));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=john&active=true&scores=10&scores=20&scores=30""#);
    }

    #[test]
    fn test_object_query_parameter_error() {
        use serde_json::json;

        let query = CallQuery::new().add_param("config", QueryParam::new(json!({"key": "value"})));

        let result = query.to_query_string();
        assert!(matches!(
            result,
            Err(ApiClientError::UnsupportedQueryParameterValue { .. })
        ));
    }

    #[test]
    fn test_nested_object_in_array_error() {
        use serde_json::json;

        let query = CallQuery::new().add_param(
            "items",
            QueryParam::new(json!(["valid", {"nested": "object"}])),
        );

        let result = query.to_query_string();
        assert!(matches!(
            result,
            Err(ApiClientError::UnsupportedQueryParameterValue { .. })
        ));
    }

    #[test]
    fn test_to_parameters_generates_correct_openapi_parameters() {
        let query = CallQuery::new()
            .add_param("name", QueryParam::new("test"))
            .add_param(
                "tags",
                QueryParam::with_style(vec!["a", "b"], QueryStyle::SpaceDelimited),
            )
            .add_param(
                "limit",
                QueryParam::with_style(10, QueryStyle::PipeDelimited),
            );

        let parameters: Vec<_> = query.to_parameters().collect();

        assert_eq!(parameters.len(), 3);

        // Check that all parameters are query parameters and not required
        for param in &parameters {
            assert_eq!(param.parameter_in, ParameterIn::Query);
            assert_eq!(param.required, Required::False);
            assert!(param.schema.is_some());
            assert!(param.style.is_some());
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
            .add_param("search", QueryParam::new("hello world"))
            .add_param("active", QueryParam::new(true))
            .add_param("count", QueryParam::new(42))
            .add_param("tags", QueryParam::new(vec!["rust", "api", "web"]))
            .add_param(
                "categories",
                QueryParam::with_style(vec!["tech", "programming"], QueryStyle::SpaceDelimited),
            )
            .add_param(
                "ids",
                QueryParam::with_style(vec![1, 2, 3], QueryStyle::PipeDelimited),
            );

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""search=hello+world&active=true&count=42&tags=rust&tags=api&tags=web&categories=tech+programming&ids=1%7C2%7C3""#);
    }

    #[test]
    fn test_query_parameters_openapi_generation_snapshot() {
        let query = CallQuery::new()
            .add_param("q", QueryParam::new("search term"))
            .add_param(
                "filters",
                QueryParam::with_style(vec!["active", "verified"], QueryStyle::SpaceDelimited),
            )
            .add_param(
                "sort",
                QueryParam::with_style(vec!["name", "date"], QueryStyle::PipeDelimited),
            );

        let parameters: Vec<_> = query.to_parameters().collect();
        let debug_params: Vec<_> = parameters
            .iter()
            .map(|p| format!("{}({:?})", p.name, p.style.as_ref().unwrap()))
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
            .add_param("empty", QueryParam::new(""))
            .add_param("nullable", QueryParam::new(serde_json::Value::Null));

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""empty=&nullable=""#);
    }

    #[test]
    fn test_special_characters_encoding_snapshot() {
        let query = CallQuery::new()
            .add_param("special", QueryParam::new("hello & goodbye"))
            .add_param("unicode", QueryParam::new("café résumé"))
            .add_param("symbols", QueryParam::new("100% guaranteed!"));

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""special=hello+%26+goodbye&unicode=caf%C3%A9+r%C3%A9sum%C3%A9&symbols=100%25+guaranteed%21""#);
    }
}
