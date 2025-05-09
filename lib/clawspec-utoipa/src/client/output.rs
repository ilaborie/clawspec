use tracing::info;

use super::ApiClientError;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Output {
    Empty,
    Json(String),
    Text(String),
    Bytes(Vec<u8>),
    Other { body: String },
}

impl Output {
    pub fn as_example_value(&self) -> Result<Option<serde_json::Value>, ApiClientError> {
        let value = match self {
            Self::Json(json) => serde_json::from_str(json)?,
            Self::Text(text) => serde_json::Value::String(text.clone()),
            Self::Empty | Self::Bytes(_) | Self::Other { .. } => {
                info!("skip example for this output");
                return Ok(None);
            }
        };
        Ok(Some(value))
    }
}

// TODO something like axum response extractor
