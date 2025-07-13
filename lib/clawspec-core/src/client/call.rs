use std::ops::{Range, RangeInclusive};
use std::sync::Arc;

use headers::HeaderMapExt;
use http::header::{HeaderName, HeaderValue};
use http::{Method, Uri};
use reqwest::{Body, Request};
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::debug;
use url::Url;
use utoipa::ToSchema;

use super::collectors::{CalledOperation, Collectors};
use super::param::ParameterValue;
use super::path::PathResolved;
use super::status::ExpectedStatusCodes;
use super::{ApiClientError, CallBody, CallHeaders, CallPath, CallQuery, CallResult, ParamValue};

const BODY_MAX_LENGTH: usize = 1024;

/// Metadata for OpenAPI operation documentation.
#[derive(Debug, Clone, Default)]
struct OperationMetadata {
    /// Operation ID for the OpenAPI operation
    operation_id: String,
    /// Operation tags for categorization
    tags: Option<Vec<String>>,
    /// Operation description for documentation
    description: Option<String>,
}

/// Builder for configuring HTTP API calls with comprehensive parameter and validation support.
///
/// `ApiCall` provides a fluent interface for building HTTP requests with automatic OpenAPI schema collection.
/// It supports query parameters, headers, request bodies, and flexible status code validation.
///
/// # Method Groups
///
/// ## Request Body Methods
/// - [`json(data)`](Self::json) - Set JSON request body
/// - [`form(data)`](Self::form) - Set form-encoded request body  
/// - [`multipart(form)`](Self::multipart) - Set multipart form request body
/// - [`text(content)`](Self::text) - Set plain text request body
/// - [`raw(bytes)`](Self::raw) - Set raw binary request body
///
/// ## Parameter Methods  
/// - [`with_query(query)`](Self::with_query) - Set query parameters
/// - [`with_headers(headers)`](Self::with_headers) - Set request headers
/// - [`with_header(name, value)`](Self::with_header) - Add single header
///
/// ## Status Code Validation
/// - [`with_expected_status_codes(codes)`](Self::with_expected_status_codes) - Set expected status codes
/// - [`with_status_range_inclusive(range)`](Self::with_status_range_inclusive) - Set inclusive range (200..=299)
/// - [`with_status_range(range)`](Self::with_status_range) - Set exclusive range (200..300)
/// - [`add_expected_status(code)`](Self::add_expected_status) - Add single expected status
/// - [`add_expected_status_range_inclusive(range)`](Self::add_expected_status_range_inclusive) - Add inclusive range
/// - [`add_expected_status_range(range)`](Self::add_expected_status_range) - Add exclusive range
/// - [`with_client_errors()`](Self::with_client_errors) - Accept 2xx and 4xx codes
///
/// ## OpenAPI Metadata
/// - [`with_operation_id(id)`](Self::with_operation_id) - Set operation ID
/// - [`with_tags(tags)`](Self::with_tags) - Set operation tags
/// - [`with_description(desc)`](Self::with_description) - Set operation description
///
/// ## Execution
/// - [`exchange()`](Self::exchange) - Execute the request and return response
///
/// # Default Behavior
///
/// - **Status codes**: Accepts 200-499 (inclusive of 200, exclusive of 500)
/// - **Content-Type**: Automatically set based on body type
/// - **Schema collection**: Request/response schemas are automatically captured
#[derive(derive_more::Debug)]
pub struct ApiCall {
    client: reqwest::Client,
    base_uri: Uri,
    collectors: Arc<RwLock<Collectors>>,

    method: Method,
    path: CallPath,
    query: CallQuery,
    headers: Option<CallHeaders>,

    #[debug(ignore)]
    body: Option<CallBody>,
    // TODO auth - https://github.com/ilaborie/clawspec/issues/17
    // TODO cookiess - https://github.com/ilaborie/clawspec/issues/18
    /// Expected status codes for this request (default: 200..500)
    expected_status_codes: ExpectedStatusCodes,
    /// Operation metadata for OpenAPI documentation
    metadata: OperationMetadata,
}

impl ApiCall {
    pub(super) fn build(
        client: reqwest::Client,
        base_uri: Uri,
        collectors: Arc<RwLock<Collectors>>,
        method: Method,
        path: CallPath,
    ) -> Result<Self, ApiClientError> {
        let operation_id = slug::slugify(format!("{method} {}", path.path));

        let result = Self {
            client,
            base_uri,
            collectors,
            method,
            path,
            query: CallQuery::default(),
            headers: None,
            body: None,
            expected_status_codes: ExpectedStatusCodes::default(),
            metadata: OperationMetadata {
                operation_id,
                tags: None,
                description: None,
            },
        };
        Ok(result)
    }
}

// Builder Implementation
// Methods are organized by functionality for better discoverability:
// 1. OpenAPI Metadata (operation_id, description, tags)
// 2. Request Configuration (query, headers)
// 3. Status Code Validation
// 4. Request Body Methods
impl ApiCall {
    // =============================================================================
    // OpenAPI Metadata Methods
    // =============================================================================
    pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.metadata.operation_id = operation_id.into();
        self
    }

    /// Sets the operation description for OpenAPI documentation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/users")?.with_description("Retrieve all users");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.metadata.description = Some(description.into());
        self
    }

    /// Sets the operation tags for OpenAPI categorization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/users")?.with_tags(vec!["users", "admin"]);
    /// // Also works with arrays, slices, or any IntoIterator
    /// let call = client.get("/users")?.with_tags(["users", "admin"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.metadata.tags = Some(tags.into_iter().map(|t| t.into()).collect());
        self
    }

    /// Adds a single tag to the operation for OpenAPI categorization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/users")?.with_tag("users").with_tag("admin");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.metadata
            .tags
            .get_or_insert_with(Vec::new)
            .push(tag.into());
        self
    }

    // =============================================================================
    // Request Configuration Methods
    // =============================================================================

    pub fn with_query(mut self, query: CallQuery) -> Self {
        self.query = query;
        self
    }

    pub fn with_headers_option(mut self, headers: Option<CallHeaders>) -> Self {
        self.headers = match (self.headers.take(), headers) {
            (Some(existing), Some(new)) => Some(existing.merge(new)),
            (existing, new) => existing.or(new),
        };
        self
    }

    /// Adds headers to the API call, merging with any existing headers.
    ///
    /// This is a convenience method that automatically wraps the headers in Some().
    pub fn with_headers(self, headers: CallHeaders) -> Self {
        self.with_headers_option(Some(headers))
    }

    /// Convenience method to add a single header.
    ///
    /// This method automatically handles type conversion and merges with existing headers.
    /// If a header with the same name already exists, the new value will override it.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/users")?
    ///     .with_header("Authorization", "Bearer token123")
    ///     .with_header("X-Request-ID", "abc-123-def");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Type Flexibility and Edge Cases
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Different value types are automatically converted
    /// let call = client.post("/api/data")?
    ///     .with_header("Content-Length", 1024_u64)           // Numeric values
    ///     .with_header("X-Retry-Count", 3_u32)               // Different numeric types
    ///     .with_header("X-Debug", true)                      // Boolean values
    ///     .with_header("X-Session-ID", "session-123");       // String values
    ///
    /// // Headers can be chained and overridden
    /// let call = client.get("/protected")?
    ///     .with_header("Authorization", "Bearer old-token")
    ///     .with_header("Authorization", "Bearer new-token");  // Overrides previous value
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_header<T: ParameterValue>(
        self,
        name: impl Into<String>,
        value: impl Into<ParamValue<T>>,
    ) -> Self {
        let headers = CallHeaders::new().add_header(name, value);
        self.with_headers(headers)
    }

    // =============================================================================
    // Status Code Validation Methods
    // =============================================================================

    /// Sets the expected status codes for this request using an inclusive range.
    ///
    /// By default, status codes 200..500 are considered successful.
    /// Use this method to customize which status codes should be accepted.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept only 200 to 201 (inclusive)
    /// let call = client.post("/users")?.with_status_range_inclusive(200..=201);
    ///
    /// // Accept any 2xx status code
    /// let call = client.get("/users")?.with_status_range_inclusive(200..=299);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Edge Cases
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Single status code range (equivalent to with_expected_status)
    /// let call = client.get("/health")?.with_status_range_inclusive(200..=200);
    ///
    /// // Accept both success and client error ranges  
    /// let call = client.delete("/users/123")?
    ///     .with_status_range_inclusive(200..=299)
    ///     .add_expected_status_range_inclusive(400..=404);
    ///
    /// // Handle APIs that return 2xx or 3xx for different success states
    /// let call = client.post("/async-operation")?.with_status_range_inclusive(200..=302);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_status_range_inclusive(mut self, range: RangeInclusive<u16>) -> Self {
        self.expected_status_codes = ExpectedStatusCodes::from_inclusive_range(range);
        self
    }

    /// Sets the expected status codes for this request using an exclusive range.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200 to 299 (200 included, 300 excluded)
    /// let call = client.get("/users")?.with_status_range(200..300);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_status_range(mut self, range: Range<u16>) -> Self {
        self.expected_status_codes = ExpectedStatusCodes::from_exclusive_range(range);
        self
    }

    /// Sets a single expected status code for this request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept only 204 for DELETE operations
    /// let call = client.delete("/users/123")?.with_expected_status(204);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_expected_status(mut self, status: u16) -> Self {
        self.expected_status_codes = ExpectedStatusCodes::from_single(status);
        self
    }

    /// Adds an additional expected status code to the existing set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200..299 and also 404
    /// let call = client.get("/users")?.with_status_range_inclusive(200..=299).add_expected_status(404);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_expected_status(mut self, status: u16) -> Self {
        self.expected_status_codes = self.expected_status_codes.add_expected_status(status);
        self
    }

    /// Adds an additional expected status range (inclusive) to the existing set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200..=204 and also 400..=402
    /// let call = client.post("/users")?.with_status_range_inclusive(200..=204).add_expected_status_range_inclusive(400..=402);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_expected_status_range_inclusive(mut self, range: RangeInclusive<u16>) -> Self {
        self.expected_status_codes = self.expected_status_codes.add_expected_range(range);
        self
    }

    /// Adds an additional expected status range (exclusive) to the existing set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200..=204 and also 400..403
    /// let call = client.post("/users")?.with_status_range_inclusive(200..=204).add_expected_status_range(400..403);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_expected_status_range(mut self, range: Range<u16>) -> Self {
        self.expected_status_codes = self.expected_status_codes.add_exclusive_range(range);
        self
    }

    /// Convenience method to accept only 2xx status codes (200..300).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/users")?.with_success_only();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_success_only(self) -> Self {
        self.with_status_range(200..300)
    }

    /// Convenience method to accept 2xx and 4xx status codes (200..500, excluding 3xx).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.post("/users")?.with_client_errors();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_client_errors(self) -> Self {
        self.with_status_range_inclusive(200..=299)
            .add_expected_status_range_inclusive(400..=499)
    }

    /// Sets the expected status codes using an `ExpectedStatusCodes` instance.
    ///
    /// This method allows you to pass pre-configured `ExpectedStatusCodes` instances,
    /// which is particularly useful with the `expected_status_codes!` macro.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::{ApiClient, expected_status_codes};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Using the macro with with_expected_status_codes
    /// let call = client.get("/users")?
    ///     .with_expected_status_codes(expected_status_codes!(200-299));
    ///
    /// // Using manually created ExpectedStatusCodes
    /// let codes = clawspec_core::ExpectedStatusCodes::from_inclusive_range(200..=204)
    ///     .add_expected_status(404);
    /// let call = client.get("/items")?.with_expected_status_codes(codes);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_expected_status_codes(mut self, codes: ExpectedStatusCodes) -> Self {
        self.expected_status_codes = codes;
        self
    }

    /// Sets expected status codes from a single `http::StatusCode`.
    ///
    /// This method provides **compile-time validation** of status codes through the type system.
    /// Unlike the `u16` variants, this method does not perform runtime validation since
    /// `http::StatusCode` guarantees valid HTTP status codes at compile time.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// use http::StatusCode;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// let call = client.get("/users")?
    ///     .with_expected_status_code(StatusCode::OK);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_expected_status_code(self, status: http::StatusCode) -> Self {
        self.with_expected_status_codes(ExpectedStatusCodes::from_status_code(status))
    }

    /// Sets expected status codes from a range of `http::StatusCode`.
    ///
    /// This method provides **compile-time validation** of status codes through the type system.
    /// Unlike the `u16` variants, this method does not perform runtime validation since
    /// `http::StatusCode` guarantees valid HTTP status codes at compile time.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// use http::StatusCode;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// let call = client.get("/users")?
    ///     .with_expected_status_code_range(StatusCode::OK..=StatusCode::NO_CONTENT);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_expected_status_code_range(self, range: RangeInclusive<http::StatusCode>) -> Self {
        self.with_expected_status_codes(ExpectedStatusCodes::from_status_code_range_inclusive(
            range,
        ))
    }

    // =============================================================================
    // Request Body Methods
    // =============================================================================

    /// Sets the request body to JSON.
    ///
    /// This method serializes the provided data as JSON and sets the
    /// Content-Type header to `application/json`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #[derive(Serialize, ToSchema)]
    /// struct CreateUser {
    ///     name: String,
    ///     email: String,
    /// }
    ///
    /// let mut client = ApiClient::builder().build()?;
    /// let user_data = CreateUser {
    ///     name: "John Doe".to_string(),
    ///     email: "john@example.com".to_string(),
    /// };
    ///
    /// let call = client.post("/users")?.json(&user_data)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn json<T>(mut self, t: &T) -> Result<Self, ApiClientError>
    where
        T: Serialize + ToSchema + 'static,
    {
        let body = CallBody::json(t)?;
        self.body = Some(body);
        Ok(self)
    }

    /// Sets the request body to form-encoded data.
    ///
    /// This method serializes the provided data as `application/x-www-form-urlencoded`
    /// and sets the appropriate Content-Type header.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # use serde::Serialize;
    /// # use utoipa::ToSchema;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #[derive(Serialize, ToSchema)]
    /// struct LoginForm {
    ///     username: String,
    ///     password: String,
    /// }
    ///
    /// let mut client = ApiClient::builder().build()?;
    /// let form_data = LoginForm {
    ///     username: "user@example.com".to_string(),
    ///     password: "secret".to_string(),
    /// };
    ///
    /// let call = client.post("/login")?.form(&form_data)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn form<T>(mut self, t: &T) -> Result<Self, ApiClientError>
    where
        T: Serialize + ToSchema + 'static,
    {
        let body = CallBody::form(t)?;
        self.body = Some(body);
        Ok(self)
    }

    /// Sets the request body to raw binary data with a custom content type.
    ///
    /// This method allows you to send arbitrary binary data with a specified
    /// content type. This is useful for sending data that doesn't fit into
    /// the standard JSON or form categories.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # use headers::ContentType;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// // Send XML data
    /// let xml_data = r#"<?xml version="1.0"?><user><name>John</name></user>"#;
    /// let call = client.post("/import")?
    ///     .raw(xml_data.as_bytes().to_vec(), ContentType::xml());
    ///
    /// // Send binary file
    /// let binary_data = vec![0xFF, 0xFE, 0xFD];
    /// let call = client.post("/upload")?
    ///     .raw(binary_data, ContentType::octet_stream());
    /// # Ok(())
    /// # }
    /// ```
    pub fn raw(mut self, data: Vec<u8>, content_type: headers::ContentType) -> Self {
        let body = CallBody::raw(data, content_type);
        self.body = Some(body);
        self
    }

    /// Sets the request body to plain text.
    ///
    /// This is a convenience method for sending plain text data with
    /// `text/plain` content type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.post("/notes")?.text("This is a plain text note");
    /// # Ok(())
    /// # }
    /// ```
    pub fn text(mut self, text: &str) -> Self {
        let body = CallBody::text(text);
        self.body = Some(body);
        self
    }

    /// Sets the request body to multipart/form-data.
    ///
    /// This method creates a multipart body with a generated boundary and supports
    /// both text fields and file uploads. This is commonly used for file uploads
    /// or when combining different types of data in a single request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let parts = vec![
    ///     ("title", "My Document"),
    ///     ("file", "file content here"),
    /// ];
    /// let call = client.post("/upload")?.multipart(parts);
    /// # Ok(())
    /// # }
    /// ```
    pub fn multipart(mut self, parts: Vec<(&str, &str)>) -> Self {
        let body = CallBody::multipart(parts);
        self.body = Some(body);
        self
    }
}

// Call
impl ApiCall {
    // XXX code to abstract if we want multiple client
    pub async fn exchange(self) -> Result<CallResult, ApiClientError> {
        let Self {
            client,
            base_uri,
            collectors,
            method,
            path,
            query,
            headers,
            body,
            expected_status_codes,
            metadata,
        } = self;

        // Build URL and request
        let url = Self::build_url(&base_uri, &path, &query)?;
        let request = Self::build_request(method.clone(), url, &headers, &body)?;

        // Create operation for OpenAPI documentation
        let operation_id = metadata.operation_id.clone();
        let mut operation =
            Self::build_operation(metadata, &method, &path, query.clone(), &headers, &body);

        // Execute HTTP request
        debug!(?request, "sending...");
        let response = client.execute(request).await?;
        debug!(?response, "...receiving");

        // Validate status code
        let status_code = response.status().as_u16();
        if !expected_status_codes.contains(status_code) {
            // Get the body only if status code is unexpected
            let body = response
                .text()
                .await
                .map(|text| {
                    if text.len() > BODY_MAX_LENGTH {
                        format!("{}... (truncated)", &text[..1024])
                    } else {
                        text
                    }
                })
                .unwrap_or_else(|e| format!("<unable to read response body: {e}>"));
            return Err(ApiClientError::UnexpectedStatusCode { status_code, body });
        }

        // Process response and collect schemas
        let call_result = CallResult::new(operation_id, Arc::clone(&collectors), response).await?;
        operation.add_response(call_result.clone());

        Self::collect_schemas_and_operation(collectors, &path, &headers, operation).await;

        Ok(call_result)
    }

    fn build_url(
        base_uri: &Uri,
        path: &CallPath,
        query: &CallQuery,
    ) -> Result<Url, ApiClientError> {
        let path_resolved = PathResolved::try_from(path.clone())?;
        let url = format!("{base_uri}/{}", path_resolved.path.trim_start_matches('/'));
        let mut url = url.parse::<Url>()?;

        if !query.is_empty() {
            let query_string = query.to_query_string()?;
            url.set_query(Some(&query_string));
        }

        Ok(url)
    }

    fn build_request(
        method: Method,
        url: Url,
        headers: &Option<CallHeaders>,
        body: &Option<CallBody>,
    ) -> Result<Request, ApiClientError> {
        let mut request = Request::new(method, url);
        let req_headers = request.headers_mut();

        // Add custom headers
        if let Some(headers) = headers {
            for (name, value) in headers.to_http_headers()? {
                req_headers.insert(
                    HeaderName::from_bytes(name.as_bytes())?,
                    HeaderValue::from_str(&value)?,
                );
            }
        }

        // Set body
        if let Some(body) = body {
            req_headers.typed_insert(body.content_type.clone());
            let req_body = request.body_mut();
            *req_body = Some(Body::from(body.data.clone()));
        }

        Ok(request)
    }

    fn build_operation(
        metadata: OperationMetadata,
        method: &Method,
        path: &CallPath,
        query: CallQuery,
        headers: &Option<CallHeaders>,
        body: &Option<CallBody>,
    ) -> CalledOperation {
        let OperationMetadata {
            operation_id,
            tags,
            description,
        } = metadata;

        CalledOperation::build(
            operation_id.to_string(),
            method.clone(),
            &path.path,
            path,
            query,
            headers.as_ref(),
            body.as_ref(),
            tags,
            description,
        )
    }

    async fn collect_schemas_and_operation(
        collectors: Arc<RwLock<Collectors>>,
        path: &CallPath,
        headers: &Option<CallHeaders>,
        operation: CalledOperation,
    ) {
        let mut cs = collectors.write().await;
        cs.collect_schemas(path.schemas().clone());
        if let Some(headers) = headers {
            cs.collect_schemas(headers.schemas().clone());
        }
        cs.collect_operation(operation);
    }
}
