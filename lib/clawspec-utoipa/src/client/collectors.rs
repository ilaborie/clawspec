use std::any::type_name;
use std::mem;
use std::sync::Arc;

use headers::{ContentType, Header};
use http::header::CONTENT_TYPE;
use http::{Method, StatusCode};
use indexmap::IndexMap;
use reqwest::Response;
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;
use tracing::{error, warn};
use utoipa::ToSchema;
use utoipa::openapi::path::{Operation, Parameter};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{Content, PathItem, RefOr, ResponseBuilder, Schema};

use super::output::Output;
use super::schema::Schemas;
use super::{ApiClientError, CallBody, CallHeaders, CallPath, CallQuery};

/// Normalizes content types for OpenAPI specification by removing parameters
/// that are implementation details (like multipart boundaries, charset, etc.).
fn normalize_content_type(content_type: &ContentType) -> String {
    let content_type_str = content_type.to_string();

    // Strip all parameters by truncating at the first semicolon
    if let Some(semicolon_pos) = content_type_str.find(';') {
        content_type_str[..semicolon_pos].to_string()
    } else {
        content_type_str
    }
}

#[cfg(test)]
mod content_type_tests {
    use super::*;
    use headers::ContentType;

    #[test]
    fn test_normalize_json_content_type() {
        let content_type = ContentType::json();
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/json");
    }

    #[test]
    fn test_normalize_multipart_content_type() {
        // Create a multipart content type with boundary
        let content_type_str = "multipart/form-data; boundary=----formdata-clawspec-12345";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "multipart/form-data");
    }

    #[test]
    fn test_normalize_form_urlencoded_content_type() {
        let content_type = ContentType::form_url_encoded();
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/x-www-form-urlencoded");
    }

    #[test]
    fn test_normalize_content_type_with_charset() {
        // Test content type with charset parameter
        let content_type_str = "application/json; charset=utf-8";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/json");
    }

    #[test]
    fn test_normalize_content_type_with_multiple_parameters() {
        // Test content type with multiple parameters
        let content_type_str = "text/html; charset=utf-8; boundary=something";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "text/html");
    }

    #[test]
    fn test_normalize_content_type_without_parameters() {
        // Test content type without parameters (should remain unchanged)
        let content_type_str = "application/xml";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/xml");
    }
}

// TODO: Add unit tests for all collector functionality - https://github.com/ilaborie/clawspec/issues/30
// TODO: Optimize clone-heavy merge operations - https://github.com/ilaborie/clawspec/issues/31
#[derive(Debug, Clone, Default)]
pub(super) struct Collectors {
    operations: IndexMap<String, Vec<CalledOperation>>,
    schemas: Schemas,
}

impl Collectors {
    pub(super) fn collect_schemas(&mut self, schemas: Schemas) {
        self.schemas.merge(schemas);
    }

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

/// Represents a called operation with its metadata and potential result.
///
/// This struct stores information about an API operation that has been called,
/// including its identifier, HTTP method, path, and the actual operation definition.
/// It can optionally contain a result if the operation has been executed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub(super) struct CalledOperation {
    pub(super) operation_id: String,
    method: http::Method,
    path: String,
    operation: Operation,
    result: Option<CallResult>,
}

/// Represents the result of an API call with response processing capabilities.
///
/// This struct contains the response from an HTTP request along with methods to
/// process the response in various formats (JSON, text, bytes, etc.) while
/// automatically collecting OpenAPI schema information.
#[derive(Debug, Clone)]
pub struct CallResult {
    operation_id: String,
    status: StatusCode,
    content_type: Option<ContentType>,
    output: Output,
    collectors: Arc<RwLock<Collectors>>,
}

impl CallResult {
    pub(super) async fn new(
        operation_id: String,
        collectors: Arc<RwLock<Collectors>>,
        response: Response,
    ) -> Result<Self, ApiClientError> {
        let status = response.status();
        let content_type = response
            .headers()
            .get_all(CONTENT_TYPE)
            .iter()
            .collect::<Vec<_>>();
        let content_type = if content_type.is_empty() {
            None
        } else {
            let ct = ContentType::decode(&mut content_type.into_iter())?;
            Some(ct)
        };

        let output = if let Some(content_type) = content_type.clone()
            && status != StatusCode::NO_CONTENT
        {
            if content_type == ContentType::json() {
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
            }
        } else {
            Output::Empty
        };

        Ok(Self {
            operation_id,
            status,
            content_type,
            output,
            collectors,
        })
    }

    async fn get_output(&self, schema: Option<RefOr<Schema>>) -> Result<&Output, ApiClientError> {
        // add operation response desc
        let mut cs = self.collectors.write().await;
        let Some(operation) = cs.operations.get_mut(&self.operation_id) else {
            return Err(ApiClientError::MissingOperation {
                id: self.operation_id.clone(),
            });
        };

        let Some(operation) = operation.last_mut() else {
            return Err(ApiClientError::MissingOperation {
                id: self.operation_id.clone(),
            });
        };

        let response = if let Some(content_type) = &self.content_type {
            // Create content
            let content = Content::builder().schema(schema).build();
            ResponseBuilder::new()
                .content(content_type.to_string(), content)
                .build()
        } else {
            // Empty response
            ResponseBuilder::new().build()
        };

        operation
            .operation
            .responses
            .responses
            .insert(self.status.as_u16().to_string(), RefOr::T(response));

        Ok(&self.output)
    }

    /// Processes the response as JSON and deserializes it to the specified type.
    ///
    /// This method automatically records the response schema in the OpenAPI specification
    /// and processes the response body as JSON. The type parameter must implement
    /// `DeserializeOwned` and `ToSchema` for proper JSON parsing and schema generation.
    ///
    /// # Type Parameters
    ///
    /// - `T`: The target type for deserialization, must implement `DeserializeOwned`, `ToSchema`, and `'static`
    ///
    /// # Returns
    ///
    /// - `Ok(T)`: The deserialized response object
    /// - `Err(ApiClientError)`: If the response is not JSON or deserialization fails
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # use serde::{Deserialize, Serialize};
    /// # use utoipa::ToSchema;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #[derive(Deserialize, ToSchema)]
    /// struct User {
    ///     id: u32,
    ///     name: String,
    /// }
    ///
    /// let mut client = ApiClient::builder().build()?;
    /// let user: User = client
    ///     .get("/users/123")?
    ///     .exchange()
    ///     .await?
    ///     .as_json()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_json<T>(&mut self) -> Result<T, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
    {
        let mut cs = self.collectors.write().await;
        let schema = cs.schemas.add::<T>();
        mem::drop(cs);
        let output = self.get_output(Some(schema)).await?;

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
            let mut cs = self.collectors.write().await;
            cs.schemas.add_example::<T>(example);
        }

        Ok(result)
    }

    /// Processes the response as plain text.
    ///
    /// This method records the response in the OpenAPI specification and returns
    /// the response body as a string slice. The response must have a text content type.
    ///
    /// # Returns
    ///
    /// - `Ok(&str)`: The response body as a string slice
    /// - `Err(ApiClientError)`: If the response is not text
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let text = client
    ///     .get("/api/status")?
    ///     .exchange()
    ///     .await?
    ///     .as_text()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_text(&mut self) -> Result<&str, ApiClientError> {
        let output = self.get_output(None).await?;

        let Output::Text(text) = &output else {
            return Err(ApiClientError::UnsupportedTextOutput {
                output: output.clone(),
            });
        };

        Ok(text)
    }

    /// Processes the response as binary data.
    ///
    /// This method records the response in the OpenAPI specification and returns
    /// the response body as a byte slice. The response must have a binary content type.
    ///
    /// # Returns
    ///
    /// - `Ok(&[u8])`: The response body as a byte slice
    /// - `Err(ApiClientError)`: If the response is not binary
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let bytes = client
    ///     .get("/api/download")?
    ///     .exchange()
    ///     .await?
    ///     .as_bytes()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_bytes(&mut self) -> Result<&[u8], ApiClientError> {
        let output = self.get_output(None).await?;

        let Output::Bytes(bytes) = &output else {
            return Err(ApiClientError::UnsupportedBytesOutput {
                output: output.clone(),
            });
        };

        Ok(bytes.as_slice())
    }

    /// Processes the response as raw content with content type information.
    ///
    /// This method records the response in the OpenAPI specification and returns
    /// the response body as a string along with its content type. If the response
    /// has no content type, returns `None`.
    ///
    /// # Returns
    ///
    /// - `Ok(Some((ContentType, &str)))`: The content type and response body
    /// - `Ok(None)`: If the response has no content type
    /// - `Err(ApiClientError)`: If processing fails
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// if let Some((content_type, body)) = client
    ///     .get("/api/data")?
    ///     .exchange()
    ///     .await?
    ///     .as_raw()
    ///     .await?
    /// {
    ///     println!("Content-Type: {}", content_type);
    ///     println!("Body: {}", body);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_raw(&mut self) -> Result<Option<(ContentType, &str)>, ApiClientError> {
        let Some(content_type) = self.content_type.clone() else {
            return Ok(None);
        };
        let output = self.get_output(None).await?;

        let body = match output {
            Output::Empty => "",
            Output::Json(body) | Output::Text(body) | Output::Other { body, .. } => body.as_str(),
            Output::Bytes(_bytes) => todo!("base64 encoding"),
        };

        Ok(Some((content_type, body)))
    }

    /// Records this response as an empty response in the OpenAPI specification.
    ///
    /// This method should be used for endpoints that return no content (e.g., DELETE operations,
    /// PUT operations that don't return a response body).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// client
    ///     .delete("/items/123")?
    ///     .exchange()
    ///     .await?
    ///     .as_empty()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_empty(&mut self) -> Result<(), ApiClientError> {
        self.get_output(None).await?;
        Ok(())
    }
}

impl CalledOperation {
    pub(super) fn build(
        operation_id: String,
        method: http::Method,
        path_name: &str,
        path: &CallPath,
        query: CallQuery,
        headers: Option<&CallHeaders>,
        request_body: Option<&CallBody>,
        // TODO cookie - https://github.com/ilaborie/clawspec/issues/18
    ) -> Self {
        // Build parameters
        let mut parameters: Vec<_> = path.to_parameters().collect();

        let mut schemas = path.schemas().clone();

        // Add query parameters
        if !query.is_empty() {
            parameters.extend(query.to_parameters());
            schemas.merge(query.schemas);
        }

        // Add header parameters
        if let Some(headers) = headers {
            parameters.extend(headers.to_parameters());
            schemas.merge(headers.schemas().clone());
        }

        let builder = Operation::builder()
            .operation_id(Some(&operation_id))
            .parameters(Some(parameters));

        // Request body
        let builder = if let Some(body) = request_body {
            let schema_ref = schemas.add_entry(body.entry.clone());
            let content_type = normalize_content_type(&body.content_type);
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
            path: path_name.to_string(),
            operation,
            result: None,
        }
    }

    pub(super) fn add_response(&mut self, call_result: CallResult) {
        self.result = Some(call_result);
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
        .request_body(merge_request_body(current.request_body, new.request_body))
        .deprecated(current.deprecated.or(new.deprecated))
        // TODO security - https://github.com/ilaborie/clawspec/issues/23
        // TODO servers - https://github.com/ilaborie/clawspec/issues/23
        // extension
        .responses(merge_responses(current.responses, new.responses));
    Some(operation.build())
}

fn merge_request_body(
    current: Option<RequestBody>,
    new: Option<RequestBody>,
) -> Option<RequestBody> {
    match (current, new) {
        (Some(current), Some(new)) => {
            // Merge content types from both request bodies
            let mut merged_content = current.content.clone();
            for (content_type, content) in new.content {
                merged_content.insert(content_type, content);
            }

            let mut merged_builder = RequestBody::builder();
            for (content_type, content) in merged_content {
                merged_builder = merged_builder.content(content_type, content);
            }

            let merged = merged_builder
                .description(current.description.or(new.description))
                .required(current.required.or(new.required))
                .build();

            Some(merged)
        }
        (Some(current), None) => Some(current),
        (None, Some(new)) => Some(new),
        (None, None) => None,
    }
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

fn merge_responses(
    current: utoipa::openapi::Responses,
    new: utoipa::openapi::Responses,
) -> utoipa::openapi::Responses {
    use utoipa::openapi::ResponsesBuilder;

    let mut merged_responses = IndexMap::new();

    // Add responses from new operation first
    for (status, response) in new.responses {
        merged_responses.insert(status, response);
    }

    // Add responses from current operation, preferring new ones
    for (status, response) in current.responses {
        merged_responses.entry(status).or_insert(response);
    }

    let mut builder = ResponsesBuilder::new();
    for (status, response) in merged_responses {
        builder = builder.response(status, response);
    }

    builder.build()
}
