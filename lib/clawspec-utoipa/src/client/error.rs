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

    #[display("invalid state, expected a call result")]
    CallResultRequired,

    #[display("Invalid base path: error: {error:?}")]
    InvalidBasePath {
        error: String,
    },

    #[display("fail to deserialize JSON at '{path}' because {error}\n{body}")]
    #[from(skip)]
    JsonError {
        path: String,
        error: serde_json::Error,
        body: String,
    },

    #[display("unsupported output for {name} as JSON:\n{output:?}")]
    #[from(skip)]
    UnsupportedJsonOutput {
        output: Output,
        name: &'static str,
    },

    #[display("unsupported output for text:\n{output:?}")]
    #[from(skip)]
    UnsupportedTextOutput {
        output: Output,
    },

    #[display("unsupported output for bytes:\n{output:?}")]
    #[from(skip)]
    UnsupportedBytesOutput {
        output: Output,
    },

    #[display("path '{path}' miss arguments: {missings:?}")]
    #[from(skip)]
    PathUnresolved {
        path: String,
        missings: Vec<String>,
    },

    #[display(
        "unsupported query parameter value: objects are not supported for query parameters. Got: {value}"
    )]
    #[from(skip)]
    UnsupportedQueryParameterValue {
        value: serde_json::Value,
    },

    #[display("unsupported parameter value: {message}. Got: {value}")]
    #[from(skip)]
    UnsupportedParameterValue {
        message: String,
        value: serde_json::Value,
    },

    #[display("Missing operation {id}")]
    #[from(skip)]
    MissingOperation {
        id: String,
    },

    #[display("having a 500 error with raw body: {raw_body}")]
    #[from(skip)]
    ServerFailure {
        raw_body: String,
    },

    #[display("serialization error: {message}")]
    #[from(skip)]
    SerializationError {
        message: String,
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
