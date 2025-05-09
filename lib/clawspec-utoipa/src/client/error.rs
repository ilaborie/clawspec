use super::output::Output;

#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum ApiClientError {
    ReqwestError(reqwest::Error),
    UrlError(url::ParseError),
    HeadersError(headers::Error),
    JsonValueError(serde_json::Error),

    #[display("invalid state, expected a call result")]
    CallResultRequired,

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

    #[display("having a 500 error with raw body: {raw_body}")]
    #[from(skip)]
    ServerFailure {
        raw_body: String,
    },
}
