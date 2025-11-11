use indexmap::IndexMap;
use utoipa::openapi::Required;
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};

use super::param::{ParamValue, ParameterValue, ResolvedParamValue};
use crate::client::error::ApiClientError;
use crate::client::openapi::schema::Schemas;

/// Represents HTTP headers for an API call.
///
/// This struct manages HTTP headers using the same ParamValue pattern as query and path parameters,
/// allowing for type-safe header handling with automatic OpenAPI schema generation.
#[derive(Debug, Clone, Default)]
pub struct CallHeaders {
    headers: IndexMap<String, ResolvedParamValue>,
    pub(in crate::client) schemas: Schemas,
}

impl CallHeaders {
    /// Creates a new empty CallHeaders instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a header parameter to the collection.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The header value type, must implement `Serialize`, `ToSchema`, `Debug`, `Send`, `Sync`, and `Clone`
    ///
    /// # Arguments
    ///
    /// * `name` - The header name (e.g., "Authorization", "Content-Type")
    /// * `value` - The header value, either a direct value or wrapped in ParamValue
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::CallHeaders;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let headers = CallHeaders::new()
    ///     .add_header("Authorization", "Bearer token123")
    ///     .add_header("X-Request-ID", "abc-123-def");
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_header<T: ParameterValue>(
        mut self,
        name: impl Into<String>,
        value: impl Into<ParamValue<T>>,
    ) -> Self {
        let name = name.into();
        let param_value = value.into();

        // Generate schema for the header value
        let schema = self.schemas.add::<T>();

        // Convert to resolved param value
        let resolved = ResolvedParamValue {
            value: param_value
                .as_header_value()
                .expect("Header serialization should not fail"),
            schema,
            style: param_value.header_style(),
        };

        self.headers.insert(name, resolved);
        self
    }

    /// Merges another CallHeaders instance into this one.
    ///
    /// Headers from the other instance will override headers with the same name in this instance.
    pub fn merge(mut self, other: Self) -> Self {
        // Merge schemas first
        self.schemas.merge(other.schemas);

        // Merge headers (other takes precedence)
        for (name, value) in other.headers {
            self.headers.insert(name, value);
        }

        self
    }

    /// Checks if the headers collection is empty.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Returns the number of headers.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Converts headers to OpenAPI Parameter objects.
    pub(in crate::client) fn to_parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        self.headers.iter().map(|(name, resolved)| {
            ParameterBuilder::new()
                .name(name)
                .parameter_in(ParameterIn::Header)
                .required(Required::False) // Headers are typically optional
                .schema(Some(resolved.schema.clone()))
                .build()
        })
    }

    /// Converts headers to HTTP header format for reqwest.
    pub(in crate::client) fn to_http_headers(
        &self,
    ) -> Result<Vec<(String, String)>, ApiClientError> {
        let mut result = Vec::new();

        for (name, resolved) in &self.headers {
            let value = resolved.to_string_value()?;
            result.push((name.clone(), value));
        }

        Ok(result)
    }

    /// Returns a reference to the schemas collected from header values.
    pub(in crate::client) fn schemas(&self) -> &Schemas {
        &self.schemas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ParamStyle;
    use indexmap::IndexMap;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    struct TestId(u64);

    #[test]
    fn test_new_empty_headers() {
        let headers = CallHeaders::new();

        assert!(headers.is_empty());
        assert_eq!(headers.len(), 0);
    }

    #[test]
    fn test_add_string_header() {
        let headers = CallHeaders::new().add_header("Authorization", "Bearer token123");

        assert!(!headers.is_empty());
        assert_eq!(headers.len(), 1);

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        assert_eq!(
            http_headers,
            vec![("Authorization".to_string(), "Bearer token123".to_string())]
        );
    }

    #[test]
    fn test_add_multiple_headers() {
        let headers = CallHeaders::new()
            .add_header("Authorization", "Bearer token123")
            .add_header("X-Request-ID", "abc-123-def")
            .add_header("Content-Type", "application/json");

        assert_eq!(headers.len(), 3);

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        assert_eq!(http_headers.len(), 3);

        // Check that all headers are present and verify order preservation
        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();
        assert_eq!(
            header_map.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert_eq!(
            header_map.get("X-Request-ID"),
            Some(&"abc-123-def".to_string())
        );
        assert_eq!(
            header_map.get("Content-Type"),
            Some(&"application/json".to_string())
        );

        // Verify insertion order is preserved
        let keys: Vec<_> = header_map.keys().cloned().collect();
        assert_eq!(keys, vec!["Authorization", "X-Request-ID", "Content-Type"]);
    }

    #[test]
    fn test_add_numeric_header() {
        let headers = CallHeaders::new().add_header("X-Rate-Limit", 1000u32);

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        assert_eq!(
            http_headers,
            vec![("X-Rate-Limit".to_string(), "1000".to_string())]
        );
    }

    #[test]
    fn test_add_custom_type_header() {
        let headers = CallHeaders::new().add_header("X-User-ID", TestId(42));

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        assert_eq!(
            http_headers,
            vec![("X-User-ID".to_string(), "42".to_string())]
        );
    }

    #[test]
    fn test_add_header_with_param_style() {
        let headers = CallHeaders::new().add_header(
            "X-Tags",
            ParamValue::with_style(vec!["rust", "web", "api"], ParamStyle::Simple),
        );

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        assert_eq!(
            http_headers,
            vec![("X-Tags".to_string(), "rust,web,api".to_string())]
        );
    }

    #[test]
    fn test_header_merge() {
        let headers1 = CallHeaders::new()
            .add_header("Authorization", "Bearer token123")
            .add_header("X-Request-ID", "abc-123-def");

        let headers2 = CallHeaders::new()
            .add_header("Content-Type", "application/json")
            .add_header("X-Request-ID", "xyz-789-ghi"); // Override

        let merged = headers1.merge(headers2);

        assert_eq!(merged.len(), 3);

        let http_headers = merged
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();

        assert_eq!(
            header_map.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert_eq!(
            header_map.get("X-Request-ID"),
            Some(&"xyz-789-ghi".to_string())
        ); // Should be overridden
        assert_eq!(
            header_map.get("Content-Type"),
            Some(&"application/json".to_string())
        );

        // Verify merge order: original headers first, then new headers
        let keys: Vec<_> = header_map.keys().cloned().collect();
        assert_eq!(keys, vec!["Authorization", "X-Request-ID", "Content-Type"]);
    }

    #[test]
    fn test_headers_to_parameters() {
        let headers = CallHeaders::new()
            .add_header("Authorization", "Bearer token123")
            .add_header("X-Rate-Limit", 1000u32);

        let parameters: Vec<Parameter> = headers.to_parameters().collect();

        assert_eq!(parameters.len(), 2);

        // Check parameter properties
        for param in &parameters {
            assert_eq!(param.parameter_in, ParameterIn::Header);
            assert_eq!(param.required, Required::False);
            assert!(param.schema.is_some());
            assert!(param.name == "Authorization" || param.name == "X-Rate-Limit");
        }
    }

    #[test]
    fn test_empty_headers_merge() {
        let headers1 = CallHeaders::new().add_header("Authorization", "Bearer token123");

        let headers2 = CallHeaders::new();

        let merged = headers1.merge(headers2);
        assert_eq!(merged.len(), 1);

        let http_headers = merged
            .to_http_headers()
            .expect("Should convert to HTTP headers");
        assert_eq!(
            http_headers,
            vec![("Authorization".to_string(), "Bearer token123".to_string())]
        );
    }

    #[test]
    fn test_headers_schema_collection() {
        let headers = CallHeaders::new()
            .add_header("Authorization", "Bearer token123")
            .add_header("X-User-ID", TestId(42));

        let schemas = headers.schemas();

        // Should have collected schemas for String and TestId
        assert!(!schemas.schema_vec().is_empty());
    }

    #[test]
    fn test_header_insertion_order_preserved() {
        let headers = CallHeaders::new()
            .add_header("First", "value1")
            .add_header("Second", "value2")
            .add_header("Third", "value3")
            .add_header("Fourth", "value4");

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");

        // Verify that headers maintain insertion order
        let actual_order: Vec<String> = http_headers.iter().map(|(name, _)| name.clone()).collect();
        let expected_order = vec!["First", "Second", "Third", "Fourth"];

        assert_eq!(actual_order, expected_order);
    }

    #[test]
    fn test_header_with_array_values() {
        let headers = CallHeaders::new()
            .add_header("X-Tags", vec!["rust", "web", "api"])
            .add_header("X-Numbers", vec![1, 2, 3]);

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");

        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();

        // Headers use Simple style by default for arrays (comma-separated)
        assert_eq!(header_map.get("X-Tags"), Some(&"rust,web,api".to_string()));
        assert_eq!(header_map.get("X-Numbers"), Some(&"1,2,3".to_string()));
    }

    #[test]
    fn test_header_with_boolean_values() {
        let headers = CallHeaders::new()
            .add_header("X-Debug", true)
            .add_header("X-Enabled", false);

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");

        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();

        assert_eq!(header_map.get("X-Debug"), Some(&"true".to_string()));
        assert_eq!(header_map.get("X-Enabled"), Some(&"false".to_string()));
    }

    #[test]
    fn test_header_with_null_value() {
        let headers = CallHeaders::new().add_header("X-Optional", serde_json::Value::Null);

        let http_headers = headers
            .to_http_headers()
            .expect("Should convert to HTTP headers");

        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();

        // Null values should serialize to empty string
        assert_eq!(header_map.get("X-Optional"), Some(&String::new()));
    }

    #[test]
    fn test_header_error_with_complex_object() {
        use serde_json::json;

        // Create a headers collection with a complex nested object
        // Note: We need to bypass the normal add_header method since it expects()
        // the header value to serialize correctly. Instead, we'll create a
        // ResolvedParamValue manually with an unsupported object type.
        let mut headers = CallHeaders::new();

        // Add a complex object that should cause an error during to_string_value conversion
        let complex_value = json!({
            "nested": {
                "object": "not supported in headers"
            }
        });

        let resolved = ResolvedParamValue {
            value: complex_value,
            schema: headers.schemas.add::<serde_json::Value>(),
            style: ParamStyle::Simple,
        };

        headers.headers.insert("X-Complex".to_string(), resolved);

        // Now test that to_http_headers fails for the complex object
        let result = headers.to_http_headers();
        assert!(
            result.is_err(),
            "Complex objects should cause error in headers"
        );

        match result {
            Err(ApiClientError::UnsupportedParameterValue { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected UnsupportedParameterValue error for complex object in header"),
        }
    }

    #[test]
    fn test_header_error_with_array_containing_objects() {
        use serde_json::json;

        // Similar to above, test arrays containing objects
        let mut headers = CallHeaders::new();

        let array_with_objects = json!([
            "simple_string",
            {"nested": "object"}
        ]);

        let resolved = ResolvedParamValue {
            value: array_with_objects,
            schema: headers.schemas.add::<serde_json::Value>(),
            style: ParamStyle::Simple,
        };

        headers
            .headers
            .insert("X-Invalid-Array".to_string(), resolved);

        let result = headers.to_http_headers();
        assert!(
            result.is_err(),
            "Arrays containing objects should cause error"
        );

        match result {
            Err(ApiClientError::UnsupportedParameterValue { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected UnsupportedParameterValue error for array with objects"),
        }
    }

    #[test]
    fn test_header_with_empty_array() {
        let headers = CallHeaders::new().add_header("X-Empty-List", Vec::<String>::new());

        let http_headers = headers
            .to_http_headers()
            .expect("Should handle empty arrays");

        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();

        // Empty arrays should serialize to empty string
        assert_eq!(header_map.get("X-Empty-List"), Some(&String::new()));
    }

    #[test]
    fn test_header_override_in_merge() {
        let headers1 = CallHeaders::new()
            .add_header("Same-Header", "original-value")
            .add_header("Unique-1", "value1");

        let headers2 = CallHeaders::new()
            .add_header("Same-Header", "new-value")
            .add_header("Unique-2", "value2");

        let merged = headers1.merge(headers2);

        let http_headers = merged
            .to_http_headers()
            .expect("Should convert merged headers");

        let header_map: IndexMap<String, String> = http_headers.into_iter().collect();

        // The second headers collection should override the first
        assert_eq!(
            header_map.get("Same-Header"),
            Some(&"new-value".to_string())
        );
        assert_eq!(header_map.get("Unique-1"), Some(&"value1".to_string()));
        assert_eq!(header_map.get("Unique-2"), Some(&"value2".to_string()));
        assert_eq!(header_map.len(), 3);
    }
}
