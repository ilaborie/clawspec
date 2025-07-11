use headers::ContentType;
use serde::Serialize;
use utoipa::ToSchema;

use super::ApiClientError;
use super::schema::SchemaEntry;

/// Represents the body of an HTTP request with its content type and schema information.
///
/// `CallBody` encapsulates the raw body data, content type, and schema entry information
/// needed for API requests. It supports various content types including JSON, form-encoded,
/// and raw binary data.
#[derive(Clone, derive_more::Debug)]
pub struct CallBody {
    pub(super) content_type: ContentType,
    pub(super) entry: SchemaEntry,
    #[debug(ignore)]
    pub(super) data: Vec<u8>,
}

impl CallBody {
    /// Creates a JSON body from a serializable type.
    ///
    /// This method serializes the data as `application/json` using the `serde_json` crate.
    /// The data must implement `Serialize` and `ToSchema` for proper JSON serialization
    /// and OpenAPI schema generation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::CallBody;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #[derive(Serialize, ToSchema)]
    /// struct User {
    ///     name: String,
    ///     age: u32,
    /// }
    ///
    /// let user = User {
    ///     name: "Alice".to_string(),
    ///     age: 30,
    /// };
    ///
    /// let body = CallBody::json(&user)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn json<T>(t: &T) -> Result<Self, ApiClientError>
    where
        T: Serialize + ToSchema + 'static,
    {
        let content_type = ContentType::json();

        let mut entry = SchemaEntry::of::<T>();
        let example = serde_json::to_value(t)?;
        entry.add_example(example);

        let data = serde_json::to_vec(t)?;

        let result = Self {
            content_type,
            entry,
            data,
        };
        Ok(result)
    }

    /// Creates a form-encoded body from a serializable type.
    ///
    /// This method serializes the data as `application/x-www-form-urlencoded`
    /// using the `serde_urlencoded` crate. The data must implement `Serialize`
    /// and `ToSchema` for proper form encoding and OpenAPI schema generation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::CallBody;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #[derive(Serialize, ToSchema)]
    /// struct LoginForm {
    ///     username: String,
    ///     password: String,
    /// }
    ///
    /// let form = LoginForm {
    ///     username: "user@example.com".to_string(),
    ///     password: "secret".to_string(),
    /// };
    ///
    /// let body = CallBody::form(&form)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn form<T>(t: &T) -> Result<Self, ApiClientError>
    where
        T: Serialize + ToSchema + 'static,
    {
        let content_type = ContentType::form_url_encoded();

        let mut entry = SchemaEntry::of::<T>();
        let example = serde_json::to_value(t)?;
        entry.add_example(example);

        let data = serde_urlencoded::to_string(t)
            .map_err(|e| ApiClientError::SerializationError {
                message: format!("Failed to serialize form data: {e}"),
            })?
            .into_bytes();

        let result = Self {
            content_type,
            entry,
            data,
        };
        Ok(result)
    }

    /// Creates a raw body with custom content type.
    ///
    /// This method allows you to send arbitrary binary data with a specified
    /// content type. This is useful for sending data that doesn't fit into
    /// the standard JSON or form categories.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallBody;
    /// use headers::ContentType;
    ///
    /// // Send XML data
    /// let xml_data = r#"<?xml version="1.0"?><user><name>John</name></user>"#;
    /// let body = CallBody::raw(
    ///     xml_data.as_bytes().to_vec(),
    ///     ContentType::xml()
    /// );
    ///
    /// // Send binary data
    /// let binary_data = vec![0xFF, 0xFE, 0xFD];
    /// let body = CallBody::raw(
    ///     binary_data,
    ///     ContentType::octet_stream()
    /// );
    /// ```
    pub fn raw(data: Vec<u8>, content_type: ContentType) -> Self {
        // For raw bodies, we don't have a specific type to generate schema from,
        // so we create a generic binary schema entry
        let entry = SchemaEntry::raw_binary();

        Self {
            content_type,
            entry,
            data,
        }
    }

    /// Creates a text body with text/plain content type.
    ///
    /// This is a convenience method for sending plain text data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallBody;
    ///
    /// let body = CallBody::text("Hello, World!");
    /// ```
    pub fn text(text: &str) -> Self {
        Self::raw(text.as_bytes().to_vec(), ContentType::text())
    }

    /// Creates a multipart/form-data body for file uploads and form data.
    ///
    /// This method creates a multipart body with a generated boundary and supports
    /// both text fields and file uploads. The boundary is automatically generated
    /// and included in the content type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::CallBody;
    ///
    /// let mut parts = Vec::new();
    /// parts.push(("field1", "value1"));
    /// parts.push(("file", "file content"));
    ///
    /// let body = CallBody::multipart(parts);
    /// ```
    pub fn multipart(parts: Vec<(&str, &str)>) -> Self {
        let boundary = format!("----formdata-clawspec-{}", uuid::Uuid::new_v4());
        let content_type = format!("multipart/form-data; boundary={boundary}");

        let mut body_data = Vec::new();

        for (name, value) in parts {
            body_data.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body_data.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body_data.extend_from_slice(value.as_bytes());
            body_data.extend_from_slice(b"\r\n");
        }

        body_data.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        let content_type = ContentType::from(content_type.parse::<mime::Mime>().unwrap());
        let entry = SchemaEntry::raw_binary();

        Self {
            content_type,
            entry,
            data: body_data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_call_body_json_creates_valid_body() {
        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let body = CallBody::json(&test_data).expect("should create body");

        insta::assert_debug_snapshot!(body, @r#"
        CallBody {
            content_type: ContentType(
                "application/json",
            ),
            entry: SchemaEntry {
                type_name: "clawspec_core::client::body::tests::TestData",
                name: "TestData",
                examples: {
                    Object {
                        "name": String("test"),
                        "value": Number(42),
                    },
                },
                ..
            },
            ..
        }
        "#);
        let parsed = serde_json::from_slice::<TestData>(&body.data).expect("should parse JSON");
        assert_eq!(parsed, test_data);
    }

    #[test]
    fn test_call_body_form_creates_valid_body() {
        let test_data = TestData {
            name: "test user".to_string(),
            value: 42,
        };

        let body = CallBody::form(&test_data).expect("should create form body");

        assert_eq!(body.content_type, headers::ContentType::form_url_encoded());
        assert_eq!(body.entry.name, "TestData");

        // Verify the form encoding
        let form_data = String::from_utf8(body.data).expect("should be valid UTF-8");
        insta::assert_snapshot!(form_data, @"name=test+user&value=42");
    }

    #[test]
    fn test_call_body_raw_creates_valid_body() {
        let binary_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
        let content_type = headers::ContentType::octet_stream();

        let body = CallBody::raw(binary_data.clone(), content_type.clone());

        assert_eq!(body.content_type, content_type);
        assert_eq!(body.entry.name, "binary");
        assert_eq!(body.data, binary_data);
    }

    #[test]
    fn test_call_body_text_creates_valid_body() {
        let text_content = "Hello, World!";

        let body = CallBody::text(text_content);

        assert_eq!(body.content_type, headers::ContentType::text());
        assert_eq!(body.entry.name, "binary");
        assert_eq!(body.data, text_content.as_bytes());
    }
}
