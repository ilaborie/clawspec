use indexmap::IndexMap;
use utoipa::openapi::Required;
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};

use super::param::{ParamValue, ParameterValue, ResolvedParamValue};
use crate::client::error::ApiClientError;
use crate::client::openapi::schema::Schemas;

/// Represents HTTP cookies for an API call.
///
/// This struct manages HTTP cookies using the same ParamValue pattern as query, path, and header parameters,
/// allowing for type-safe cookie handling with automatic OpenAPI schema generation.
///
/// # Examples
///
/// ```rust
/// use clawspec_core::CallCookies;
///
/// let cookies = CallCookies::new()
///     .add_cookie("session_id", "abc123")
///     .add_cookie("user_id", 12345)
///     .add_cookie("preferences", "dark_mode=true");
/// ```
///
/// # OpenAPI Integration
///
/// Cookies are automatically documented in the OpenAPI specification with `in: cookie` parameter type.
/// This follows the OpenAPI 3.1.0 specification for cookie parameters.
#[derive(Debug, Clone, Default)]
pub struct CallCookies {
    cookies: IndexMap<String, ResolvedParamValue>,
    pub(in crate::client) schemas: Schemas,
}

impl CallCookies {
    /// Creates a new empty CallCookies instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallCookies;
    ///
    /// let cookies = CallCookies::new();
    /// assert!(cookies.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a cookie parameter to the collection.
    ///
    /// This method follows the same pattern as `CallHeaders::add_header` and `CallQuery::add_param`,
    /// providing a consistent API across all parameter types.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The cookie value type, must implement `Serialize`, `ToSchema`, `Debug`, `Send`, `Sync`, and `Clone`
    ///
    /// # Arguments
    ///
    /// * `name` - The cookie name (e.g., "session_id", "user_preferences")
    /// * `value` - The cookie value, either a direct value or wrapped in ParamValue
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallCookies;
    ///
    /// let cookies = CallCookies::new()
    ///     .add_cookie("session_id", "abc123")
    ///     .add_cookie("user_id", 12345)
    ///     .add_cookie("is_admin", true);
    /// ```
    pub fn add_cookie<T: ParameterValue>(
        mut self,
        name: impl Into<String>,
        value: impl Into<ParamValue<T>>,
    ) -> Self {
        let name = name.into();
        let param_value = value.into();

        // Generate schema for the cookie value
        let schema = self.schemas.add::<T>();

        // Convert to resolved param value
        let resolved = ResolvedParamValue {
            value: param_value
                .as_query_value()
                .expect("Cookie serialization should not fail"),
            schema,
            style: param_value.query_style(), // Cookies use simple string serialization like query params
        };

        self.cookies.insert(name, resolved);
        self
    }

    /// Merges another CallCookies instance into this one.
    ///
    /// Cookies from the other instance will override cookies with the same name in this instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallCookies;
    ///
    /// let cookies1 = CallCookies::new()
    ///     .add_cookie("session_id", "abc123");
    ///
    /// let cookies2 = CallCookies::new()
    ///     .add_cookie("user_id", 456);
    ///
    /// let merged = cookies1.merge(cookies2);
    /// assert_eq!(merged.len(), 2);
    /// ```
    pub fn merge(mut self, other: Self) -> Self {
        // Merge schemas first
        self.schemas.merge(other.schemas);

        // Merge cookies (other takes precedence)
        for (name, value) in other.cookies {
            self.cookies.insert(name, value);
        }

        self
    }

    /// Checks if the cookies collection is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallCookies;
    ///
    /// let cookies = CallCookies::new();
    /// assert!(cookies.is_empty());
    ///
    /// let cookies = cookies.add_cookie("session_id", "abc123");
    /// assert!(!cookies.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Returns the number of cookies.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallCookies;
    ///
    /// let cookies = CallCookies::new()
    ///     .add_cookie("session_id", "abc123")
    ///     .add_cookie("user_id", 456);
    /// assert_eq!(cookies.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    /// Converts cookies to OpenAPI Parameter objects.
    ///
    /// According to the OpenAPI 3.1.0 specification, cookies are represented as parameters
    /// with `in: cookie`. This method generates the appropriate Parameter objects for
    /// inclusion in the OpenAPI specification.
    ///
    /// # OpenAPI Specification
    ///
    /// From the OpenAPI 3.1.0 specification:
    /// - Parameter location: `in: cookie`
    /// - Cookies are typically optional parameters
    /// - Cookie values are serialized as simple strings
    pub(in crate::client) fn to_parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        self.cookies.iter().map(|(name, resolved)| {
            ParameterBuilder::new()
                .name(name)
                .parameter_in(ParameterIn::Cookie)
                .required(Required::False) // Cookies are typically optional
                .schema(Some(resolved.schema.clone()))
                .build()
        })
    }

    /// Converts cookies to HTTP Cookie header format.
    ///
    /// This method serializes all cookies into a single "Cookie" header value
    /// following the format: `name1=value1; name2=value2; name3=value3`
    ///
    /// # Returns
    ///
    /// Returns a Result containing the formatted cookie header value, or an error
    /// if any cookie value cannot be serialized.
    pub(in crate::client) fn to_cookie_header(&self) -> Result<String, ApiClientError> {
        if self.cookies.is_empty() {
            return Ok(String::new());
        }

        let mut cookie_parts = Vec::new();

        for (name, resolved) in &self.cookies {
            let value = resolved.to_string_value()?;
            cookie_parts.push(format!("{name}={value}"));
        }

        Ok(cookie_parts.join("; "))
    }

    /// Returns a reference to the schemas collected from cookie values.
    ///
    /// This method provides access to the internal schema collection for integration
    /// with the broader OpenAPI schema system.
    pub(in crate::client) fn schemas(&self) -> &Schemas {
        &self.schemas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ParamStyle;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    struct UserId(u64);

    #[test]
    fn test_new_empty_cookies() {
        let cookies = CallCookies::new();

        assert!(cookies.is_empty());
        assert_eq!(cookies.len(), 0);
    }

    #[test]
    fn test_add_string_cookie() {
        let cookies = CallCookies::new().add_cookie("session_id", "abc123");

        assert!(!cookies.is_empty());
        assert_eq!(cookies.len(), 1);

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should convert to cookie header");
        assert_eq!(cookie_header, "session_id=abc123");
    }

    #[test]
    fn test_add_multiple_cookies() {
        let cookies = CallCookies::new()
            .add_cookie("session_id", "abc123")
            .add_cookie("user_id", 456)
            .add_cookie("is_admin", true);

        assert_eq!(cookies.len(), 3);

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should convert to cookie header");

        // Check that all cookies are present
        assert!(cookie_header.contains("session_id=abc123"));
        assert!(cookie_header.contains("user_id=456"));
        assert!(cookie_header.contains("is_admin=true"));

        // Check format (separated by "; ")
        let parts: Vec<&str> = cookie_header.split("; ").collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_add_custom_type_cookie() {
        let cookies = CallCookies::new().add_cookie("user_id", UserId(42));

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should convert to cookie header");
        assert_eq!(cookie_header, "user_id=42");
    }

    #[test]
    fn test_cookie_merge() {
        let cookies1 = CallCookies::new()
            .add_cookie("session_id", "abc123")
            .add_cookie("user_id", 456);

        let cookies2 = CallCookies::new()
            .add_cookie("theme", "dark")
            .add_cookie("user_id", 789); // Override

        let merged = cookies1.merge(cookies2);

        assert_eq!(merged.len(), 3);

        let cookie_header = merged
            .to_cookie_header()
            .expect("Should convert to cookie header");

        assert!(cookie_header.contains("session_id=abc123"));
        assert!(cookie_header.contains("user_id=789")); // Should be overridden
        assert!(cookie_header.contains("theme=dark"));
    }

    #[test]
    fn test_cookies_to_parameters() {
        let cookies = CallCookies::new()
            .add_cookie("session_id", "abc123")
            .add_cookie("user_id", 456);

        let parameters: Vec<Parameter> = cookies.to_parameters().collect();

        assert_eq!(parameters.len(), 2);

        // Check parameter properties
        for param in &parameters {
            assert_eq!(param.parameter_in, ParameterIn::Cookie);
            assert_eq!(param.required, Required::False);
            assert!(param.schema.is_some());
            assert!(param.name == "session_id" || param.name == "user_id");
        }
    }

    #[test]
    fn test_empty_cookies_header() {
        let cookies = CallCookies::new();
        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle empty cookies");
        assert_eq!(cookie_header, "");
    }

    #[test]
    fn test_cookie_insertion_order_preserved() {
        let cookies = CallCookies::new()
            .add_cookie("first", "value1")
            .add_cookie("second", "value2")
            .add_cookie("third", "value3");

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should convert to cookie header");

        // Check that order is preserved in the header
        let parts: Vec<&str> = cookie_header.split("; ").collect();
        assert_eq!(parts, vec!["first=value1", "second=value2", "third=value3"]);
    }

    #[test]
    fn test_cookie_with_special_characters() {
        let cookies = CallCookies::new()
            .add_cookie("encoded_data", "hello world")
            .add_cookie("special", "test@example.com");

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle special characters");

        // Note: Cookie values are typically URL-encoded in real scenarios,
        // but our implementation treats them as simple strings
        assert!(cookie_header.contains("encoded_data=hello world"));
        assert!(cookie_header.contains("special=test@example.com"));
    }

    #[test]
    fn test_cookie_with_numeric_values() {
        let cookies = CallCookies::new()
            .add_cookie("count", 42)
            .add_cookie("rate", 2.5);

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle numeric values");

        assert!(cookie_header.contains("count=42"));
        assert!(cookie_header.contains("rate=2.5"));
    }

    #[test]
    fn test_cookie_with_boolean_values() {
        let cookies = CallCookies::new()
            .add_cookie("is_logged_in", true)
            .add_cookie("is_admin", false);

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle boolean values");

        assert!(cookie_header.contains("is_logged_in=true"));
        assert!(cookie_header.contains("is_admin=false"));
    }

    #[test]
    fn test_cookie_schemas_collection() {
        let cookies = CallCookies::new()
            .add_cookie("session_id", "abc123")
            .add_cookie("user_id", UserId(42));

        let schemas = cookies.schemas();

        // Should have collected schemas for String and UserId
        assert!(!schemas.schema_vec().is_empty());
    }

    #[test]
    fn test_cookie_override_in_merge() {
        let cookies1 = CallCookies::new()
            .add_cookie("same_cookie", "original_value")
            .add_cookie("unique_1", "value1");

        let cookies2 = CallCookies::new()
            .add_cookie("same_cookie", "new_value")
            .add_cookie("unique_2", "value2");

        let merged = cookies1.merge(cookies2);

        let cookie_header = merged
            .to_cookie_header()
            .expect("Should convert merged cookies");

        // The second cookies collection should override the first
        assert!(cookie_header.contains("same_cookie=new_value"));
        assert!(cookie_header.contains("unique_1=value1"));
        assert!(cookie_header.contains("unique_2=value2"));
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_cookie_with_array_values() {
        let cookies = CallCookies::new()
            .add_cookie("tags", vec!["rust", "web", "api"])
            .add_cookie("ids", vec![1, 2, 3]);

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle array values");

        // Arrays should be serialized as comma-separated values (Simple style)
        assert!(cookie_header.contains("tags=rust,web,api"));
        assert!(cookie_header.contains("ids=1,2,3"));
    }

    #[test]
    fn test_cookie_with_null_value() {
        let cookies = CallCookies::new().add_cookie("optional", serde_json::Value::Null);

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle null values");

        // Null values should serialize to empty string
        assert!(cookie_header.contains("optional="));
    }

    #[test]
    fn test_cookie_error_with_complex_object() {
        use serde_json::json;

        // Create a cookies collection with a complex nested object
        let mut cookies = CallCookies::new();

        // Add a complex object that should cause an error during to_string_value conversion
        let complex_value = json!({
            "nested": {
                "object": "not supported in cookies"
            }
        });

        let resolved = ResolvedParamValue {
            value: complex_value,
            schema: cookies.schemas.add::<serde_json::Value>(),
            style: ParamStyle::Simple,
        };

        cookies
            .cookies
            .insert("complex_cookie".to_string(), resolved);

        // Now test that to_cookie_header fails for the complex object
        let result = cookies.to_cookie_header();
        assert!(
            result.is_err(),
            "Complex objects should cause error in cookies"
        );

        match result {
            Err(ApiClientError::UnsupportedParameterValue { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected UnsupportedParameterValue error for complex object in cookie"),
        }
    }

    #[test]
    fn test_single_cookie_no_semicolon() {
        let cookies = CallCookies::new().add_cookie("single", "value");

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should convert single cookie");

        // Single cookie should not have trailing semicolon
        assert_eq!(cookie_header, "single=value");
    }

    #[test]
    fn test_cookie_with_empty_string_value() {
        let cookies = CallCookies::new().add_cookie("empty", "");

        let cookie_header = cookies
            .to_cookie_header()
            .expect("Should handle empty string values");

        assert_eq!(cookie_header, "empty=");
    }
}
