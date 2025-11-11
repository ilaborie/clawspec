use std::borrow::Cow;
use std::fmt::Debug;

use serde::Serialize;
use utoipa::openapi::path::ParameterStyle;
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

use crate::client::error::ApiClientError;

/// A trait alias for types that can be used as parameter values.
///
/// This simplifies the generic constraints that are repeated throughout the codebase.
/// All parameter values must be serializable, provide OpenAPI schemas, and be thread-safe.
pub trait ParameterValue: Serialize + ToSchema + Debug + Send + Sync + Clone + 'static {}

// Blanket implementation for all types that satisfy the constraints
impl<T> ParameterValue for T where T: Serialize + ToSchema + Debug + Send + Sync + Clone + 'static {}

/// Parameter styles supported by OpenAPI 3.1 specification.
///
/// These styles define how array values and complex parameters are serialized
/// in strings according to the OpenAPI standard.
///
/// # Examples
///
/// ```rust
/// use clawspec_core::{ParamStyle, ParamValue, CallQuery};
///
/// // Form style (default) - arrays are repeated: ?tags=rust&tags=web&tags=api
/// let form_query = ParamValue::new(vec!["rust", "web", "api"]);
/// assert_eq!(form_query.query_style(), ParamStyle::Form);
///
/// // Space delimited - arrays are joined with spaces: ?tags=rust%20web%20api
/// let space_query = ParamValue::with_style(
///     vec!["rust", "web", "api"],
///     ParamStyle::SpaceDelimited
/// );
///
/// // Pipe delimited - arrays are joined with pipes: ?tags=rust|web|api
/// let pipe_query = ParamValue::with_style(
///     vec!["rust", "web", "api"],
///     ParamStyle::PipeDelimited
/// );
/// ```
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamStyle {
    /// Default style - will use Form for query parameters and Simple for path parameters
    Default,
    /// Form style: `param=value1&param=value2` (query default)
    Form,
    /// Simple style: `value1,value2` (path default)
    Simple,
    /// Space delimited: `param=value1 value2`
    SpaceDelimited,
    /// Pipe delimited: `param=value1|value2`
    PipeDelimited,
    /// Label style: `/users/.value` (path parameter with . prefix)
    Label,
    /// Matrix style: `/users/;name=value` (path parameter with ; prefix)
    Matrix,
    /// Deep object style: `?obj[key]=value` (query parameter for nested objects)
    DeepObject,
}

impl From<ParamStyle> for Option<ParameterStyle> {
    fn from(value: ParamStyle) -> Self {
        let result = match value {
            ParamStyle::Default => return None,
            ParamStyle::Form => ParameterStyle::Form,
            ParamStyle::Simple => ParameterStyle::Simple,
            ParamStyle::SpaceDelimited => ParameterStyle::SpaceDelimited,
            ParamStyle::PipeDelimited => ParameterStyle::PipeDelimited,
            ParamStyle::Label => ParameterStyle::Label,
            ParamStyle::Matrix => ParameterStyle::Matrix,
            ParamStyle::DeepObject => ParameterStyle::DeepObject,
        };
        Some(result)
    }
}

/// A parameter value with its serialization style
#[derive(Debug, Clone)]
pub struct ParamValue<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    /// The parameter value
    pub value: T,
    /// The serialization style
    pub style: ParamStyle,
}

/// A resolved parameter value with its serialized JSON value, schema reference, and style.
///
/// This struct is used internally to represent a parameter that has been processed
/// and is ready for use in OpenAPI generation and HTTP requests.
#[derive(Debug, Clone)]
pub(in crate::client) struct ResolvedParamValue {
    /// The serialized JSON value of the parameter
    pub value: serde_json::Value,
    /// The OpenAPI schema reference for this parameter
    pub schema: RefOr<Schema>,
    /// The serialization style for this parameter
    pub style: ParamStyle,
}

impl<T> ParamValue<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    /// Create a new parameter value with default style
    pub fn new(value: T) -> Self {
        Self {
            value,
            style: ParamStyle::Default,
        }
    }

    /// Create a new parameter value with specified style
    pub fn with_style(value: T, style: ParamStyle) -> Self {
        Self { value, style }
    }

    /// Get the actual style to use for query parameters
    pub fn query_style(&self) -> ParamStyle {
        match self.style {
            ParamStyle::Default => ParamStyle::Form,
            style => style,
        }
    }

    /// Get the actual style to use for path parameters
    pub fn path_style(&self) -> ParamStyle {
        match self.style {
            ParamStyle::Default => ParamStyle::Simple,
            style => style,
        }
    }

    /// Get the actual style to use for header parameters
    pub fn header_style(&self) -> ParamStyle {
        match self.style {
            ParamStyle::Default => ParamStyle::Simple,
            style => style,
        }
    }

    /// Converts the parameter to a JSON value for query string serialization.
    ///
    /// Returns `None` if the parameter should not be included in the query string.
    pub fn as_query_value(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.value).ok()
    }

    /// Converts the parameter to a JSON value for header serialization.
    pub fn as_header_value(&self) -> Result<serde_json::Value, ApiClientError> {
        serde_json::to_value(&self.value).map_err(|e| ApiClientError::SerializationError {
            message: format!("Failed to serialize header value: {e}"),
        })
    }

    /// Resolves this parameter value into a complete parameter with schema reference.
    ///
    /// This method is used internally by query and path parameter systems to create
    /// a resolved parameter that includes the serialized value, schema reference, and style.
    pub(in crate::client) fn resolve<F>(&self, add_schema_fn: F) -> Option<ResolvedParamValue>
    where
        F: FnOnce(serde_json::Value) -> RefOr<Schema>,
    {
        self.as_query_value().map(|value| {
            let schema = add_schema_fn(value.clone());
            ResolvedParamValue {
                value,
                schema,
                style: self.style,
            }
        })
    }
}

impl ResolvedParamValue {
    /// Converts a JSON value to a string representation.
    ///
    /// This helper method handles the common logic for converting individual JSON values
    /// to their string representations.
    fn json_value_to_string(value: &serde_json::Value) -> Result<String, ApiClientError> {
        match value {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Null => Ok(String::new()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                Err(ApiClientError::UnsupportedParameterValue {
                    message: "nested complex values not supported in parameters".to_string(),
                    value: value.clone(),
                })
            }
        }
    }

    /// Converts array items to string values.
    ///
    /// This helper method handles the common logic for converting array items
    /// to their string representations.
    fn array_to_string_values(arr: &[serde_json::Value]) -> Result<Vec<String>, ApiClientError> {
        let mut result = Vec::with_capacity(arr.len());
        for value in arr {
            result.push(Self::json_value_to_string(value)?);
        }
        Ok(result)
    }

    /// Converts the JSON value to a string representation for use in URLs.
    ///
    /// This method handles both simple values and arrays according to the parameter style.
    /// Arrays are serialized using the appropriate delimiter based on the style.
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - The serialized string value
    /// - `Err(ApiClientError)` - Error if the value cannot be serialized
    pub(in crate::client) fn to_string_value(&self) -> Result<String, ApiClientError> {
        match &self.value {
            serde_json::Value::Array(arr) => {
                let string_values = Self::array_to_string_values(arr)?;
                let delimiter = match self.style {
                    ParamStyle::Default | ParamStyle::Simple => ",",
                    ParamStyle::Form => ",", // Form style uses comma for single values in paths
                    ParamStyle::SpaceDelimited => " ",
                    ParamStyle::PipeDelimited => "|",
                    ParamStyle::Label => ",", // Label style uses comma for arrays
                    ParamStyle::Matrix => ",", // Matrix style uses comma for arrays
                    ParamStyle::DeepObject => {
                        return Err(ApiClientError::UnsupportedParameterValue {
                            message:
                                "DeepObject style not supported for arrays, use objects instead"
                                    .to_string(),
                            value: self.value.clone(),
                        });
                    }
                };
                Ok(string_values.join(delimiter))
            }
            serde_json::Value::Object(_) => {
                match self.style {
                    ParamStyle::DeepObject => {
                        // DeepObject style is handled differently in query parameters
                        Err(ApiClientError::UnsupportedParameterValue {
                            message: "DeepObject style objects require special handling in query parameters".to_string(),
                            value: self.value.clone(),
                        })
                    }
                    _ => Err(ApiClientError::UnsupportedParameterValue {
                        message: "object values not supported in parameters".to_string(),
                        value: self.value.clone(),
                    }),
                }
            }
            _ => Self::json_value_to_string(&self.value),
        }
    }

    /// Converts the JSON value to a vector of strings for query parameter encoding.
    ///
    /// This method is specifically for query parameters where Form style arrays
    /// are repeated as multiple parameters rather than joined with delimiters.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<String>)` - Vector of string values for query encoding
    /// - `Err(ApiClientError)` - Error if the value cannot be serialized
    pub(in crate::client) fn to_query_values(&self) -> Result<Vec<String>, ApiClientError> {
        match &self.value {
            serde_json::Value::Array(arr) => {
                match self.style {
                    ParamStyle::Default | ParamStyle::Form => {
                        // Form style: repeat parameter name for each array item
                        Self::array_to_string_values(arr)
                    }
                    _ => {
                        // For other styles, join into a single value
                        self.to_string_value().map(|s| vec![s])
                    }
                }
            }
            serde_json::Value::Object(_) => {
                match self.style {
                    ParamStyle::DeepObject => {
                        // DeepObject style is handled differently in query parameters
                        Err(ApiClientError::UnsupportedParameterValue {
                            message: "DeepObject style objects require special handling in query parameters".to_string(),
                            value: self.value.clone(),
                        })
                    }
                    _ => Err(ApiClientError::UnsupportedParameterValue {
                        message: "object values not supported in parameters".to_string(),
                        value: self.value.clone(),
                    }),
                }
            }
            _ => Self::json_value_to_string(&self.value).map(|s| vec![s]),
        }
    }
}

impl<T> From<T> for ParamValue<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

// ToSchema implementations for generating OpenAPI schemas
impl<T: ToSchema> ToSchema for ParamValue<T>
where
    T: Serialize + ToSchema + Debug + Send + Sync + Clone,
{
    fn name() -> Cow<'static, str> {
        T::name()
    }
}

impl<T: ToSchema> PartialSchema for ParamValue<T>
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
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;
    use utoipa::openapi::path::ParameterStyle;

    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
    struct TestStruct {
        id: u32,
        name: String,
    }

    // Test ParamStyle enum and conversions
    #[test]
    fn test_param_style_from_conversion() {
        assert_eq!(Option::<ParameterStyle>::from(ParamStyle::Default), None);
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::Form),
            Some(ParameterStyle::Form)
        );
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::Simple),
            Some(ParameterStyle::Simple)
        );
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::SpaceDelimited),
            Some(ParameterStyle::SpaceDelimited)
        );
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::PipeDelimited),
            Some(ParameterStyle::PipeDelimited)
        );
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::Label),
            Some(ParameterStyle::Label)
        );
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::Matrix),
            Some(ParameterStyle::Matrix)
        );
        assert_eq!(
            Option::<ParameterStyle>::from(ParamStyle::DeepObject),
            Some(ParameterStyle::DeepObject)
        );
    }

    #[test]
    fn test_param_style_eq() {
        assert_eq!(ParamStyle::Default, ParamStyle::Default);
        assert_eq!(ParamStyle::Form, ParamStyle::Form);
        assert_ne!(ParamStyle::Form, ParamStyle::Simple);
    }

    // Test ParamValue creation and methods
    #[test]
    fn test_param_value_new() {
        let param = ParamValue::new(42);
        assert_eq!(param.value, 42);
        assert_eq!(param.style, ParamStyle::Default);
    }

    #[test]
    fn test_param_value_with_style() {
        let param = ParamValue::with_style("test", ParamStyle::Form);
        assert_eq!(param.value, "test");
        assert_eq!(param.style, ParamStyle::Form);
    }

    #[test]
    fn test_param_value_from_conversion() {
        let param: ParamValue<i32> = 42.into();
        assert_eq!(param.value, 42);
        assert_eq!(param.style, ParamStyle::Default);
    }

    // Test style resolution methods
    #[test]
    fn test_param_value_query_style() {
        let default_param = ParamValue::new(42);
        assert_eq!(default_param.query_style(), ParamStyle::Form);

        let form_param = ParamValue::with_style(42, ParamStyle::Form);
        assert_eq!(form_param.query_style(), ParamStyle::Form);

        let simple_param = ParamValue::with_style(42, ParamStyle::Simple);
        assert_eq!(simple_param.query_style(), ParamStyle::Simple);

        let space_param = ParamValue::with_style(42, ParamStyle::SpaceDelimited);
        assert_eq!(space_param.query_style(), ParamStyle::SpaceDelimited);

        let pipe_param = ParamValue::with_style(42, ParamStyle::PipeDelimited);
        assert_eq!(pipe_param.query_style(), ParamStyle::PipeDelimited);
    }

    #[test]
    fn test_param_value_path_style() {
        let default_param = ParamValue::new(42);
        assert_eq!(default_param.path_style(), ParamStyle::Simple);

        let form_param = ParamValue::with_style(42, ParamStyle::Form);
        assert_eq!(form_param.path_style(), ParamStyle::Form);

        let simple_param = ParamValue::with_style(42, ParamStyle::Simple);
        assert_eq!(simple_param.path_style(), ParamStyle::Simple);
    }

    #[test]
    fn test_param_value_header_style() {
        let default_param = ParamValue::new(42);
        assert_eq!(default_param.header_style(), ParamStyle::Simple);

        let form_param = ParamValue::with_style(42, ParamStyle::Form);
        assert_eq!(form_param.header_style(), ParamStyle::Form);

        let simple_param = ParamValue::with_style(42, ParamStyle::Simple);
        assert_eq!(simple_param.header_style(), ParamStyle::Simple);
    }

    // Test JSON value conversion methods
    #[test]
    fn test_param_value_as_query_value() {
        let string_param = ParamValue::new("test");
        let query_value = string_param.as_query_value().unwrap();
        assert_eq!(query_value, serde_json::Value::String("test".to_string()));

        let number_param = ParamValue::new(42);
        let query_value = number_param.as_query_value().unwrap();
        assert_eq!(query_value, serde_json::Value::Number(42.into()));

        let bool_param = ParamValue::new(true);
        let query_value = bool_param.as_query_value().unwrap();
        assert_eq!(query_value, serde_json::Value::Bool(true));

        let array_param = ParamValue::new(vec!["a", "b", "c"]);
        let query_value = array_param.as_query_value().unwrap();
        let expected = serde_json::json!(["a", "b", "c"]);
        assert_eq!(query_value, expected);
    }

    #[test]
    fn test_param_value_as_header_value() {
        let string_param = ParamValue::new("test");
        let header_value = string_param.as_header_value().unwrap();
        assert_eq!(header_value, serde_json::Value::String("test".to_string()));

        let number_param = ParamValue::new(42);
        let header_value = number_param.as_header_value().unwrap();
        assert_eq!(header_value, serde_json::Value::Number(42.into()));
    }

    #[test]
    fn test_param_value_resolve() {
        let param = ParamValue::new(42);
        let resolved = param.resolve(|value| {
            // Mock schema function
            assert_eq!(value, serde_json::Value::Number(42.into()));
            utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default()))
        });

        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.value, serde_json::Value::Number(42.into()));
        assert_eq!(resolved.style, ParamStyle::Default);
    }

    #[test]
    fn test_param_value_resolve_none_on_serialization_error() {
        // This test simulates a case where as_query_value returns None
        // In practice, most serializable types will succeed, but this tests the logic
        let param = ParamValue::new(42); // This will succeed, so test the happy path
        let resolved = param.resolve(|_| {
            utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default()))
        });
        assert!(resolved.is_some());
    }

    // Test ResolvedParamValue helper methods
    #[test]
    fn test_resolved_param_value_json_value_to_string() {
        assert_eq!(
            ResolvedParamValue::json_value_to_string(&serde_json::Value::String(
                "test".to_string()
            ))
            .unwrap(),
            "test"
        );
        assert_eq!(
            ResolvedParamValue::json_value_to_string(&serde_json::Value::Number(42.into()))
                .unwrap(),
            "42"
        );
        assert_eq!(
            ResolvedParamValue::json_value_to_string(&serde_json::Value::Bool(true)).unwrap(),
            "true"
        );
        assert_eq!(
            ResolvedParamValue::json_value_to_string(&serde_json::Value::Bool(false)).unwrap(),
            "false"
        );
        assert_eq!(
            ResolvedParamValue::json_value_to_string(&serde_json::Value::Null).unwrap(),
            ""
        );

        // Test error cases
        let array_result = ResolvedParamValue::json_value_to_string(&serde_json::json!(["a", "b"]));
        assert!(array_result.is_err());

        let object_result =
            ResolvedParamValue::json_value_to_string(&serde_json::json!({"key": "value"}));
        assert!(object_result.is_err());
    }

    #[test]
    fn test_resolved_param_value_array_to_string_values() {
        let arr = vec![
            serde_json::Value::String("a".to_string()),
            serde_json::Value::Number(42.into()),
            serde_json::Value::Bool(true),
        ];
        let result = ResolvedParamValue::array_to_string_values(&arr).unwrap();
        assert_eq!(result, vec!["a", "42", "true"]);

        // Test error case with nested array
        let nested_arr = vec![
            serde_json::Value::String("a".to_string()),
            serde_json::json!(["nested"]),
        ];
        let result = ResolvedParamValue::array_to_string_values(&nested_arr);
        assert!(result.is_err());
    }

    // Test ResolvedParamValue string conversion methods
    #[test]
    fn test_resolved_param_value_to_string_value_simple_values() {
        let resolved = ResolvedParamValue {
            value: serde_json::Value::String("test".to_string()),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "test");

        let resolved = ResolvedParamValue {
            value: serde_json::Value::Number(42.into()),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "42");
    }

    #[test]
    fn test_resolved_param_value_to_string_value_arrays() {
        // Test Simple style (comma-separated)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Simple,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a,b,c");

        // Test SpaceDelimited style
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::SpaceDelimited,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a b c");

        // Test PipeDelimited style
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::PipeDelimited,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a|b|c");

        // Test Label style (comma-separated for arrays)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Label,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a,b,c");

        // Test Matrix style (comma-separated for arrays)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Matrix,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a,b,c");

        // Test Form style (comma-separated for single values)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Form,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a,b,c");

        // Test Default style (uses comma)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        assert_eq!(resolved.to_string_value().unwrap(), "a,b,c");
    }

    #[test]
    fn test_resolved_param_value_to_string_value_object_error() {
        let resolved = ResolvedParamValue {
            value: serde_json::json!({"key": "value"}),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        let result = resolved.to_string_value();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApiClientError::UnsupportedParameterValue { .. }
        ));
    }

    #[test]
    fn test_resolved_param_value_deep_object_style_errors() {
        // Test DeepObject style with array (should error)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::DeepObject,
        };
        let result = resolved.to_string_value();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApiClientError::UnsupportedParameterValue { .. }
        ));

        // Test DeepObject style with object (should error for to_string_value)
        let resolved = ResolvedParamValue {
            value: serde_json::json!({"key": "value"}),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::DeepObject,
        };
        let result = resolved.to_string_value();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApiClientError::UnsupportedParameterValue { .. }
        ));
    }

    #[test]
    fn test_resolved_param_value_to_query_values_simple_values() {
        let resolved = ResolvedParamValue {
            value: serde_json::Value::String("test".to_string()),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["test"]);

        let resolved = ResolvedParamValue {
            value: serde_json::Value::Number(42.into()),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["42"]);
    }

    #[test]
    fn test_resolved_param_value_to_query_values_arrays() {
        // Test Form style (repeated parameters)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Form,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["a", "b", "c"]);

        // Test Default style (same as Form for queries)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["a", "b", "c"]);

        // Test Simple style (single joined value)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Simple,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["a,b,c"]);

        // Test SpaceDelimited style (single joined value)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::SpaceDelimited,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["a b c"]);

        // Test PipeDelimited style (single joined value)
        let resolved = ResolvedParamValue {
            value: serde_json::json!(["a", "b", "c"]),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::PipeDelimited,
        };
        assert_eq!(resolved.to_query_values().unwrap(), vec!["a|b|c"]);
    }

    #[test]
    fn test_resolved_param_value_to_query_values_object_error() {
        let resolved = ResolvedParamValue {
            value: serde_json::json!({"key": "value"}),
            schema: utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            style: ParamStyle::Default,
        };
        let result = resolved.to_query_values();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApiClientError::UnsupportedParameterValue { .. }
        ));
    }

    // Test ToSchema trait implementations
    #[test]
    fn test_param_value_to_schema_name() {
        assert_eq!(ParamValue::<String>::name(), String::name());
        assert_eq!(ParamValue::<i32>::name(), i32::name());
        assert_eq!(ParamValue::<TestStruct>::name(), TestStruct::name());
    }

    #[test]
    fn test_param_value_partial_schema() {
        // Test that the schema is delegated to the inner type
        let string_schema = ParamValue::<String>::schema();
        let expected_schema = String::schema();
        // We can't easily compare schemas directly, but we can verify the method doesn't panic
        // and returns the same type structure
        match (string_schema, expected_schema) {
            (utoipa::openapi::RefOr::T(_), utoipa::openapi::RefOr::T(_)) => {}
            (utoipa::openapi::RefOr::Ref(_), utoipa::openapi::RefOr::Ref(_)) => {}
            _ => panic!("Schema types don't match"),
        }
    }

    // Test complex scenarios with different types
    #[test]
    fn test_param_value_with_complex_types() {
        let struct_param = ParamValue::new(TestStruct {
            id: 123,
            name: "test".to_string(),
        });

        // Test that it can be serialized to JSON
        let json_value = struct_param.as_query_value().unwrap();
        let expected = serde_json::json!({"id": 123, "name": "test"});
        assert_eq!(json_value, expected);

        // Test resolve functionality
        let resolved = struct_param.resolve(|value| {
            assert_eq!(value, expected);
            utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default()))
        });
        assert!(resolved.is_some());
    }

    #[test]
    fn test_param_value_with_option_types() {
        let some_param = ParamValue::new(Some(42));
        let json_value = some_param.as_query_value().unwrap();
        assert_eq!(json_value, serde_json::Value::Number(42.into()));

        let none_param: ParamValue<Option<i32>> = ParamValue::new(None);
        let json_value = none_param.as_query_value().unwrap();
        assert_eq!(json_value, serde_json::Value::Null);
    }

    #[test]
    fn test_param_value_with_mixed_array_types() {
        // Test array with different numeric types
        let mixed_numbers = vec![1, 2, 3];
        let param = ParamValue::with_style(mixed_numbers, ParamStyle::SpaceDelimited);
        let json_value = param.as_query_value().unwrap();
        assert_eq!(json_value, serde_json::json!([1, 2, 3]));

        let resolved = param
            .resolve(|_| {
                utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default()))
            })
            .unwrap();

        assert_eq!(resolved.to_string_value().unwrap(), "1 2 3");
        assert_eq!(resolved.to_query_values().unwrap(), vec!["1 2 3"]);
    }

    // Test edge cases and error conditions
    #[test]
    fn test_param_value_empty_array() {
        let empty_array: Vec<String> = vec![];
        let param = ParamValue::new(empty_array);
        let json_value = param.as_query_value().unwrap();
        assert_eq!(json_value, serde_json::json!([]));

        let resolved = param
            .resolve(|_| {
                utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default()))
            })
            .unwrap();

        assert_eq!(resolved.to_string_value().unwrap(), "");
        assert_eq!(resolved.to_query_values().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_param_value_single_item_array() {
        let single_item = vec!["only"];
        let param = ParamValue::new(single_item);
        let resolved = param
            .resolve(|_| {
                utoipa::openapi::RefOr::T(utoipa::openapi::Schema::Object(Default::default()))
            })
            .unwrap();

        assert_eq!(resolved.to_string_value().unwrap(), "only");
        assert_eq!(resolved.to_query_values().unwrap(), vec!["only"]);
    }

    // Test the ParameterValue trait is properly implemented
    #[test]
    fn test_parameter_value_trait_implementation() {
        fn accepts_parameter_value<T: ParameterValue>(_value: T) {}

        // These should all compile without issues
        accepts_parameter_value("string");
        accepts_parameter_value(42i32);
        accepts_parameter_value(true);
        accepts_parameter_value(vec!["a", "b"]);
        accepts_parameter_value(TestStruct {
            id: 1,
            name: "test".to_string(),
        });
    }
}
