//! Custom extractors for handling multiple content types and streaming data.

use axum::{
    Json,
    extract::{FromRequest, Multipart, Request},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{StreamDeserializer, json};
use std::{io::Cursor, string::ToString};
use utoipa::ToSchema;

/// Custom error type for extractors that provides structured error responses.
///
/// This enum represents different types of extraction failures that can occur
/// when processing HTTP requests. Each variant corresponds to a specific error
/// scenario and provides appropriate HTTP status codes and error details.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "details")]
pub enum ExtractorError {
    /// JSON parsing or serialization error
    JsonError {
        /// Detailed error message
        message: String,
        /// Location where the error occurred (e.g., "body", "`field_name`")
        location: Option<String>,
    },

    /// XML parsing error
    XmlError {
        /// Detailed error message
        message: String,
        /// XML element where the error occurred
        element: Option<String>,
    },

    /// Form data processing error
    FormError {
        /// Detailed error message
        message: String,
        /// Form field that caused the error
        field: Option<String>,
    },

    /// Multipart processing error
    MultipartError {
        /// Detailed error message
        message: String,
        /// Multipart section that caused the error
        part: Option<String>,
    },

    /// Encoding/decoding error (e.g., UTF-8 conversion)
    EncodingError {
        /// Detailed error message
        message: String,
        /// Expected encoding type
        encoding: String,
    },

    /// Unsupported content type
    UnsupportedMediaType {
        /// The unsupported content type
        content_type: String,
        /// List of supported content types
        supported: Vec<String>,
    },

    /// Generic bad request error
    BadRequest {
        /// Detailed error message
        message: String,
    },
}

impl ExtractorError {
    /// Creates a new bad request error with a message.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    /// Creates a new unsupported media type error.
    pub fn unsupported_media_type(content_type: impl Into<String>) -> Self {
        Self::UnsupportedMediaType {
            content_type: content_type.into(),
            supported: vec![
                "application/json".to_string(),
                "application/xml".to_string(),
                "text/xml".to_string(),
                "application/x-www-form-urlencoded".to_string(),
                "multipart/form-data".to_string(),
            ],
        }
    }

    /// Creates an error for multipart processing failures.
    pub fn multipart_error(message: impl Into<String>) -> Self {
        Self::MultipartError {
            message: message.into(),
            part: None,
        }
    }

    /// Creates an error for multipart processing failures with part information.
    pub fn multipart_error_with_part(message: impl Into<String>, part: impl Into<String>) -> Self {
        Self::MultipartError {
            message: message.into(),
            part: Some(part.into()),
        }
    }

    /// Creates an error for JSON parsing failures.
    pub fn json_error(message: impl Into<String>) -> Self {
        Self::JsonError {
            message: message.into(),
            location: None,
        }
    }

    /// Creates an error for JSON parsing failures with location information.
    pub fn json_error_with_location(
        message: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        Self::JsonError {
            message: message.into(),
            location: Some(location.into()),
        }
    }

    /// Creates an error for XML parsing failures.
    pub fn xml_error(message: impl Into<String>) -> Self {
        Self::XmlError {
            message: message.into(),
            element: None,
        }
    }

    /// Creates an error for XML parsing failures with element information.
    pub fn xml_error_with_element(message: impl Into<String>, element: impl Into<String>) -> Self {
        Self::XmlError {
            message: message.into(),
            element: Some(element.into()),
        }
    }

    /// Creates an error for form data processing failures.
    pub fn form_error(message: impl Into<String>) -> Self {
        Self::FormError {
            message: message.into(),
            field: None,
        }
    }

    /// Creates an error for form data processing failures with field information.
    pub fn form_error_with_field(message: impl Into<String>, field: impl Into<String>) -> Self {
        Self::FormError {
            message: message.into(),
            field: Some(field.into()),
        }
    }

    /// Creates an error for encoding/decoding failures.
    pub fn encoding_error(message: impl Into<String>, encoding: impl Into<String>) -> Self {
        Self::EncodingError {
            message: message.into(),
            encoding: encoding.into(),
        }
    }

    /// Returns the appropriate HTTP status code for this error.
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::JsonError { .. }
            | Self::XmlError { .. }
            | Self::FormError { .. }
            | Self::MultipartError { .. }
            | Self::EncodingError { .. }
            | Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::UnsupportedMediaType { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        }
    }
}

impl IntoResponse for ExtractorError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        // Serialize the enum directly as JSON
        // The serde tag/content attributes will create the proper structure
        let error_response = json!({
            "status": status.as_u16(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "error": self,
        });

        (status, Json(error_response)).into_response()
    }
}

impl std::fmt::Display for ExtractorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonError { message, location } => {
                if let Some(loc) = location {
                    write!(f, "JSON error in {loc}: {message}")
                } else {
                    write!(f, "JSON error: {message}")
                }
            }
            Self::XmlError { message, element } => {
                if let Some(elem) = element {
                    write!(f, "XML error in element '{elem}': {message}")
                } else {
                    write!(f, "XML error: {message}")
                }
            }
            Self::FormError { message, field } => {
                if let Some(field_name) = field {
                    write!(f, "Form error in field '{field_name}': {message}")
                } else {
                    write!(f, "Form error: {message}")
                }
            }
            Self::MultipartError { message, part } => {
                if let Some(part_name) = part {
                    write!(f, "Multipart error in part '{part_name}': {message}")
                } else {
                    write!(f, "Multipart error: {message}")
                }
            }
            Self::EncodingError { message, encoding } => {
                write!(f, "Encoding error ({encoding}): {message}")
            }
            Self::UnsupportedMediaType { content_type, .. } => {
                write!(f, "Unsupported media type: {content_type}")
            }
            Self::BadRequest { message } => {
                write!(f, "Bad request: {message}")
            }
        }
    }
}

impl std::error::Error for ExtractorError {}

/// Multi-format extractor that can handle JSON, form-encoded, or XML data.
///
/// This extractor automatically detects the content type from the request headers
/// and deserializes the body using the appropriate format. For form data, it expects
/// a flattened structure since form encoding doesn't support nested objects.
pub struct AnyFormat<T>(pub T);

impl<S, T> FromRequest<S> for AnyFormat<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        match content_type {
            ct if ct.starts_with("application/json") => axum::Json::<T>::from_request(req, state)
                .await
                .map(|axum::Json(data)| Self(data))
                .map_err(|rejection| {
                    ExtractorError::json_error_with_location(rejection.body_text(), "request_body")
                }),
            ct if ct.starts_with("application/x-www-form-urlencoded") => {
                axum::extract::Form::<T>::from_request(req, state)
                    .await
                    .map(|axum::extract::Form(data)| Self(data))
                    .map_err(|rejection| ExtractorError::form_error(rejection.body_text()))
            }
            ct if ct.starts_with("application/xml") || ct.starts_with("text/xml") => {
                let bytes = Bytes::from_request(req, state)
                    .await
                    .map_err(|rejection| ExtractorError::bad_request(rejection.body_text()))?;

                serde_xml_rs::from_reader(bytes.as_ref())
                    .map(Self)
                    .map_err(|err| ExtractorError::xml_error(err.to_string()))
            }
            _ => Err(ExtractorError::unsupported_media_type(content_type)),
        }
    }
}

/// Binary stream processor that uses [`serde_json::StreamDeserializer`] to parse
/// multiple JSON objects from a raw binary stream.
///
/// This is useful for bulk data imports where the payload contains multiple
/// JSON objects separated by newlines or as a JSON array.
pub struct JsonStream<T> {
    pub data: Vec<T>,
    pub bytes_processed: usize,
}

impl<S, T> FromRequest<S> for JsonStream<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|rejection| ExtractorError::bad_request(rejection.body_text()))?;

        let cursor = Cursor::new(&bytes);
        let mut stream = StreamDeserializer::new(serde_json::de::IoRead::new(cursor));
        let mut data = Vec::new();
        let mut bytes_processed = 0;

        loop {
            let offset_before = stream.byte_offset();
            match stream.next() {
                Some(Ok(value)) => {
                    data.push(value);
                    bytes_processed = stream.byte_offset();
                }
                Some(Err(err)) if err.is_eof() => {
                    bytes_processed = stream.byte_offset();
                    break;
                }
                Some(Err(err)) => {
                    return Err(ExtractorError::json_error_with_location(
                        format!("Error at byte {offset_before}: {err}"),
                        "stream_content",
                    ));
                }
                None => break,
            }
        }

        Ok(Self {
            data,
            bytes_processed,
        })
    }
}

/// Multipart file upload extractor that handles form data and file uploads.
///
/// This extractor processes multipart/form-data requests and extracts both
/// text fields and file uploads. It's commonly used for file upload endpoints.
pub struct MultipartUpload {
    pub fields: Vec<(String, String)>,
    pub files: Vec<(String, Vec<u8>, Option<String>)>, // (field_name, data, filename)
}

impl<S> FromRequest<S> for MultipartUpload
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut multipart = Multipart::from_request(req, state)
            .await
            .map_err(|rejection| ExtractorError::multipart_error(rejection.body_text()))?;

        let mut fields = Vec::new();
        let mut files = Vec::new();

        while let Some(field) = multipart.next_field().await.map_err(|err| {
            ExtractorError::multipart_error(format!("Failed to read field: {err}"))
        })? {
            let name = field.name().unwrap_or("unknown").to_string();
            let filename = field.file_name().map(ToString::to_string);

            let data = field.bytes().await.map_err(|err| {
                ExtractorError::multipart_error_with_part(
                    format!("Failed to read field data: {err}"),
                    &name,
                )
            })?;

            if filename.is_some() {
                // This is a file upload
                files.push((name, data.to_vec(), filename));
            } else {
                // This is a text field
                let text = String::from_utf8(data.to_vec()).map_err(|err| {
                    ExtractorError::encoding_error(
                        format!("Invalid UTF-8 in text field: {err}"),
                        "utf-8",
                    )
                })?;
                fields.push((name, text));
            }
        }

        Ok(Self { fields, files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::{http::StatusCode, response::IntoResponse};

    #[tokio::test]
    async fn test_extractor_error_into_response() {
        let error = ExtractorError::json_error("Invalid JSON syntax");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_extractor_error_unsupported_media_type() {
        let error = ExtractorError::unsupported_media_type("application/pdf");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn test_extractor_error_multipart() {
        let error = ExtractorError::multipart_error("Failed to read boundary");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_extractor_error_encoding() {
        let error = ExtractorError::encoding_error("Invalid UTF-8 sequence", "utf-8");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_extractor_error_xml() {
        let error = ExtractorError::xml_error("Missing closing tag");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_extractor_error_enum_serialization() -> anyhow::Result<()> {
        // Test that our enum-based error properly serializes to JSON
        // This demonstrates the structured error responses

        let json_error = ExtractorError::json_error_with_location(
            "Unexpected token at line 5, column 10",
            "request_body",
        );

        let xml_error =
            ExtractorError::xml_error_with_element("Missing closing tag", "observation");

        let form_error = ExtractorError::form_error_with_field("Required field missing", "name");

        let multipart_error =
            ExtractorError::multipart_error_with_part("Invalid boundary", "file_upload");

        let encoding_error =
            ExtractorError::encoding_error("Invalid UTF-8 sequence at byte 42", "utf-8");

        let unsupported_error = ExtractorError::unsupported_media_type("application/yaml");

        // Serialize each error to verify the JSON structure
        let json_error_json = serde_json::to_string_pretty(&json_error)?;
        let xml_error_json = serde_json::to_string_pretty(&xml_error)?;
        let form_error_json = serde_json::to_string_pretty(&form_error)?;
        let multipart_error_json = serde_json::to_string_pretty(&multipart_error)?;
        let encoding_error_json = serde_json::to_string_pretty(&encoding_error)?;
        let unsupported_error_json = serde_json::to_string_pretty(&unsupported_error)?;

        // Verify the tagged union structure is correct
        assert!(json_error_json.contains("\"type\": \"JsonError\""));
        assert!(json_error_json.contains("\"details\""));
        assert!(json_error_json.contains("\"location\": \"request_body\""));

        assert!(xml_error_json.contains("\"type\": \"XmlError\""));
        assert!(xml_error_json.contains("\"element\": \"observation\""));

        assert!(form_error_json.contains("\"type\": \"FormError\""));
        assert!(form_error_json.contains("\"field\": \"name\""));

        assert!(multipart_error_json.contains("\"type\": \"MultipartError\""));
        assert!(multipart_error_json.contains("\"part\": \"file_upload\""));

        assert!(encoding_error_json.contains("\"type\": \"EncodingError\""));
        assert!(encoding_error_json.contains("\"encoding\": \"utf-8\""));

        assert!(unsupported_error_json.contains("\"type\": \"UnsupportedMediaType\""));
        assert!(unsupported_error_json.contains("\"content_type\": \"application/yaml\""));
        assert!(unsupported_error_json.contains("\"supported\""));

        Ok(())
    }
}
