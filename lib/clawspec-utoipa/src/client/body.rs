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

    // TODO form with serde_urlencoded
    // TODO raw with content_type + bytes
}
