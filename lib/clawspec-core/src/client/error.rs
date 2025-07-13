use std::fmt::Debug;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_error_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ApiClientError>();
        assert_sync::<ApiClientError>();
    }
}
