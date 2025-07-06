use std::any::type_name;

use headers::{ContentType, Header};
use http::header::CONTENT_TYPE;
use http::{Method, StatusCode};
use indexmap::IndexMap;
use reqwest::Response;
use serde::de::DeserializeOwned;
use tracing::{error, warn};
use utoipa::ToSchema;
use utoipa::openapi::path::{Operation, Parameter, ParameterIn};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{Content, PathItem, RefOr, Required, ResponseBuilder, Schema};

use super::output::Output;
use super::{ApiClientError, CallBody, CallHeaders, CallQuery, PathParam, Schemas};

// TODO: Add unit tests for all collector functionality - https://github.com/ilaborie/clawspec/issues/30
// TODO: Optimize clone-heavy merge operations - https://github.com/ilaborie/clawspec/issues/31
#[derive(Debug, Clone, Default)]
pub(super) struct Collectors {
    operations: IndexMap<String, Vec<CalledOperation>>,
    schemas: Schemas,
}

impl Collectors {
    pub(super) fn collect_operation(
        &mut self,
        operation: CalledOperation,
    ) -> Option<&mut CalledOperation> {
        let operation_id = operation.operation_id.clone();
        let operations = self.operations.entry(operation_id).or_default();

        operations.push(operation);
        operations.last_mut()
    }

    pub(super) fn schemas(&self) -> Vec<(String, RefOr<Schema>)> {
        self.schemas.schema_vec()
    }

    pub(super) fn as_map(&mut self, base_path: &str) -> IndexMap<String, PathItem> {
        let mut result = IndexMap::<String, PathItem>::new();
        for (operation_id, calls) in &self.operations {
            debug_assert!(!calls.is_empty(), "having at least a call");
            let path = format!("{base_path}/{}", calls[0].path.trim_start_matches('/'));
            let item = result.entry(path.clone()).or_default();
            for call in calls {
                let method = call.method.clone();
                self.schemas.merge(call.schemas.clone());
                match method {
                    Method::GET => {
                        item.get =
                            merge_operation(operation_id, item.get.clone(), call.operation.clone());
                    }
                    Method::PUT => {
                        item.put =
                            merge_operation(operation_id, item.put.clone(), call.operation.clone());
                    }
                    Method::POST => {
                        item.post = merge_operation(
                            operation_id,
                            item.post.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::DELETE => {
                        item.delete = merge_operation(
                            operation_id,
                            item.delete.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::OPTIONS => {
                        item.options = merge_operation(
                            operation_id,
                            item.options.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::HEAD => {
                        item.head = merge_operation(
                            operation_id,
                            item.head.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::PATCH => {
                        item.patch = merge_operation(
                            operation_id,
                            item.patch.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::TRACE => {
                        item.trace = merge_operation(
                            operation_id,
                            item.trace.clone(),
                            call.operation.clone(),
                        );
                    }
                    _ => {
                        warn!(%method, "unsupported method");
                    }
                }
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CalledOperation {
    pub(super) operation_id: String,
    method: http::Method,
    path: String,
    operation: Operation,
    result: Option<CallResult>,
    pub(super) schemas: Schemas,
}

#[derive(Debug, Clone)]
struct CallResult {
    status: StatusCode,
    content_type: ContentType,
    output: Output,
}

impl CalledOperation {
    pub(super) fn build(
        operation_id: String,
        method: http::Method,
        path: &str,
        path_params: &[PathParam],
        query: Option<&CallQuery>,
        headers: Option<&CallHeaders>,
        request_body: Option<&CallBody>,
        // TODO cookie - https://github.com/ilaborie/clawspec/issues/18
    ) -> Self {
        let mut schemas = Schemas::default();

        // Build parameters
        let mut parameters = vec![];
        for path_param in path_params {
            let PathParam(name) = path_param;
            let param = Parameter::builder()
                .name(name)
                .required(Required::True)
                .parameter_in(ParameterIn::Path)
                .build();
            parameters.push(param);
        }

        // TODO query - https://github.com/ilaborie/clawspec/issues/20
        if let Some(_query) = query {
            todo!("add query parameters");
        }

        // TODO headers - https://github.com/ilaborie/clawspec/issues/20
        if let Some(_headers) = headers {
            todo!("add headers parameters");
        }

        let builder = Operation::builder()
            .operation_id(Some(&operation_id))
            .parameters(Some(parameters));

        // Request body
        let builder = if let Some(body) = request_body {
            let schema_ref = schemas.add_entry(body.entry.clone());
            let content_type = body.content_type.to_string();
            let example = if body.content_type == ContentType::json() {
                serde_json::from_slice(&body.data).ok()
            } else {
                None
            };

            let content = Content::builder()
                .schema(Some(schema_ref))
                .example(example)
                .build();
            let request_body = RequestBody::builder()
                .content(content_type, content)
                .build();
            builder.request_body(Some(request_body))
        } else {
            builder
        };

        let operation = builder.build();
        Self {
            operation_id,
            method,
            path: path.to_string(),
            operation,
            result: None,
            schemas,
        }
    }

    pub(super) async fn add_response(&mut self, response: Response) -> Result<(), ApiClientError> {
        let status = response.status();

        let content_type = response
            .headers()
            .get_all(CONTENT_TYPE)
            .iter()
            .collect::<Vec<_>>();
        let content_type = ContentType::decode(&mut content_type.into_iter())?;

        let output = if status == StatusCode::NO_CONTENT {
            Output::Empty
        } else if content_type == ContentType::json() {
            let json = response.text().await?;
            Output::Json(json)
        } else if content_type == ContentType::octet_stream() {
            let bytes = response.bytes().await?;
            Output::Bytes(bytes.to_vec())
        } else if content_type.to_string().starts_with("text/") {
            let text = response.text().await?;
            Output::Text(text)
        } else {
            let body = response.text().await?;
            Output::Other { body }
        };

        self.result = Some(CallResult {
            status,
            content_type,
            output,
        });

        Ok(())
    }

    fn get_output(&mut self, schema: Option<RefOr<Schema>>) -> Result<&Output, ApiClientError> {
        let Some(CallResult {
            status,
            content_type,
            output,
        }) = &self.result
        else {
            return Err(ApiClientError::CallResultRequired);
        };

        // Create content
        let content = Content::builder().schema(schema).build();

        // add operation response desc
        self.operation.responses.responses.insert(
            status.as_u16().to_string(),
            RefOr::T(
                ResponseBuilder::new()
                    .content(content_type.to_string(), content)
                    .build(),
            ),
        );

        Ok(output)
    }

    pub fn as_json<T>(&mut self) -> Result<T, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
    {
        let schema = self.schemas.add::<T>();
        let output = self.get_output(Some(schema))?;

        let Output::Json(json) = output else {
            return Err(ApiClientError::UnsupportedJsonOutput {
                output: output.clone(),
                name: type_name::<T>(),
            });
        };
        let deserializer = &mut serde_json::Deserializer::from_str(json.as_str());
        let result = serde_path_to_error::deserialize(deserializer).map_err(|err| {
            ApiClientError::JsonError {
                path: err.path().to_string(),
                error: err.into_inner(),
                body: json.clone(),
            }
        })?;

        if let Ok(example) = serde_json::to_value(json.as_str()) {
            self.schemas.add_example::<T>(example);
        }

        Ok(result)
    }

    pub fn as_text(&mut self) -> Result<&str, ApiClientError> {
        let output = self.get_output(None)?;

        let Output::Text(text) = &output else {
            return Err(ApiClientError::UnsupportedTextOutput {
                output: output.clone(),
            });
        };

        Ok(text)
    }

    pub fn as_bytes(&mut self) -> Result<&[u8], ApiClientError> {
        let output = self.get_output(None)?;

        let Output::Bytes(bytes) = &output else {
            return Err(ApiClientError::UnsupportedBytesOutput {
                output: output.clone(),
            });
        };

        Ok(bytes.as_slice())
    }

    pub fn as_raw(&mut self) -> Result<(ContentType, &str), ApiClientError> {
        let content_type = self.result.as_ref().cloned().expect("exist").content_type;
        let output = self.get_output(None)?;

        let body = match output {
            Output::Empty => "",
            Output::Json(body) | Output::Text(body) | Output::Other { body, .. } => body.as_str(),
            Output::Bytes(_bytes) => todo!("base64 encoding"),
        };

        Ok((content_type, body))
    }
}

fn merge_operation(id: &str, current: Option<Operation>, new: Operation) -> Option<Operation> {
    let Some(current) = current else {
        return Some(new);
    };

    let current_id = current.operation_id.as_deref().unwrap_or_default();
    if current_id != id {
        error!("conflicting operation id {id} with {current_id}");
        return None;
    }

    let operation = Operation::builder()
        .tags(merge_tags(current.tags, new.tags))
        .description(current.description.or(new.description))
        .operation_id(Some(id))
        // external_docs
        .parameters(merge_parameters(current.parameters, new.parameters))
        // TODO body - https://github.com/ilaborie/clawspec/issues/19
        .deprecated(current.deprecated.or(new.deprecated))
        // TODO security - https://github.com/ilaborie/clawspec/issues/23
        // TODO servers - https://github.com/ilaborie/clawspec/issues/23
        // extension
        ;
    // resp
    Some(operation.build())
}

fn merge_tags(current: Option<Vec<String>>, new: Option<Vec<String>>) -> Option<Vec<String>> {
    let Some(mut current) = current else {
        return new;
    };
    let Some(new) = new else {
        return Some(current);
    };

    current.extend(new);
    current.sort();
    current.dedup();

    Some(current)
}

fn merge_parameters(
    current: Option<Vec<Parameter>>,
    new: Option<Vec<Parameter>>,
) -> Option<Vec<Parameter>> {
    let mut result = IndexMap::new();
    for param in new.unwrap_or_default() {
        result.insert(param.name.clone(), param);
    }
    for param in current.unwrap_or_default() {
        result.insert(param.name.clone(), param);
    }

    let result = result.into_values().collect();
    Some(result)
}
