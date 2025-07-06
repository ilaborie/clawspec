use headers::ContentType;
use serde::Serialize;
use utoipa::ToSchema;

use super::{ApiClientError, SchemaEntry};

#[derive(Clone, derive_more::Debug)]
pub struct CallBody {
    pub(super) content_type: ContentType,
    pub(super) entry: SchemaEntry,
    #[debug(ignore)]
    pub(super) data: Vec<u8>,
}

impl CallBody {
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

    // TODO form with serde_urlencoded - https://github.com/ilaborie/clawspec/issues/19
    // TODO raw with content_type + bytes - https://github.com/ilaborie/clawspec/issues/19
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
                type_name: "clawspec_utoipa::client::body::tests::TestData",
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
}
