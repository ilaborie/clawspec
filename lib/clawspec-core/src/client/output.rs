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

// TODO something like axum response extractor - https://github.com/ilaborie/clawspec/issues/24

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_empty_as_example_value() {
        let output = Output::Empty;

        let result = output.as_example_value().expect("should succeed");

        assert_eq!(result, None);
    }

    #[test]
    fn test_output_json_as_example_value() {
        let output = Output::Json(r#"{"name": "test", "value": 42}"#.to_string());

        let result = output
            .as_example_value()
            .expect("should succeed")
            .expect("should have example");

        insta::assert_debug_snapshot!(result, @r#"
        Object {
            "name": String("test"),
            "value": Number(42),
        }
        "#);
    }

    #[test]
    fn test_output_json_invalid_returns_error() {
        let output = Output::Json("invalid json".to_string());
        let result = output.as_example_value();
        assert!(result.is_err());
    }

    #[test]
    fn test_output_text_as_example_value() {
        let output = Output::Text("hello world".to_string());
        let result = output
            .as_example_value()
            .expect("should succeed")
            .expect("should have example");

        insta::assert_debug_snapshot!(result, @r#"String("hello world")"#);
    }

    #[test]
    fn test_output_bytes_as_example_value() {
        let output = Output::Bytes(vec![1, 2, 3, 4]);
        let result = output.as_example_value().expect("should succeed");
        assert_eq!(result, None);
    }

    #[test]
    fn test_output_other_as_example_value() {
        let output = Output::Other {
            body: "some other content".to_string(),
        };
        let result = output.as_example_value().expect("should succeed");
        assert_eq!(result, None);
    }
}
