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
//! use clawspec_utoipa::{CallQuery, DisplayQuery, SerializableQuery, QueryStyle};
//!
//! // Build a query with mixed parameter types
//! let query = CallQuery::new()
//!     .add_param("search", DisplayQuery("hello world"))
//!     .add_param("limit", DisplayQuery(10))
//!     .add_param("active", DisplayQuery(true))
//!     .add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]))
//!     .add_param("categories", SerializableQuery::with_style(
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
//! ## DisplayQuery
//!
//! Use [`DisplayQuery`] for simple types that implement [`std::fmt::Display`]:
//!
//! ```rust
//! # use clawspec_utoipa::{CallQuery, DisplayQuery};
//! let query = CallQuery::new()
//!     .add_param("name", DisplayQuery("John Doe"))
//!     .add_param("age", DisplayQuery(30))
//!     .add_param("active", DisplayQuery(true));
//! ```
//!
//! ## SerializableQuery
//!
//! Use [`SerializableQuery`] for complex types like arrays or custom structs:
//!
//! ```rust
//! # use clawspec_utoipa::{CallQuery, SerializableQuery, QueryStyle};
//! let query = CallQuery::new()
//!     .add_param("tags", SerializableQuery::new(vec!["rust", "web"]))
//!     .add_param("ids", SerializableQuery::with_style(vec![1, 2, 3], QueryStyle::PipeDelimited));
//! ```
//!
//! # Query Styles
//!
//! The module supports three OpenAPI 3.1 query parameter styles:
//!
//! - **Form** (default): Arrays are repeated `?tags=a&tags=b&tags=c`
//! - **SpaceDelimited**: Arrays are joined with spaces `?tags=a%20b%20c`
//! - **PipeDelimited**: Arrays are joined with pipes `?tags=a|b|c`

use std::fmt::{Debug, Display};

use indexmap::IndexMap;
use serde::Serialize;
use std::borrow::Cow;
use utoipa::openapi::path::{Parameter, ParameterIn, ParameterStyle};
use utoipa::openapi::schema::{ObjectBuilder, Type};
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
/// use clawspec_utoipa::{QueryStyle, SerializableQuery, CallQuery, QueryParam};
///
/// // Form style (default) - arrays are repeated: ?tags=rust&tags=web&tags=api
/// let form_query = SerializableQuery::new(vec!["rust", "web", "api"]);
/// assert_eq!(form_query.query_style(), QueryStyle::Form);
///
/// // Space delimited - arrays are joined with spaces: ?tags=rust%20web%20api
/// let space_query = SerializableQuery::with_style(
///     vec!["rust", "web", "api"],
///     QueryStyle::SpaceDelimited
/// );
///
/// // Pipe delimited - arrays are joined with pipes: ?tags=rust|web|api
/// let pipe_query = SerializableQuery::with_style(
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
/// use clawspec_utoipa::{CallQuery, DisplayQuery, SerializableQuery, QueryStyle};
///
/// let query = CallQuery::new()
///     .add_param("search", DisplayQuery("hello world"))
///     .add_param("limit", DisplayQuery(10))
///     .add_param("active", DisplayQuery(true));
///
/// // This would generate: ?search=hello+world&limit=10&active=true
/// ```
///
/// ## Array Parameters with Different Styles
///
/// ```rust
/// # use clawspec_utoipa::{CallQuery, SerializableQuery, QueryStyle};
/// let query = CallQuery::new()
///     // Form style (default): ?tags=rust&tags=web&tags=api
///     .add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]))
///     // Space delimited: ?categories=tech%20programming
///     .add_param("categories", SerializableQuery::with_style(
///         vec!["tech", "programming"],
///         QueryStyle::SpaceDelimited
///     ))
///     // Pipe delimited: ?ids=1|2|3
///     .add_param("ids", SerializableQuery::with_style(
///         vec![1, 2, 3],
///         QueryStyle::PipeDelimited
///     ));
/// ```
///
/// # Type Safety
///
/// The query system is type-safe and will prevent invalid parameter types:
/// - Objects are not supported as query parameters (will return an error)
/// - All parameters must implement the `QueryParam` trait
/// - Parameters are automatically converted to appropriate string representations
#[derive(Debug, Default)]
pub struct CallQuery {
    params: IndexMap<String, Box<dyn QueryParam>>,
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
    /// allowing for method chaining. The parameter must implement both `QueryParam` and
    /// `ToSchema` traits for proper serialization and OpenAPI schema generation.
    ///
    /// # Parameters
    ///
    /// - `name`: The parameter name (will be converted to `String`)
    /// - `param`: The parameter value implementing `QueryParam + ToSchema`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{CallQuery, DisplayQuery, SerializableQuery, QueryStyle};
    ///
    /// let query = CallQuery::new()
    ///     .add_param("search", DisplayQuery("hello"))
    ///     .add_param("limit", DisplayQuery(10))
    ///     .add_param("tags", SerializableQuery::with_style(
    ///         vec!["rust", "web"],
    ///         QueryStyle::SpaceDelimited
    ///     ));
    /// ```
    pub fn add_param<Q>(mut self, name: impl Into<String>, param: Q) -> Self
    where
        Q: QueryParam + ToSchema + 'static,
    {
        let name = name.into();
        let example = param.as_query_value();
        self.params.insert(name, Box::new(param));
        self.schemas.add_example::<Q>(example);
        self
    }

    /// Check if the query is empty
    pub(super) fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Get query parameters with their styles for OpenAPI generation
    pub(super) fn params_with_styles(&self) -> impl Iterator<Item = (&str, QueryStyle)> + '_ {
        self.params
            .iter()
            .map(|(name, param)| (name.as_str(), param.query_style()))
    }

    /// Convert query parameters to OpenAPI Parameters
    pub(super) fn to_parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        // For query parameters, we need to create a schema for each parameter
        // The current schema system is type-based, but for query parameters,
        // we need to create individual parameter schemas
        self.params_with_styles().map(|(param_name, param_style)| {
            // Create a simple string schema for the parameter
            // In a more sophisticated implementation, we might want to
            // extract the actual schema from the QueryParam trait
            let schema = Schema::Object(ObjectBuilder::new().schema_type(Type::String).build());

            Parameter::builder()
                .name(param_name)
                .parameter_in(ParameterIn::Query)
                .required(Required::False) // Query parameters are typically optional
                .schema(Some(RefOr::T(schema)))
                .style(Some(param_style.into()))
                .build()
        })
    }

    /// Serialize query parameters to URL-encoded string
    pub(super) fn to_query_string(&self) -> Result<String, ApiClientError> {
        let mut pairs = Vec::new();

        for (name, param) in &self.params {
            if let Some(value) = param.as_query_value() {
                match param.query_style() {
                    QueryStyle::Form => {
                        self.encode_form_style(name, &value, &mut pairs)?;
                    }
                    QueryStyle::SpaceDelimited => {
                        self.encode_delimited_style(name, &value, ' ', &mut pairs)?;
                    }
                    QueryStyle::PipeDelimited => {
                        self.encode_delimited_style(name, &value, '|', &mut pairs)?;
                    }
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

/// Trait for types that can be used as query parameters.
///
/// This trait defines the interface for converting Rust types into query string
/// values that can be serialized according to OpenAPI 3.1 standards.
///
/// # Implementation Notes
///
/// - The `as_query_value()` method should return `None` for parameters that
///   shouldn't be included in the query string (e.g., when the value is `None`)
/// - The `query_style()` method defaults to `QueryStyle::Form` but can be
///   overridden for custom serialization behavior
/// - All query parameters must be `Debug + Send + Sync` for thread safety
///
/// # Examples
///
/// ```rust
/// use clawspec_utoipa::{QueryParam, QueryStyle};
/// use std::fmt::Debug;
///
/// #[derive(Debug)]
/// struct CustomParam {
///     value: String,
///     style: QueryStyle,
/// }
///
/// impl QueryParam for CustomParam {
///     fn as_query_value(&self) -> Option<serde_json::Value> {
///         Some(serde_json::Value::String(self.value.clone()))
///     }
///
///     fn query_style(&self) -> QueryStyle {
///         self.style
///     }
/// }
/// ```
pub trait QueryParam: Debug + Send + Sync {
    /// Converts the parameter to a JSON value for query string serialization.
    ///
    /// Returns `None` if the parameter should not be included in the query string.
    /// The returned JSON value will be converted to a string representation according
    /// to the parameter's query style.
    ///
    /// # Supported JSON Types
    ///
    /// - `String`: Used as-is
    /// - `Number`: Converted to string representation
    /// - `Bool`: Converted to "true" or "false"
    /// - `Null`: Converted to empty string
    /// - `Array`: Serialized according to the query style (form/space/pipe delimited)
    /// - `Object`: **Not supported** - will result in an error
    fn as_query_value(&self) -> Option<serde_json::Value>;

    /// Returns the query style for this parameter.
    ///
    /// The default implementation returns `QueryStyle::Form`. Override this method
    /// to specify different serialization behavior for array values.
    fn query_style(&self) -> QueryStyle {
        QueryStyle::Form
    }
}

/// A simple wrapper for types implementing `Display` to be used as query parameters.
///
/// This is the easiest way to convert basic types like strings, numbers, and booleans
/// into query parameters. The wrapped value will be converted to a string using its
/// `Display` implementation.
///
/// # Examples
///
/// ```rust
/// use clawspec_utoipa::{CallQuery, DisplayQuery};
///
/// let query = CallQuery::new()
///     .add_param("name", DisplayQuery("John Doe"))
///     .add_param("age", DisplayQuery(30))
///     .add_param("active", DisplayQuery(true));
///
/// // This generates: ?name=John+Doe&age=30&active=true
/// ```
///
/// # Common Use Cases
///
/// - String literals: `DisplayQuery("hello")`
/// - Numbers: `DisplayQuery(42)`, `DisplayQuery(3.14)`
/// - Booleans: `DisplayQuery(true)`
/// - Any type implementing `Display`
#[derive(Debug, Clone)]
pub struct DisplayQuery<T>(pub T);

impl<T> QueryParam for DisplayQuery<T>
where
    T: Display + Debug + Send + Sync + Clone,
{
    fn as_query_value(&self) -> Option<serde_json::Value> {
        Some(serde_json::Value::String(self.0.to_string()))
    }
}

/// A wrapper for serializable types to be used as query parameters with configurable styles.
///
/// This wrapper is designed for complex types like arrays, vectors, and custom structs
/// that implement `Serialize`. It allows you to specify the query parameter style,
/// which is particularly useful for array serialization.
///
/// # Examples
///
/// ## Basic Usage with Arrays
///
/// ```rust
/// use clawspec_utoipa::{CallQuery, SerializableQuery, QueryStyle};
///
/// let query = CallQuery::new()
///     // Form style (default): ?tags=rust&tags=web&tags=api
///     .add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]))
///     // Space delimited: ?categories=tech%20programming
///     .add_param("categories", SerializableQuery::with_style(
///         vec!["tech", "programming"],
///         QueryStyle::SpaceDelimited
///     ))
///     // Pipe delimited: ?ids=1|2|3
///     .add_param("ids", SerializableQuery::with_style(
///         vec![1, 2, 3],
///         QueryStyle::PipeDelimited
///     ));
/// ```
///
/// ## Custom Serializable Types
///
/// ```rust
/// use serde::Serialize;
/// use clawspec_utoipa::{SerializableQuery, QueryStyle};
///
/// #[derive(Serialize)]
/// struct Filters {
///     active: bool,
///     category: String,
/// }
///
/// let filters = Filters { active: true, category: "tech".to_string() };
/// let param = SerializableQuery::new(filters);
/// // Note: Objects are not supported and will result in an error
/// ```
#[derive(Debug, Clone)]
pub struct SerializableQuery<T> {
    pub value: T,
    pub style: QueryStyle,
}

impl<T> SerializableQuery<T> {
    /// Creates a new serializable query parameter with the default form style.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{SerializableQuery, QueryStyle, QueryParam};
    ///
    /// let param = SerializableQuery::new(vec!["a", "b", "c"]);
    /// assert_eq!(param.query_style(), QueryStyle::Form);
    /// ```
    pub fn new(value: T) -> Self {
        Self {
            value,
            style: QueryStyle::default(),
        }
    }

    /// Creates a new serializable query parameter with the specified style.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_utoipa::{SerializableQuery, QueryStyle, QueryParam};
    ///
    /// let param = SerializableQuery::with_style(
    ///     vec!["a", "b", "c"],
    ///     QueryStyle::PipeDelimited
    /// );
    /// assert_eq!(param.query_style(), QueryStyle::PipeDelimited);
    /// ```
    pub fn with_style(value: T, style: QueryStyle) -> Self {
        Self { value, style }
    }
}

impl<T> QueryParam for SerializableQuery<T>
where
    T: Serialize + Debug + Send + Sync + Clone,
{
    fn as_query_value(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.value).ok()
    }

    fn query_style(&self) -> QueryStyle {
        self.style
    }
}

// ToSchema implementations for generating OpenAPI schemas
impl<T: ToSchema> ToSchema for DisplayQuery<T>
where
    T: Display + Debug + Send + Sync + Clone,
{
    fn name() -> Cow<'static, str> {
        T::name()
    }
}

impl<T: ToSchema> PartialSchema for DisplayQuery<T>
where
    T: Display + Debug + Send + Sync + Clone,
{
    fn schema() -> RefOr<Schema> {
        T::schema()
    }
}

impl<T: ToSchema> ToSchema for SerializableQuery<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    fn name() -> Cow<'static, str> {
        T::name()
    }
}

impl<T: ToSchema> PartialSchema for SerializableQuery<T>
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
            .add_param("name", DisplayQuery("test"))
            .add_param("age", DisplayQuery(25));

        assert!(!query.is_empty());
    }

    #[test]
    fn test_display_query_as_query_value() {
        let query = DisplayQuery("hello world");
        let value = query.as_query_value().expect("should have value");

        insta::assert_debug_snapshot!(value, @r#"String("hello world")"#);
    }

    #[test]
    fn test_serializable_query_with_different_styles() {
        let form_query = SerializableQuery::new("test");
        assert_eq!(form_query.query_style(), QueryStyle::Form);

        let space_query = SerializableQuery::with_style("test", QueryStyle::SpaceDelimited);
        assert_eq!(space_query.query_style(), QueryStyle::SpaceDelimited);

        let pipe_query = SerializableQuery::with_style("test", QueryStyle::PipeDelimited);
        assert_eq!(pipe_query.query_style(), QueryStyle::PipeDelimited);
    }

    #[test]
    fn test_query_string_serialization_form_style() {
        let query = CallQuery::new()
            .add_param("name", DisplayQuery("john"))
            .add_param("age", DisplayQuery(25));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=john&age=25""#);
    }

    #[test]
    fn test_query_string_serialization_with_arrays() {
        let query =
            CallQuery::new().add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust&tags=web&tags=api""#);
    }

    #[test]
    fn test_query_string_serialization_space_delimited() {
        let query = CallQuery::new().add_param(
            "tags",
            SerializableQuery::with_style(vec!["rust", "web", "api"], QueryStyle::SpaceDelimited),
        );

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""tags=rust+web+api""#);
    }

    #[test]
    fn test_query_string_serialization_pipe_delimited() {
        let query = CallQuery::new().add_param(
            "tags",
            SerializableQuery::with_style(vec!["rust", "web", "api"], QueryStyle::PipeDelimited),
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
            .add_param("name", DisplayQuery("john"))
            .add_param("active", DisplayQuery(true))
            .add_param("scores", SerializableQuery::new(vec![10, 20, 30]));

        let query_string = query.to_query_string().expect("should serialize");
        insta::assert_debug_snapshot!(query_string, @r#""name=john&active=true&scores=10&scores=20&scores=30""#);
    }

    #[test]
    fn test_object_query_parameter_error() {
        use serde_json::json;

        let query =
            CallQuery::new().add_param("config", SerializableQuery::new(json!({"key": "value"})));

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
            SerializableQuery::new(json!(["valid", {"nested": "object"}])),
        );

        let result = query.to_query_string();
        assert!(matches!(
            result,
            Err(ApiClientError::UnsupportedQueryParameterValue { .. })
        ));
    }

    #[test]
    fn test_params_with_styles_iterator() {
        let query = CallQuery::new()
            .add_param("name", DisplayQuery("test"))
            .add_param(
                "tags",
                SerializableQuery::with_style(vec!["a", "b"], QueryStyle::SpaceDelimited),
            )
            .add_param(
                "ids",
                SerializableQuery::with_style(vec![1, 2], QueryStyle::PipeDelimited),
            );

        let styles: std::collections::HashMap<_, _> = query.params_with_styles().collect();

        assert_eq!(styles.get("name"), Some(&QueryStyle::Form));
        assert_eq!(styles.get("tags"), Some(&QueryStyle::SpaceDelimited));
        assert_eq!(styles.get("ids"), Some(&QueryStyle::PipeDelimited));
        assert_eq!(styles.len(), 3);
    }

    #[test]
    fn test_to_parameters_generates_correct_openapi_parameters() {
        let query = CallQuery::new()
            .add_param("name", DisplayQuery("test"))
            .add_param(
                "tags",
                SerializableQuery::with_style(vec!["a", "b"], QueryStyle::SpaceDelimited),
            )
            .add_param(
                "limit",
                SerializableQuery::with_style(10, QueryStyle::PipeDelimited),
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
            .add_param("search", DisplayQuery("hello world"))
            .add_param("active", DisplayQuery(true))
            .add_param("count", DisplayQuery(42))
            .add_param("tags", SerializableQuery::new(vec!["rust", "api", "web"]))
            .add_param(
                "categories",
                SerializableQuery::with_style(
                    vec!["tech", "programming"],
                    QueryStyle::SpaceDelimited,
                ),
            )
            .add_param(
                "ids",
                SerializableQuery::with_style(vec![1, 2, 3], QueryStyle::PipeDelimited),
            );

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""search=hello+world&active=true&count=42&tags=rust&tags=api&tags=web&categories=tech+programming&ids=1%7C2%7C3""#);
    }

    #[test]
    fn test_query_parameters_openapi_generation_snapshot() {
        let query = CallQuery::new()
            .add_param("q", DisplayQuery("search term"))
            .add_param(
                "filters",
                SerializableQuery::with_style(
                    vec!["active", "verified"],
                    QueryStyle::SpaceDelimited,
                ),
            )
            .add_param(
                "sort",
                SerializableQuery::with_style(vec!["name", "date"], QueryStyle::PipeDelimited),
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
            .add_param("empty", DisplayQuery(""))
            .add_param("nullable", SerializableQuery::new(serde_json::Value::Null));

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""empty=&nullable=""#);
    }

    #[test]
    fn test_special_characters_encoding_snapshot() {
        let query = CallQuery::new()
            .add_param("special", DisplayQuery("hello & goodbye"))
            .add_param("unicode", DisplayQuery("café résumé"))
            .add_param("symbols", DisplayQuery("100% guaranteed!"));

        let query_string = query
            .to_query_string()
            .expect("serialization should succeed");
        insta::assert_debug_snapshot!(query_string, @r#""special=hello+%26+goodbye&unicode=caf%C3%A9+r%C3%A9sum%C3%A9&symbols=100%25+guaranteed%21""#);
    }
}
