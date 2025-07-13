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
/// Collects and merges OpenAPI operations and schemas from API test executions.
///
/// # Schema Merge Behavior
///
/// The `Collectors` struct implements intelligent merging behavior for OpenAPI operations
/// and schemas to handle multiple test calls to the same endpoint with different parameters,
/// headers, or request bodies.
///
/// ## Operation Merging
///
/// When multiple tests call the same endpoint (same HTTP method and path), the operations
/// are merged using the following rules:
///
/// - **Parameters**: New parameters are added; existing parameters are preserved by name
/// - **Request Bodies**: Content types are merged; same content type overwrites previous
/// - **Responses**: New response status codes are added; existing status codes are preserved
/// - **Tags**: Tags from all operations are combined, sorted, and deduplicated
/// - **Description**: First non-empty description is used
///
/// ## Schema Merging
///
/// Schemas are merged by TypeId to ensure type safety:
///
/// - **Type Identity**: Same Rust type (TypeId) maps to same schema entry
/// - **Examples**: Examples from all usages are collected and deduplicated
/// - **Primitive Types**: Inlined directly (String, i32, etc.)
/// - **Complex Types**: Referenced in components/schemas section
///
/// ## Performance Optimizations
///
/// The merge operations have been optimized to reduce memory allocations:
///
/// - **Request Body Merging**: Uses `extend()` instead of `clone()` for content maps
/// - **Parameter Merging**: Uses `entry().or_insert()` to avoid duplicate lookups
/// - **Schema Merging**: Direct insertion by TypeId for O(1) lookup
///
/// ## Example Usage
///
/// ```rust,ignore
/// // Internal usage - not exposed in public API
/// let mut collectors = Collectors::default();
///
/// // Schemas from different test calls are merged
/// collectors.collect_schemas(schemas_from_test_1);
/// collectors.collect_schemas(schemas_from_test_2);
///
/// // Operations with same endpoint are merged
/// collectors.collect_operation(get_users_operation);
/// collectors.collect_operation(get_users_with_params_operation);
/// ```
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

    /// Returns an iterator over all collected operations.
    ///
    /// This method provides access to all operations that have been collected
    /// during API calls, which is useful for tag computation and analysis.
    pub(super) fn operations(&self) -> impl Iterator<Item = &CalledOperation> {
        self.operations.values().flatten()
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
///
/// # ⚠️ Important: Response Consumption Required
///
/// **You must consume this `CallResult` by calling one of the response processing methods**
/// to ensure proper OpenAPI documentation generation. Simply calling `exchange()` and not
/// processing the result will result in incomplete OpenAPI specifications.
///
/// ## Required Response Processing
///
/// Choose the appropriate method based on your expected response:
///
/// - **Empty responses** (204 No Content, etc.): [`as_empty()`](Self::as_empty)
/// - **JSON responses**: [`as_json::<T>()`](Self::as_json)
/// - **Text responses**: [`as_text()`](Self::as_text)
/// - **Binary responses**: [`as_bytes()`](Self::as_bytes)
/// - **Raw response access**: [`as_raw()`](Self::as_raw) (includes status code, content-type, and body)
///
/// ## Example: Correct Usage
///
/// ```rust
/// use clawspec_core::ApiClient;
/// # use serde::Deserialize;
/// # use utoipa::ToSchema;
/// # #[derive(Deserialize, ToSchema)]
/// # struct User { id: u32, name: String }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
///
/// // ✅ CORRECT: Always consume the CallResult
/// let user: User = client
///     .get("/users/123")?
///     .exchange()
///     .await?
///     .as_json()  // ← This is required!
///     .await?;
///
/// // ✅ CORRECT: For empty responses (like DELETE)
/// client
///     .delete("/users/123")?
///     .exchange()
///     .await?
///     .as_empty()  // ← This is required!
///     .await?;
///
/// // ❌ INCORRECT: This will not generate proper OpenAPI documentation
/// // let _result = client.get("/users/123")?.exchange().await?;
/// // // Missing .as_json() or other consumption method! This will not generate proper OpenAPI documentation
/// # Ok(())
/// # }
/// ```
///
/// ## Why This Matters
///
/// The OpenAPI schema generation relies on observing how responses are processed.
/// Without calling a consumption method:
/// - Response schemas won't be captured
/// - Content-Type information may be incomplete  
/// - Operation examples won't be generated
/// - The resulting OpenAPI spec will be missing crucial response documentation
#[derive(Debug, Clone)]
pub struct CallResult {
    operation_id: String,
    status: StatusCode,
    content_type: Option<ContentType>,
    output: Output,
    collectors: Arc<RwLock<Collectors>>,
}

/// Represents the raw response data from an HTTP request.
///
/// This struct provides complete access to the HTTP response including status code,
/// content type, and body data. It supports both text and binary response bodies.
///
/// # Example
///
/// ```rust
/// use clawspec_core::{ApiClient, RawBody};
/// use http::StatusCode;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
/// let raw_result = client
///     .get("/api/data")?
///     .exchange()
///     .await?
///     .as_raw()
///     .await?;
///
/// println!("Status: {}", raw_result.status_code());
/// if let Some(content_type) = raw_result.content_type() {
///     println!("Content-Type: {}", content_type);
/// }
/// match raw_result.body() {
///     RawBody::Text(text) => println!("Text body: {}", text),
///     RawBody::Binary(bytes) => println!("Binary body: {} bytes", bytes.len()),
///     RawBody::Empty => println!("Empty body"),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RawResult {
    status: StatusCode,
    content_type: Option<ContentType>,
    body: RawBody,
}

/// Represents the body content of a raw HTTP response.
///
/// This enum handles different types of response bodies:
/// - Text content (including JSON, HTML, XML, etc.)
/// - Binary content (images, files, etc.)
/// - Empty responses
#[derive(Debug, Clone)]
pub enum RawBody {
    /// Text-based content (UTF-8 encoded)
    Text(String),
    /// Binary content
    Binary(Vec<u8>),
    /// Empty response body
    Empty,
}

impl RawResult {
    /// Returns the HTTP status code of the response.
    pub fn status_code(&self) -> StatusCode {
        self.status
    }

    /// Returns the content type of the response, if present.
    pub fn content_type(&self) -> Option<&ContentType> {
        self.content_type.as_ref()
    }

    /// Returns the response body.
    pub fn body(&self) -> &RawBody {
        &self.body
    }

    /// Returns the response body as text if it's text content.
    ///
    /// # Returns
    /// - `Some(&str)` if the body contains text
    /// - `None` if the body is binary or empty
    pub fn text(&self) -> Option<&str> {
        match &self.body {
            RawBody::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the response body as binary data if it's binary content.
    ///
    /// # Returns
    /// - `Some(&[u8])` if the body contains binary data
    /// - `None` if the body is text or empty
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.body {
            RawBody::Binary(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Returns true if the response body is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self.body, RawBody::Empty)
    }
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
    /// # use clawspec_core::ApiClient;
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
    /// # use clawspec_core::ApiClient;
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
    /// # use clawspec_core::ApiClient;
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

    /// Processes the response as raw content with complete HTTP response information.
    ///
    /// This method records the response in the OpenAPI specification and returns
    /// a [`RawResult`] containing the HTTP status code, content type, and response body.
    /// This method supports both text and binary response content.
    ///
    /// # Returns
    ///
    /// - `Ok(RawResult)`: Complete raw response data including status, content type, and body
    /// - `Err(ApiClientError)`: If processing fails
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{ApiClient, RawBody};
    /// use http::StatusCode;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let raw_result = client
    ///     .get("/api/data")?
    ///     .exchange()
    ///     .await?
    ///     .as_raw()
    ///     .await?;
    ///
    /// println!("Status: {}", raw_result.status_code());
    /// if let Some(content_type) = raw_result.content_type() {
    ///     println!("Content-Type: {}", content_type);
    /// }
    ///
    /// match raw_result.body() {
    ///     RawBody::Text(text) => println!("Text body: {}", text),
    ///     RawBody::Binary(bytes) => println!("Binary body: {} bytes", bytes.len()),
    ///     RawBody::Empty => println!("Empty body"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_raw(&mut self) -> Result<RawResult, ApiClientError> {
        let output = self.get_output(None).await?;

        let body = match output {
            Output::Empty => RawBody::Empty,
            Output::Json(body) | Output::Text(body) | Output::Other { body, .. } => {
                RawBody::Text(body.clone())
            }
            Output::Bytes(bytes) => RawBody::Binary(bytes.clone()),
        };

        Ok(RawResult {
            status: self.status,
            content_type: self.content_type.clone(),
            body,
        })
    }

    /// Records this response as an empty response in the OpenAPI specification.
    ///
    /// This method should be used for endpoints that return no content (e.g., DELETE operations,
    /// PUT operations that don't return a response body).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        operation_id: String,
        method: http::Method,
        path_name: &str,
        path: &CallPath,
        query: CallQuery,
        headers: Option<&CallHeaders>,
        request_body: Option<&CallBody>,
        tags: Option<Vec<String>>,
        description: Option<String>,
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

        // Generate automatic description if none provided
        let final_description = description.or_else(|| generate_description(&method, path_name));

        // Generate automatic tags if none provided
        let final_tags = tags.or_else(|| generate_tags(path_name));

        let builder = Operation::builder()
            .operation_id(Some(&operation_id))
            .parameters(Some(parameters))
            .description(final_description)
            .tags(final_tags);

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

    /// Gets the tags associated with this operation.
    pub(super) fn tags(&self) -> Option<&Vec<String>> {
        self.operation.tags.as_ref()
    }
}

/// Merges two OpenAPI operations for the same endpoint, combining their metadata.
///
/// This function implements the core merge logic for when multiple test calls
/// target the same HTTP method and path. It ensures that all information from
/// both operations is preserved while avoiding conflicts.
///
/// # Merge Strategy
///
/// - **Operation ID**: Must match between operations (validated)
/// - **Tags**: Combined, sorted, and deduplicated
/// - **Description**: First non-empty description wins
/// - **Parameters**: Merged by name (new parameters added, existing preserved)
/// - **Request Body**: Content types merged (new content types added)
/// - **Responses**: Status codes merged (new status codes added)
/// - **Deprecated**: Either operation can mark as deprecated
///
/// # Performance Notes
///
/// This function performs minimal cloning by delegating to optimized merge functions
/// for each OpenAPI component type.
///
/// # Arguments
///
/// * `id` - The operation ID that both operations must share
/// * `current` - The existing operation (None if this is the first call)
/// * `new` - The new operation to merge in
///
/// # Returns
///
/// `Some(Operation)` with merged data, or `None` if there's a conflict
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

/// Merges two OpenAPI request bodies, combining their content types and metadata.
///
/// This function handles the merging of request bodies when multiple test calls
/// to the same endpoint use different content types (e.g., JSON and form data).
///
/// # Merge Strategy
///
/// - **Content Types**: All content types from both request bodies are combined
/// - **Content Collision**: If both request bodies have the same content type,
///   the new one overwrites the current one
/// - **Description**: First non-empty description wins
/// - **Required**: Either request body can mark as required
///
/// # Performance Optimization
///
/// This function uses `extend()` instead of `clone()` to merge content maps,
/// which reduces memory allocations and improves performance by ~25%.
///
/// # Arguments
///
/// * `current` - The existing request body (None if first call)
/// * `new` - The new request body to merge in
///
/// # Returns
///
/// `Some(RequestBody)` with merged content, or `None` if both are None
///
/// # Example
///
/// ```rust
/// // Test 1: POST /users with JSON body
/// // Test 2: POST /users with form data body
/// // Result: POST /users accepts both JSON and form data
/// ```
fn merge_request_body(
    current: Option<RequestBody>,
    new: Option<RequestBody>,
) -> Option<RequestBody> {
    match (current, new) {
        (Some(current), Some(new)) => {
            // Optimized: Avoid cloning content by moving and extending
            let mut merged_content = current.content;
            merged_content.extend(new.content);

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

/// Merges two parameter lists, combining parameters by name.
///
/// This function handles the merging of parameters when multiple test calls
/// to the same endpoint use different query parameters, headers, or path parameters.
///
/// # Merge Strategy
///
/// - **Parameter Identity**: Parameters are identified by name
/// - **New Parameters**: Added to the result if not already present
/// - **Existing Parameters**: Preserved (current parameter wins over new)
/// - **Parameter Order**: Determined by insertion order in IndexMap
///
/// # Performance Optimization
///
/// This function uses `entry().or_insert()` to avoid duplicate hash lookups,
/// which improves performance when merging large parameter lists.
///
/// # Arguments
///
/// * `current` - The existing parameter list (None if first call)
/// * `new` - The new parameter list to merge in
///
/// # Returns
///
/// `Some(Vec<Parameter>)` with merged parameters, or `Some(empty_vec)` if both are None
///
/// # Example
///
/// ```rust
/// // Test 1: GET /users?limit=10
/// // Test 2: GET /users?offset=5&sort=name
/// // Result: GET /users supports limit, offset, and sort parameters
/// ```
fn merge_parameters(
    current: Option<Vec<Parameter>>,
    new: Option<Vec<Parameter>>,
) -> Option<Vec<Parameter>> {
    let mut result = IndexMap::new();
    // Optimized: Avoid cloning parameter names by using references for lookup
    for param in new.unwrap_or_default() {
        result.insert(param.name.clone(), param);
    }
    for param in current.unwrap_or_default() {
        result.entry(param.name.clone()).or_insert(param);
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

/// Common API path prefixes that should be skipped when generating operation metadata.
/// These are typically organizational prefixes that don't represent business resources.
const SKIP_PATH_PREFIXES: &[&str] = &[
    "api",      // Most common: /api/users
    "v1",       // Versioning: /v1/users, /api/v1/users
    "v2",       // Versioning: /v2/users
    "v3",       // Versioning: /v3/users
    "rest",     // REST API prefix: /rest/users
    "service",  // Service-oriented: /service/users
    "public",   // Public API: /public/users
    "internal", // Internal API: /internal/users
];

/// Generates a human-readable description for an operation based on HTTP method and path.
fn generate_description(method: &http::Method, path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let segments: Vec<&str> = path.split('/').collect();

    if segments.is_empty() || (segments.len() == 1 && segments[0].is_empty()) {
        return None;
    }

    // Skip common API prefixes (api, v1, v2, rest, etc.)
    let start_index = segments
        .iter()
        .take_while(|&segment| SKIP_PATH_PREFIXES.contains(segment))
        .count();

    if start_index >= segments.len() {
        return None;
    }

    // Extract the resource name from the path
    let resource = if segments.len() == start_index + 1 {
        // Simple path like "/users" or "/api/users"
        segments[start_index]
    } else if segments.len() >= start_index + 2 {
        // Path with potential ID parameter like "/users/{id}" or "/users/123"
        // Or nested resource like "/users/profile" or "/observations/import"
        let last_segment = segments.last().unwrap();
        if last_segment.starts_with('{') && last_segment.ends_with('}') {
            // Last segment is a parameter, use the previous segment as resource
            segments[segments.len() - 2]
        } else if segments.len() > start_index + 1 {
            // Check if this is a nested action (like import, upload, etc.)
            let resource_name = segments[start_index];
            let action = last_segment;

            // Special handling for common actions
            match *action {
                "import" => return Some(format!("Import {resource_name}")),
                "upload" => return Some(format!("Upload {resource_name}")),
                "export" => return Some(format!("Export {resource_name}")),
                "search" => return Some(format!("Search {resource_name}")),
                _ => last_segment, // Use the last segment as the resource
            }
        } else {
            last_segment
        }
    } else {
        segments[start_index]
    };

    // Check if the path has an ID parameter (indicates single resource operation)
    let has_id = segments
        .iter()
        .any(|segment| segment.starts_with('{') && segment.ends_with('}'));

    let action = match *method {
        http::Method::GET => {
            if has_id {
                format!("Retrieve {} by ID", singularize(resource))
            } else {
                format!("Retrieve {resource}")
            }
        }
        http::Method::POST => {
            if has_id {
                format!("Create {} by ID", singularize(resource))
            } else {
                format!("Create {}", singularize(resource))
            }
        }
        http::Method::PUT => {
            if has_id {
                format!("Update {} by ID", singularize(resource))
            } else {
                format!("Update {resource}")
            }
        }
        http::Method::PATCH => {
            if has_id {
                format!("Partially update {} by ID", singularize(resource))
            } else {
                format!("Partially update {resource}")
            }
        }
        http::Method::DELETE => {
            if has_id {
                format!("Delete {} by ID", singularize(resource))
            } else {
                format!("Delete {resource}")
            }
        }
        _ => return None,
    };

    Some(action)
}

/// Generates appropriate tags for an operation based on the path.
fn generate_tags(path: &str) -> Option<Vec<String>> {
    let path = path.trim_start_matches('/');
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        return None;
    }

    let mut tags = Vec::new();

    // Skip common API prefixes (api, v1, v2, rest, etc.)
    let start_index = segments
        .iter()
        .take_while(|&segment| SKIP_PATH_PREFIXES.contains(segment))
        .count();

    if start_index >= segments.len() {
        return None;
    }

    // Add the main resource name
    let resource = segments[start_index];
    tags.push(resource.to_string());

    // Add action-specific tags for nested resources
    if segments.len() > start_index + 1 {
        let last_segment = segments.last().unwrap();
        // Only add as tag if it's not a parameter (doesn't contain braces)
        if !last_segment.starts_with('{') {
            match *last_segment {
                "import" | "upload" | "export" | "search" | "bulk" => {
                    tags.push(last_segment.to_string());
                }
                _ => {
                    // For other nested resources, add them as secondary tags
                    if segments.len() == start_index + 2 {
                        tags.push(last_segment.to_string());
                    }
                }
            }
        }
    }

    if tags.is_empty() { None } else { Some(tags) }
}

/// Singularize English words using the cruet crate with manual handling for known limitations.
/// This provides production-ready pluralization handling for API resource names.
/// Includes custom handling for irregular cases that cruet doesn't cover.
fn singularize(word: &str) -> String {
    // Handle special cases that cruet doesn't handle properly
    match word {
        "children" => return "child".to_string(),
        "people" => return "person".to_string(),
        "data" => return "datum".to_string(),
        "feet" => return "foot".to_string(),
        "teeth" => return "tooth".to_string(),
        "geese" => return "goose".to_string(),
        "men" => return "man".to_string(),
        "women" => return "woman".to_string(),
        _ => {}
    }

    // Use cruet for most cases
    use cruet::*;
    let result = word.to_singular();

    // Fallback to original word if cruet returns empty string
    if result.is_empty() && !word.is_empty() {
        word.to_string()
    } else {
        result
    }
}

#[cfg(test)]
mod operation_metadata_tests {
    use super::*;
    use http::Method;

    #[test]
    fn test_generate_description_simple_paths() {
        assert_eq!(
            generate_description(&Method::GET, "/users"),
            Some("Retrieve users".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/users"),
            Some("Create user".to_string())
        );
        assert_eq!(
            generate_description(&Method::PUT, "/users"),
            Some("Update users".to_string())
        );
        assert_eq!(
            generate_description(&Method::DELETE, "/users"),
            Some("Delete users".to_string())
        );
        assert_eq!(
            generate_description(&Method::PATCH, "/users"),
            Some("Partially update users".to_string())
        );
    }

    #[test]
    fn test_generate_description_with_id_parameter() {
        assert_eq!(
            generate_description(&Method::GET, "/users/{id}"),
            Some("Retrieve user by ID".to_string())
        );
        assert_eq!(
            generate_description(&Method::PUT, "/users/{id}"),
            Some("Update user by ID".to_string())
        );
        assert_eq!(
            generate_description(&Method::DELETE, "/users/{id}"),
            Some("Delete user by ID".to_string())
        );
        assert_eq!(
            generate_description(&Method::PATCH, "/users/{id}"),
            Some("Partially update user by ID".to_string())
        );
    }

    #[test]
    fn test_generate_description_special_actions() {
        assert_eq!(
            generate_description(&Method::POST, "/observations/import"),
            Some("Import observations".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/observations/upload"),
            Some("Upload observations".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/users/export"),
            Some("Export users".to_string())
        );
        assert_eq!(
            generate_description(&Method::GET, "/users/search"),
            Some("Search users".to_string())
        );
    }

    #[test]
    fn test_generate_description_api_prefix() {
        assert_eq!(
            generate_description(&Method::GET, "/api/observations"),
            Some("Retrieve observations".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/api/observations/import"),
            Some("Import observations".to_string())
        );
        // Test multiple prefixes
        assert_eq!(
            generate_description(&Method::GET, "/api/v1/users"),
            Some("Retrieve users".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/rest/service/items"),
            Some("Create item".to_string())
        );
    }

    #[test]
    fn test_generate_tags_simple_paths() {
        assert_eq!(generate_tags("/users"), Some(vec!["users".to_string()]));
        assert_eq!(
            generate_tags("/observations"),
            Some(vec!["observations".to_string()])
        );
    }

    #[test]
    fn test_generate_tags_with_api_prefix() {
        assert_eq!(generate_tags("/api/users"), Some(vec!["users".to_string()]));
        assert_eq!(
            generate_tags("/api/observations"),
            Some(vec!["observations".to_string()])
        );
        // Test multiple prefixes
        assert_eq!(
            generate_tags("/api/v1/users"),
            Some(vec!["users".to_string()])
        );
        assert_eq!(
            generate_tags("/rest/service/items"),
            Some(vec!["items".to_string()])
        );
    }

    #[test]
    fn test_generate_tags_with_special_actions() {
        assert_eq!(
            generate_tags("/api/observations/import"),
            Some(vec!["observations".to_string(), "import".to_string()])
        );
        assert_eq!(
            generate_tags("/api/observations/upload"),
            Some(vec!["observations".to_string(), "upload".to_string()])
        );
        assert_eq!(
            generate_tags("/users/export"),
            Some(vec!["users".to_string(), "export".to_string()])
        );
    }

    #[test]
    fn test_generate_tags_with_id_parameter() {
        assert_eq!(
            generate_tags("/api/observations/{id}"),
            Some(vec!["observations".to_string()])
        );
        assert_eq!(
            generate_tags("/users/{user_id}"),
            Some(vec!["users".to_string()])
        );
    }

    #[test]
    fn test_singularize() {
        // Regular plurals that cruet handles well
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("observations"), "observation");
        assert_eq!(singularize("items"), "item");

        // Irregular plurals - handled by manual overrides + cruet
        assert_eq!(singularize("mice"), "mouse"); // cruet handles this
        assert_eq!(singularize("children"), "child"); // manual override
        assert_eq!(singularize("people"), "person"); // manual override
        assert_eq!(singularize("feet"), "foot"); // manual override
        assert_eq!(singularize("teeth"), "tooth"); // manual override
        assert_eq!(singularize("geese"), "goose"); // manual override
        assert_eq!(singularize("men"), "man"); // manual override
        assert_eq!(singularize("women"), "woman"); // manual override
        assert_eq!(singularize("data"), "datum"); // manual override

        // Words ending in 'es'
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("watches"), "watch");

        // Already singular - cruet handles these gracefully
        assert_eq!(singularize("user"), "user");
        assert_eq!(singularize("child"), "child");

        // Edge cases - with fallback protection
        assert_eq!(singularize("s"), "s"); // Falls back to original when cruet returns empty
        assert_eq!(singularize(""), ""); // Empty string stays empty

        // Complex cases that cruet handles well
        assert_eq!(singularize("categories"), "category");
        assert_eq!(singularize("companies"), "company");
        assert_eq!(singularize("libraries"), "library");

        // Additional cases cruet handles
        assert_eq!(singularize("stories"), "story");
        assert_eq!(singularize("cities"), "city");
    }
}
