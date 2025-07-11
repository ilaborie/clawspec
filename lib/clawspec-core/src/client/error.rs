use std::fmt::Debug;

use super::output::Output;

#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum ApiClientError {
    ReqwestError(reqwest::Error),
    UrlError(url::ParseError),
    HeadersError(headers::Error),
    HttpError(http::Error),
    InvalidHeaderName(http::header::InvalidHeaderName),
    InvalidHeaderValue(http::header::InvalidHeaderValue),
    JsonValueError(serde_json::Error),
    QuerySerializationError(serde_urlencoded::ser::Error),

    #[display("Invalid state: expected a call result")]
    CallResultRequired,

    #[display("Invalid base path: {error}")]
    InvalidBasePath {
        error: String,
    },

    #[display("Failed to deserialize JSON at '{path}': {error}\n{body}")]
    #[from(skip)]
    JsonError {
        path: String,
        error: serde_json::Error,
        body: String,
    },

    #[display("Unsupported output for {name} as JSON:\n{output:?}")]
    #[from(skip)]
    UnsupportedJsonOutput {
        output: Output,
        name: &'static str,
    },

    #[display("Unsupported output for text:\n{output:?}")]
    #[from(skip)]
    UnsupportedTextOutput {
        output: Output,
    },

    #[display("Unsupported output for bytes:\n{output:?}")]
    #[from(skip)]
    UnsupportedBytesOutput {
        output: Output,
    },

    #[display("Path '{path}' is missing required arguments: {missings:?}")]
    #[from(skip)]
    PathUnresolved {
        path: String,
        missings: Vec<String>,
    },

    #[display(
        "Unsupported query parameter value: objects are not supported for query parameters. Got: {value}"
    )]
    #[from(skip)]
    UnsupportedQueryParameterValue {
        value: serde_json::Value,
    },

    #[display("Unsupported parameter value: {message}. Got: {value}")]
    #[from(skip)]
    UnsupportedParameterValue {
        message: String,
        value: serde_json::Value,
    },

    #[display("Missing operation: {id}")]
    #[from(skip)]
    MissingOperation {
        id: String,
    },

    #[display("Server error (500) with response body: {raw_body}")]
    #[from(skip)]
    ServerFailure {
        raw_body: String,
    },

    #[display("Serialization error: {message}")]
    #[from(skip)]
    SerializationError {
        message: String,
    },

    #[display("Unexpected status code {status_code}: {body}")]
    #[from(skip)]
    UnexpectedStatusCode {
        status_code: u16,
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
