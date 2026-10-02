use serde::Serialize;
use utoipa::ToSchema;
use utoipa::openapi::path::{Parameter, ParameterIn};
use utoipa::openapi::{Content, RefOr, Required, Schema};

use crate::client::error::ApiClientError;
use crate::client::openapi::schema::Schemas;

const FORM_URL_ENCODED: &str = "application/x-www-form-urlencoded";

/// The whole query string of a request, described by a single type.
#[derive(Debug, Clone)]
pub(in crate::client) struct CallQueryString {
    name: String,
    encoded: String,
    schema: RefOr<Schema>,
    pub(in crate::client) schemas: Schemas,
}

impl CallQueryString {
    pub(in crate::client) fn new<T>(value: &T) -> Result<Self, ApiClientError>
    where
        T: Serialize + ToSchema + 'static,
    {
        let encoded = serde_urlencoded::to_string(value)?;
        let example = serde_json::to_value(value)?;
        let mut schemas = Schemas::default();
        let schema = schemas.add_example::<T>(example);
        Ok(Self {
            name: T::name().into_owned(),
            encoded,
            schema,
            schemas,
        })
    }

    pub(in crate::client) fn encoded(&self) -> &str {
        &self.encoded
    }

    pub(in crate::client) fn to_parameter(&self) -> Parameter {
        Parameter::builder()
            .name(&self.name)
            .parameter_in(ParameterIn::QueryString)
            .required(Required::False)
            .content(
                FORM_URL_ENCODED,
                Content::builder().schema(Some(self.schema.clone())).build(),
            )
            .build()
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use serde::Serialize;
    use utoipa::ToSchema;

    use super::*;

    #[derive(Serialize, ToSchema)]
    struct Filter {
        search: String,
        limit: Option<u32>,
        offset: Option<u32>,
        active: bool,
    }

    #[derive(Serialize, ToSchema)]
    struct Range {
        min: u32,
        max: u32,
    }

    #[derive(Serialize, ToSchema)]
    struct NestedFilter {
        range: Range,
    }

    #[test]
    fn should_encode_flat_struct_and_skip_missing_options() {
        let querystring = CallQueryString::new(&Filter {
            search: "hello world&more".to_string(),
            limit: Some(10),
            offset: None,
            active: true,
        })
        .expect("flat struct should encode");

        assert_snapshot!(querystring.encoded(), @"search=hello+world%26more&limit=10&active=true");
    }

    #[test]
    fn should_reject_nested_object() {
        let error = CallQueryString::new(&NestedFilter {
            range: Range { min: 1, max: 2 },
        })
        .expect_err("nested object should not encode");

        assert!(matches!(error, ApiClientError::QuerySerializationError(_)));
    }

    #[test]
    fn should_emit_querystring_parameter() {
        let querystring = CallQueryString::new(&Filter {
            search: "rust".to_string(),
            limit: None,
            offset: None,
            active: false,
        })
        .expect("flat struct should encode");

        let yaml =
            serde_saphyr::to_string(&querystring.to_parameter()).expect("should serialize to YAML");
        assert_snapshot!(yaml, @r##"
        name: Filter
        in: querystring
        required: false
        content:
          application/x-www-form-urlencoded:
            schema:
              $ref: "#/components/schemas/Filter"
        "##);
    }
}
