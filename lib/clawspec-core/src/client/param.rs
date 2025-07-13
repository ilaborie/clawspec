use std::borrow::Cow;
use std::fmt::Debug;

use serde::Serialize;
use utoipa::openapi::path::ParameterStyle;
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

use super::ApiClientError;

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
}

impl From<ParamStyle> for Option<ParameterStyle> {
    fn from(value: ParamStyle) -> Self {
        let result = match value {
            ParamStyle::Default => return None,
            ParamStyle::Form => ParameterStyle::Form,
            ParamStyle::Simple => ParameterStyle::Simple,
            ParamStyle::SpaceDelimited => ParameterStyle::SpaceDelimited,
            ParamStyle::PipeDelimited => ParameterStyle::PipeDelimited,
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
pub(super) struct ResolvedParamValue {
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
    pub(super) fn resolve<F>(&self, add_schema_fn: F) -> Option<ResolvedParamValue>
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
        arr.iter()
            .map(Self::json_value_to_string)
            .collect::<Result<Vec<_>, _>>()
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
    pub(super) fn to_string_value(&self) -> Result<String, ApiClientError> {
        match &self.value {
            serde_json::Value::Array(arr) => {
                let string_values = Self::array_to_string_values(arr)?;
                let delimiter = match self.style {
                    ParamStyle::Default | ParamStyle::Simple => ",",
                    ParamStyle::Form => ",", // Form style uses comma for single values in paths
                    ParamStyle::SpaceDelimited => " ",
                    ParamStyle::PipeDelimited => "|",
                };
                Ok(string_values.join(delimiter))
            }
            serde_json::Value::Object(_) => Err(ApiClientError::UnsupportedParameterValue {
                message: "object values not supported in parameters".to_string(),
                value: self.value.clone(),
            }),
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
    pub(super) fn to_query_values(&self) -> Result<Vec<String>, ApiClientError> {
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
            serde_json::Value::Object(_) => Err(ApiClientError::UnsupportedParameterValue {
                message: "object values not supported in parameters".to_string(),
                value: self.value.clone(),
            }),
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
