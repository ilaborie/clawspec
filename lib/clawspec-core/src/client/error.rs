use std::fmt::Debug;

use super::auth::AuthenticationError;
use super::output::Output;

/// Errors that can occur when using the ApiClient.
///
/// This enum covers all possible error conditions from network issues to data validation failures.
/// All variants implement `std::error::Error` and provide detailed context for debugging.
#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum ApiClientError {
    /// HTTP client error from the underlying reqwest library.
    ///
    /// Occurs when network requests fail, timeouts occur, or connection issues arise.
    ReqwestError(reqwest::Error),

    /// URL parsing error when constructing request URLs.
    ///
    /// Occurs when the base URL or path parameters create an invalid URL.
    UrlError(url::ParseError),

    /// HTTP header processing error.
    ///
    /// Occurs when header values cannot be processed or are malformed.
    HeadersError(headers::Error),

    /// HTTP protocol error from the http crate.
    ///
    /// Occurs when HTTP protocol constraints are violated.
    HttpError(http::Error),

    /// Invalid HTTP header name.
    ///
    /// Occurs when attempting to create headers with invalid names.
    InvalidHeaderName(http::header::InvalidHeaderName),

    /// Invalid HTTP header value.
    ///
    /// Occurs when header values contain invalid characters.
    InvalidHeaderValue(http::header::InvalidHeaderValue),

    /// JSON serialization/deserialization error.
    ///
    /// Occurs when working with JSON request bodies or responses.
    JsonValueError(serde_json::Error),

    /// Query parameter serialization error.
    ///
    /// Occurs when converting structures to URL query strings.
    QuerySerializationError(serde_urlencoded::ser::Error),

    /// Authentication processing error.
    ///
    /// Occurs when authentication credentials cannot be processed or are invalid.
    AuthenticationError(AuthenticationError),

    /// No call result available for operation.
    ///
    /// Occurs when attempting to access response data before making a request.
    #[display("Invalid state: expected a call result")]
    CallResultRequired,

    /// Invalid base path configuration.
    ///
    /// Occurs when the provided base path cannot be used for URL construction.
    #[display("Invalid base path: {error}")]
    InvalidBasePath {
        /// Description of why the base path is invalid.
        error: String,
    },

    /// JSON response deserialization failure.
    ///
    /// Occurs when the response body cannot be parsed as the expected JSON structure.
    #[display("Failed to deserialize JSON at '{path}': {error}\n{body}")]
    #[from(skip)]
    JsonError {
        /// The request path where the error occurred.
        path: String,
        /// The underlying JSON parsing error.
        error: serde_json::Error,
        /// The response body that failed to parse.
        body: String,
    },

    /// Response output type is incompatible with JSON deserialization.
    ///
    /// Occurs when attempting to parse non-JSON responses as JSON.
    #[display("Unsupported output for {name} as JSON:\n{output:?}")]
    #[from(skip)]
    UnsupportedJsonOutput {
        /// The actual response output received.
        output: Output,
        /// Name of the operation that failed.
        name: &'static str,
    },

    /// Response output type is incompatible with text extraction.
    ///
    /// Occurs when attempting to extract text from binary or empty responses.
    #[display("Unsupported output for text:\n{output:?}")]
    #[from(skip)]
    UnsupportedTextOutput {
        /// The actual response output received.
        output: Output,
    },

    /// Response output type is incompatible with byte extraction.
    ///
    /// Occurs when attempting to extract bytes from empty responses.
    #[display("Unsupported output for bytes:\n{output:?}")]
    #[from(skip)]
    UnsupportedBytesOutput {
        /// The actual response output received.
        output: Output,
    },

    /// Path template contains unresolved parameters.
    ///
    /// Occurs when path parameters are missing for templated URLs.
    #[display("Path '{path}' is missing required arguments: {missings:?}")]
    #[from(skip)]
    PathUnresolved {
        /// The path template that couldn't be resolved.
        path: String,
        /// List of missing parameter names.
        missings: Vec<String>,
    },

    /// Query parameter value type is not supported.
    ///
    /// Occurs when attempting to use complex objects as query parameters.
    #[display(
        "Unsupported query parameter value: objects are not supported for query parameters. Got: {value}"
    )]
    #[from(skip)]
    UnsupportedQueryParameterValue {
        /// The unsupported value that was provided.
        value: serde_json::Value,
    },

    /// Parameter value cannot be converted to the required format.
    ///
    /// Occurs when parameter values are incompatible with their target type.
    #[display("Unsupported parameter value: {message}. Got: {value}")]
    #[from(skip)]
    UnsupportedParameterValue {
        /// Specific error message describing the conversion failure.
        message: String,
        /// The value that failed to convert.
        value: serde_json::Value,
    },

    /// OpenAPI operation not found in the specification.
    ///
    /// Occurs when referencing operations that don't exist in the collected spec.
    #[display("Missing operation: {id}")]
    #[from(skip)]
    MissingOperation {
        /// The operation ID that was not found.
        id: String,
    },

    /// Server returned an internal error (HTTP 500).
    ///
    /// Occurs when the server encounters an internal error during request processing.
    #[display("Server error (500) with response body: {raw_body}")]
    #[from(skip)]
    ServerFailure {
        /// The response body containing error details.
        raw_body: String,
    },

    /// Data serialization failed.
    ///
    /// Occurs when request data cannot be converted to the required format.
    #[display("Serialization error: {message}")]
    #[from(skip)]
    SerializationError {
        /// Description of the serialization failure.
        message: String,
    },

    /// Server returned an unexpected HTTP status code.
    ///
    /// Occurs when the response status code doesn't match expected values.
    #[display("Unexpected status code {status_code}: {body}")]
    #[from(skip)]
    UnexpectedStatusCode {
        /// The unexpected HTTP status code received.
        status_code: u16,
        /// The response body for debugging.
        body: String,
    },

    /// JSON redaction operation failed.
    ///
    /// Occurs when applying redactions to JSON responses.
    #[cfg(feature = "redaction")]
    #[display("Redaction error: {message}")]
    #[from(skip)]
    RedactionError {
        /// Description of the redaction failure.
        message: String,
    },

    /// Response output type doesn't match expected type.
    ///
    /// Occurs when attempting operations on incompatible response types.
    #[display("Expected output type '{expected}' but got '{actual}'")]
    #[from(skip)]
    UnexpectedOutputType {
        /// The expected output type.
        expected: String,
        /// The actual output type received.
        actual: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::output::Output;

    #[test]
    fn test_api_client_error_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ApiClientError>();
        assert_sync::<ApiClientError>();
    }

    // Test custom error variants (those with #[from(skip)])
    #[test]
    fn test_call_result_required_error() {
        let error = ApiClientError::CallResultRequired;
        assert_eq!(error.to_string(), "Invalid state: expected a call result");
    }

    #[test]
    fn test_invalid_base_path_error() {
        let error = ApiClientError::InvalidBasePath {
            error: "contains invalid characters".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Invalid base path: contains invalid characters"
        );
    }

    #[test]
    fn test_json_error() {
        // Create a real JSON error by trying to parse invalid JSON
        let json_error = serde_json::from_str::<serde_json::Value>("{ invalid json").unwrap_err();
        let error = ApiClientError::JsonError {
            path: "/api/users".to_string(),
            error: json_error,
            body: "{ invalid json }".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Failed to deserialize JSON at '/api/users'"));
        assert!(error_str.contains("{ invalid json }"));
    }

    #[test]
    fn test_unsupported_json_output_error() {
        let output = Output::Bytes(vec![0xFF, 0xFE, 0xFD]);
        let error = ApiClientError::UnsupportedJsonOutput {
            output,
            name: "User",
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Unsupported output for User as JSON"));
        assert!(error_str.contains("Bytes"));
    }

    #[test]
    fn test_unsupported_text_output_error() {
        let output = Output::Bytes(vec![0xFF, 0xFE, 0xFD]);
        let error = ApiClientError::UnsupportedTextOutput { output };

        let error_str = error.to_string();
        assert!(error_str.contains("Unsupported output for text"));
        assert!(error_str.contains("Bytes"));
    }

    #[test]
    fn test_unsupported_bytes_output_error() {
        let output = Output::Empty;
        let error = ApiClientError::UnsupportedBytesOutput { output };

        let error_str = error.to_string();
        assert!(error_str.contains("Unsupported output for bytes"));
        assert!(error_str.contains("Empty"));
    }

    #[test]
    fn test_path_unresolved_error() {
        let error = ApiClientError::PathUnresolved {
            path: "/users/{id}/posts/{post_id}".to_string(),
            missings: vec!["id".to_string(), "post_id".to_string()],
        };

        let error_str = error.to_string();
        assert!(
            error_str.contains("Path '/users/{id}/posts/{post_id}' is missing required arguments")
        );
        assert!(error_str.contains("id"));
        assert!(error_str.contains("post_id"));
    }

    #[test]
    fn test_unsupported_query_parameter_value_error() {
        let value = serde_json::json!({"nested": {"object": "not supported"}});
        let error = ApiClientError::UnsupportedQueryParameterValue {
            value: value.clone(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Unsupported query parameter value"));
        assert!(error_str.contains("objects are not supported"));
    }

    #[test]
    fn test_unsupported_parameter_value_error() {
        let value = serde_json::json!({"complex": "object"});
        let error = ApiClientError::UnsupportedParameterValue {
            message: "nested objects not allowed".to_string(),
            value: value.clone(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Unsupported parameter value: nested objects not allowed"));
        assert!(error_str.contains("complex"));
    }

    #[test]
    fn test_missing_operation_error() {
        let error = ApiClientError::MissingOperation {
            id: "get-users-by-id".to_string(),
        };

        assert_eq!(error.to_string(), "Missing operation: get-users-by-id");
    }

    #[test]
    fn test_server_failure_error() {
        let error = ApiClientError::ServerFailure {
            raw_body: "Internal Server Error: Database connection failed".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Server error (500)"));
        assert!(error_str.contains("Database connection failed"));
    }

    #[test]
    fn test_serialization_error() {
        let error = ApiClientError::SerializationError {
            message: "Cannot serialize circular reference".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Serialization error: Cannot serialize circular reference"
        );
    }

    #[test]
    fn test_unexpected_status_code_error() {
        let error = ApiClientError::UnexpectedStatusCode {
            status_code: 418,
            body: "I'm a teapot".to_string(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("Unexpected status code 418"));
        assert!(error_str.contains("I'm a teapot"));
    }

    // Test automatic conversions from underlying error types
    #[test]
    fn test_from_reqwest_error() {
        // Create a simple URL parse error and convert it to reqwest error
        let url_error = url::ParseError::InvalidPort;
        let api_error: ApiClientError = url_error.into();

        match api_error {
            ApiClientError::UrlError(_) => {} // Expected - testing URL error conversion
            _ => panic!("Should convert to UrlError"),
        }
    }

    #[test]
    fn test_from_url_parse_error() {
        let url_error = url::ParseError::InvalidPort;
        let api_error: ApiClientError = url_error.into();

        match api_error {
            ApiClientError::UrlError(url::ParseError::InvalidPort) => {} // Expected
            _ => panic!("Should convert to UrlError"),
        }
    }

    #[test]
    fn test_from_json_error() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let api_error: ApiClientError = json_error.into();

        match api_error {
            ApiClientError::JsonValueError(_) => {} // Expected
            _ => panic!("Should convert to JsonValueError"),
        }
    }

    #[test]
    fn test_from_http_error() {
        // Create an HTTP error by trying to parse an invalid header name
        let invalid_header = http::HeaderName::from_bytes(b"invalid\0header").unwrap_err();
        let api_error: ApiClientError = invalid_header.into();

        match api_error {
            ApiClientError::InvalidHeaderName(_) => {} // Expected - testing InvalidHeaderName conversion
            _ => panic!("Should convert to InvalidHeaderName"),
        }
    }

    #[test]
    fn test_from_invalid_header_name() {
        let header_error = http::HeaderName::from_bytes(b"invalid\0header").unwrap_err();
        let api_error: ApiClientError = header_error.into();

        match api_error {
            ApiClientError::InvalidHeaderName(_) => {} // Expected
            _ => panic!("Should convert to InvalidHeaderName"),
        }
    }

    #[test]
    fn test_from_invalid_header_value() {
        // Header values cannot contain control characters (0x00-0x1F except tab)
        let header_error = http::HeaderValue::from_bytes(&[0x00]).unwrap_err();
        let api_error: ApiClientError = header_error.into();

        match api_error {
            ApiClientError::InvalidHeaderValue(_) => {} // Expected
            _ => panic!("Should convert to InvalidHeaderValue"),
        }
    }

    #[test]
    fn test_from_authentication_error() {
        let auth_error = AuthenticationError::InvalidBearerToken {
            message: "contains null byte".to_string(),
        };
        let api_error: ApiClientError = auth_error.into();

        match api_error {
            ApiClientError::AuthenticationError(_) => {} // Expected
            _ => panic!("Should convert to AuthenticationError"),
        }
    }

    // Test Debug implementation
    #[test]
    fn test_error_debug_implementation() {
        let error = ApiClientError::CallResultRequired;
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("CallResultRequired"));

        let error = ApiClientError::InvalidBasePath {
            error: "test".to_string(),
        };
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("InvalidBasePath"));
        assert!(debug_str.contains("test"));
    }

    // Test that errors implement the Error trait properly
    #[test]
    fn test_error_trait_implementation() {
        use std::error::Error;

        let error = ApiClientError::CallResultRequired;
        assert!(error.source().is_none());

        let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let error = ApiClientError::JsonValueError(json_error);
        assert!(error.source().is_some());
    }

    // Test error equality (where applicable)
    #[test]
    fn test_error_equality() {
        let error1 = ApiClientError::CallResultRequired;
        let error2 = ApiClientError::CallResultRequired;

        // These errors should produce the same string representation
        assert_eq!(error1.to_string(), error2.to_string());

        let error1 = ApiClientError::InvalidBasePath {
            error: "same error".to_string(),
        };
        let error2 = ApiClientError::InvalidBasePath {
            error: "same error".to_string(),
        };
        assert_eq!(error1.to_string(), error2.to_string());
    }

    // Test error context preservation
    #[test]
    fn test_error_context_preservation() {
        let path = "/complex/path/{id}";
        let missings = vec!["id".to_string(), "user_id".to_string()];
        let error = ApiClientError::PathUnresolved {
            path: path.to_string(),
            missings: missings.clone(),
        };

        let error_string = error.to_string();
        assert!(error_string.contains(path));
        for missing in &missings {
            assert!(error_string.contains(missing));
        }
    }

    // Test complex error scenarios
    #[test]
    fn test_json_error_with_large_body() {
        let large_body = "x".repeat(2000);
        let json_error = serde_json::from_str::<serde_json::Value>("{ invalid").unwrap_err();
        let error = ApiClientError::JsonError {
            path: "/api/data".to_string(),
            error: json_error,
            body: large_body.clone(),
        };

        let error_str = error.to_string();
        assert!(error_str.contains("/api/data"));
        assert!(error_str.contains(&large_body));
    }

    #[test]
    fn test_status_code_error_edge_cases() {
        // Test various HTTP status codes
        let error = ApiClientError::UnexpectedStatusCode {
            status_code: 999, // Invalid status code
            body: "unknown status".to_string(),
        };
        assert!(error.to_string().contains("999"));

        let error = ApiClientError::UnexpectedStatusCode {
            status_code: 0, // Edge case
            body: "".to_string(),
        };
        assert!(error.to_string().contains("0"));
    }

    // Test output error variants with different Output types
    #[test]
    fn test_output_errors_with_all_output_types() {
        // Test with Text output
        let text_output = Output::Text("some text".to_string());
        let error = ApiClientError::UnsupportedBytesOutput {
            output: text_output,
        };
        assert!(error.to_string().contains("Text"));

        // Test with JSON output
        let json_output =
            Output::Json(serde_json::to_string(&serde_json::json!({"key": "value"})).unwrap());
        let error = ApiClientError::UnsupportedTextOutput {
            output: json_output,
        };
        assert!(error.to_string().contains("Json"));

        // Test with Empty output
        let empty_output = Output::Empty;
        let error = ApiClientError::UnsupportedJsonOutput {
            output: empty_output,
            name: "TestType",
        };
        assert!(error.to_string().contains("Empty"));
        assert!(error.to_string().contains("TestType"));
    }
}
