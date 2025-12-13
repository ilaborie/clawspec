use std::any::{TypeId, type_name};

use headers::{ContentType, Header};
use http::StatusCode;
use http::header::CONTENT_TYPE;
use reqwest::Response;
use serde::de::DeserializeOwned;
use utoipa::ToSchema;
use utoipa::openapi::{RefOr, Schema};

use super::channel::{CollectorMessage, CollectorSender};
use super::schema::{SchemaEntry, compute_schema_ref};
use crate::client::ApiClientError;
use crate::client::response::output::Output;

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
/// - **Optional JSON responses** (204/404 → None): [`as_optional_json::<T>()`](Self::as_optional_json)
/// - **Type-safe error handling**: [`as_result_json::<T, E>()`](Self::as_result_json) (2xx → Ok(T), 4xx/5xx → Err(E))
/// - **Optional with errors**: [`as_result_option_json::<T, E>()`](Self::as_result_option_json) (combines optional and error handling)
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
///
///     .await?
///     .as_json()  // ← This is required!
///     .await?;
///
/// // ✅ CORRECT: For empty responses (like DELETE)
/// client
///     .delete("/users/123")?
///
///     .await?
///     .as_empty()  // ← This is required!
///     .await?;
///
/// // ❌ INCORRECT: This will not generate proper OpenAPI documentation
/// // let _result = client.get("/users/123")?.await?;
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
    pub(in crate::client) collector_sender: CollectorSender,
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
///
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
    /// Returns the HTTP status code of the response.
    pub(in crate::client) fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the content type of the response, if present.
    pub(in crate::client) fn content_type(&self) -> Option<&ContentType> {
        self.content_type.as_ref()
    }

    /// Returns the operation ID for this result.
    pub(in crate::client) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    /// Returns a reference to the output.
    pub(in crate::client) fn output(&self) -> &Output {
        &self.output
    }

    /// Extracts and parses the Content-Type header from the HTTP response.
    fn extract_content_type(response: &Response) -> Result<Option<ContentType>, ApiClientError> {
        let content_type = response
            .headers()
            .get_all(CONTENT_TYPE)
            .iter()
            .collect::<Vec<_>>();

        if content_type.is_empty() {
            Ok(None)
        } else {
            let ct = ContentType::decode(&mut content_type.into_iter())?;
            Ok(Some(ct))
        }
    }

    /// Processes the response body based on content type and status code.
    async fn process_response_body(
        response: Response,
        content_type: &Option<ContentType>,
        status: StatusCode,
    ) -> Result<Output, ApiClientError> {
        if let Some(content_type) = content_type
            && status != StatusCode::NO_CONTENT
        {
            if *content_type == ContentType::json() {
                let json = response.text().await?;
                Ok(Output::Json(json))
            } else if *content_type == ContentType::octet_stream() {
                let bytes = response.bytes().await?;
                Ok(Output::Bytes(bytes.to_vec()))
            } else if content_type.to_string().starts_with("text/") {
                let text = response.text().await?;
                Ok(Output::Text(text))
            } else {
                let body = response.text().await?;
                Ok(Output::Other { body })
            }
        } else {
            Ok(Output::Empty)
        }
    }

    pub(in crate::client) async fn new(
        operation_id: String,
        collector_sender: CollectorSender,
        response: Response,
    ) -> Result<Self, ApiClientError> {
        let status = response.status();
        let content_type = Self::extract_content_type(&response)?;
        let output = Self::process_response_body(response, &content_type, status).await?;

        Ok(Self {
            operation_id,
            status,
            content_type,
            output,
            collector_sender,
        })
    }

    pub(in crate::client) async fn new_without_collection(
        response: Response,
    ) -> Result<Self, ApiClientError> {
        let status = response.status();
        let content_type = Self::extract_content_type(&response)?;
        let output = Self::process_response_body(response, &content_type, status).await?;

        Ok(Self {
            operation_id: String::new(), // Empty operation_id since it won't be used
            status,
            content_type,
            output,
            collector_sender: CollectorSender::dummy(),
        })
    }

    pub(in crate::client) async fn get_output(
        &self,
        schema: Option<RefOr<Schema>>,
    ) -> Result<&Output, ApiClientError> {
        // Skip if operation_id is empty (skip_collection case)
        if self.operation_id.is_empty() {
            return Ok(&self.output);
        }

        // Send message to register the response
        let status_code = self.status.as_u16();
        let description = format!("Status code {status_code}");

        self.collector_sender
            .send(CollectorMessage::RegisterResponse {
                operation_id: self.operation_id.clone(),
                status: self.status,
                content_type: self.content_type.clone(),
                schema,
                description,
            })
            .await;

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
    ///
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
        // Compute schema reference locally (no lock needed)
        let schema = compute_schema_ref::<T>();

        // Register the schema entry via channel
        let entry = SchemaEntry::of::<T>();
        self.collector_sender
            .send(CollectorMessage::AddSchemaEntry(entry))
            .await;

        let output = self.get_output(Some(schema)).await?;

        let Output::Json(json) = output else {
            return Err(ApiClientError::UnsupportedJsonOutput {
                output: output.clone(),
                name: type_name::<T>(),
            });
        };

        self.deserialize_and_record::<T>(json).await
    }

    /// Processes the response as optional JSON, treating 204 and 404 status codes as `None`.
    ///
    /// This method provides ergonomic handling of optional REST API responses by automatically
    /// treating 204 (No Content) and 404 (Not Found) status codes as `None`, while deserializing
    /// other successful responses as `Some(T)`. This is particularly useful for APIs that use
    /// HTTP status codes to indicate the absence of data rather than errors.
    ///
    /// The method automatically records the response schema in the OpenAPI specification,
    /// maintaining proper documentation generation.
    ///
    /// # Type Parameters
    ///
    /// - `T`: The target type for deserialization, must implement `DeserializeOwned`, `ToSchema`, and `'static`
    ///
    /// # Returns
    ///
    /// - `Ok(None)`: If the status code is 204 or 404
    /// - `Ok(Some(T))`: The deserialized response object for other successful responses
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
    ///
    /// // Returns None for 404
    /// let user: Option<User> = client
    ///     .get("/users/nonexistent")?
    ///
    ///     .await?
    ///     .as_optional_json()
    ///     .await?;
    /// assert!(user.is_none());
    ///
    /// // Returns Some(User) for successful response
    /// let user: Option<User> = client
    ///     .get("/users/123")?
    ///
    ///     .await?
    ///     .as_optional_json()
    ///     .await?;
    /// assert!(user.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_optional_json<T>(&mut self) -> Result<Option<T>, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
    {
        // Check if status code indicates absence of data
        if self.status == StatusCode::NO_CONTENT || self.status == StatusCode::NOT_FOUND {
            // Record the response without a schema
            self.get_output(None).await?;
            return Ok(None);
        }

        // Compute schema reference locally (no lock needed)
        let schema = compute_schema_ref::<T>();

        // Register the schema entry via channel
        let entry = SchemaEntry::of::<T>();
        self.collector_sender
            .send(CollectorMessage::AddSchemaEntry(entry))
            .await;

        let output = self.get_output(Some(schema)).await?;

        let Output::Json(json) = output else {
            return Err(ApiClientError::UnsupportedJsonOutput {
                output: output.clone(),
                name: type_name::<T>(),
            });
        };

        let result = self.deserialize_and_record::<T>(json).await?;
        Ok(Some(result))
    }

    /// Processes the response as a `Result<T, E>` based on HTTP status code.
    ///
    /// This method provides type-safe error handling for REST APIs that return structured
    /// error responses. It automatically deserializes the response body to either the
    /// success type `T` (for 2xx status codes) or the error type `E` (for 4xx/5xx status codes).
    ///
    /// Both success and error schemas are automatically recorded in the OpenAPI specification,
    /// providing complete documentation of your API's response patterns.
    ///
    /// # Type Parameters
    ///
    /// - `T`: The success response type, must implement `DeserializeOwned`, `ToSchema`, and `'static`
    /// - `E`: The error response type, must implement `DeserializeOwned`, `ToSchema`, and `'static`
    ///
    /// # Returns
    ///
    /// - `Ok(T)`: The deserialized success response for 2xx status codes
    /// - `Err(E)`: The deserialized error response for 4xx/5xx status codes
    ///
    /// # Errors
    ///
    /// Returns `ApiClientError` if:
    /// - The response is not JSON
    /// - JSON deserialization fails for either type
    /// - The response body is empty when content is expected
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
    /// #[derive(Deserialize, ToSchema)]
    /// struct ApiError {
    ///     code: String,
    ///     message: String,
    /// }
    ///
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Returns Ok(User) for 2xx responses
    /// let result: Result<User, ApiError> = client
    ///     .get("/users/123")?
    ///
    ///     .await?
    ///     .as_result_json()
    ///     .await?;
    ///
    /// match result {
    ///     Ok(user) => println!("User: {}", user.name),
    ///     Err(err) => println!("Error: {} - {}", err.code, err.message),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_result_json<T, E>(&mut self) -> Result<Result<T, E>, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
        E: DeserializeOwned + ToSchema + 'static,
    {
        Ok(self
            .process_result_json_internal::<T, E>(false)
            .await?
            .map(|opt| opt.expect("BUG: 404 handling disabled but got None")))
    }

    /// Processes the response as a `Result<Option<T>, E>` based on HTTP status code.
    ///
    /// This method combines optional response handling with type-safe error handling,
    /// providing comprehensive support for REST APIs that:
    /// - Return structured error responses for failures (4xx/5xx)
    /// - Use 204 (No Content) or 404 (Not Found) to indicate absence of data
    /// - Return data for other successful responses (2xx)
    ///
    /// Both success and error schemas are automatically recorded in the OpenAPI specification.
    ///
    /// # Type Parameters
    ///
    /// - `T`: The success response type, must implement `DeserializeOwned`, `ToSchema`, and `'static`
    /// - `E`: The error response type, must implement `DeserializeOwned`, `ToSchema`, and `'static`
    ///
    /// # Returns
    ///
    /// - `Ok(None)`: For 204 (No Content) or 404 (Not Found) status codes
    /// - `Ok(Some(T))`: The deserialized success response for other 2xx status codes
    /// - `Err(E)`: The deserialized error response for 4xx/5xx status codes
    ///
    /// # Errors
    ///
    /// Returns `ApiClientError` if:
    /// - The response is not JSON (when content is expected)
    /// - JSON deserialization fails for either type
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
    /// #[derive(Deserialize, ToSchema)]
    /// struct ApiError {
    ///     code: String,
    ///     message: String,
    /// }
    ///
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Returns Ok(None) for 404
    /// let result: Result<Option<User>, ApiError> = client
    ///     .get("/users/nonexistent")?
    ///
    ///     .await?
    ///     .as_result_option_json()
    ///     .await?;
    ///
    /// match result {
    ///     Ok(Some(user)) => println!("User: {}", user.name),
    ///     Ok(None) => println!("User not found"),
    ///     Err(err) => println!("Error: {} - {}", err.code, err.message),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn as_result_option_json<T, E>(
        &mut self,
    ) -> Result<Result<Option<T>, E>, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
        E: DeserializeOwned + ToSchema + 'static,
    {
        self.process_result_json_internal::<T, E>(true).await
    }

    /// Internal helper for processing Result<Option<T>, E> responses.
    ///
    /// Handles the common logic for both `as_result_json` and `as_result_option_json`.
    async fn process_result_json_internal<T, E>(
        &mut self,
        treat_404_as_none: bool,
    ) -> Result<Result<Option<T>, E>, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
        E: DeserializeOwned + ToSchema + 'static,
    {
        // Check for 204/404 which indicate absence of data (when enabled)
        if treat_404_as_none
            && (self.status == StatusCode::NO_CONTENT || self.status == StatusCode::NOT_FOUND)
        {
            // Register both schema entries via channel
            let success_entry = SchemaEntry::of::<T>();
            let error_entry = SchemaEntry::of::<E>();
            self.collector_sender
                .send(CollectorMessage::AddSchemaEntry(success_entry))
                .await;
            self.collector_sender
                .send(CollectorMessage::AddSchemaEntry(error_entry))
                .await;

            self.get_output(None).await?;
            return Ok(Ok(None));
        }

        let is_success = self.status.is_success();

        // Compute schema references locally (no lock needed)
        let success_schema = compute_schema_ref::<T>();
        let error_schema = compute_schema_ref::<E>();

        // Register both schema entries via channel
        let success_entry = SchemaEntry::of::<T>();
        let error_entry = SchemaEntry::of::<E>();
        self.collector_sender
            .send(CollectorMessage::AddSchemaEntry(success_entry))
            .await;
        self.collector_sender
            .send(CollectorMessage::AddSchemaEntry(error_entry))
            .await;

        // Get the appropriate schema based on status code
        let schema = if is_success {
            success_schema
        } else {
            error_schema
        };

        let output = self.get_output(Some(schema)).await?;

        let Output::Json(json) = output else {
            return Err(ApiClientError::UnsupportedJsonOutput {
                output: output.clone(),
                name: if is_success {
                    type_name::<T>()
                } else {
                    type_name::<E>()
                },
            });
        };

        if is_success {
            let value = self.deserialize_and_record::<T>(json).await?;
            Ok(Ok(Some(value)))
        } else {
            let error = self.deserialize_and_record::<E>(json).await?;
            Ok(Err(error))
        }
    }

    /// Helper to deserialize JSON and record examples.
    async fn deserialize_and_record<T>(&self, json: &str) -> Result<T, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
    {
        let deserializer = &mut serde_json::Deserializer::from_str(json);
        let result: T = serde_path_to_error::deserialize(deserializer).map_err(|err| {
            ApiClientError::JsonError {
                path: err.path().to_string(),
                error: err.into_inner(),
                body: json.to_string(),
            }
        })?;

        if let Ok(example) = serde_json::to_value(json) {
            self.collector_sender
                .send(CollectorMessage::AddExample {
                    type_id: TypeId::of::<T>(),
                    type_name: type_name::<T>(),
                    example,
                })
                .await;
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
    ///
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
    ///
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
    ///
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
    ///
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
