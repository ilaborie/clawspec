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
use super::path::PathResolved;
use super::{ApiClientError, CallBody, CallHeaders, CallPath, CallQuery, CallResult};

const BODY_MAX_LENGTH: usize = 1024;

/// Expected status codes for HTTP requests.
///
/// Supports multiple ranges and individual status codes for flexible validation.
#[derive(Debug, Clone)]
pub struct ExpectedStatusCodes {
    ranges: Vec<StatusCodeRange>,
}

/// Represents a range of status codes (inclusive or exclusive).
#[derive(Debug, Clone)]
enum StatusCodeRange {
    Single(u16),
    Inclusive(RangeInclusive<u16>),
    Exclusive(Range<u16>),
}

impl ExpectedStatusCodes {
    /// Creates a new set of expected status codes with default range (200..500).
    pub fn new() -> Self {
        Self {
            ranges: vec![StatusCodeRange::Exclusive(200..500)],
        }
    }

    /// Adds a single status code as valid.
    pub fn add_single(mut self, status: u16) -> Self {
        self.ranges.push(StatusCodeRange::Single(status));
        self
    }

    /// Adds an inclusive range of status codes.
    pub fn add_inclusive_range(mut self, range: RangeInclusive<u16>) -> Self {
        self.ranges.push(StatusCodeRange::Inclusive(range));
        self
    }

    /// Adds an exclusive range of status codes.
    pub fn add_exclusive_range(mut self, range: Range<u16>) -> Self {
        self.ranges.push(StatusCodeRange::Exclusive(range));
        self
    }

    /// Creates expected status codes from a single inclusive range.
    pub fn from_inclusive_range(range: RangeInclusive<u16>) -> Self {
        Self {
            ranges: vec![StatusCodeRange::Inclusive(range)],
        }
    }

    /// Creates expected status codes from a single exclusive range.
    pub fn from_exclusive_range(range: Range<u16>) -> Self {
        Self {
            ranges: vec![StatusCodeRange::Exclusive(range)],
        }
    }

    /// Creates expected status codes from a single status code.
    pub fn from_single(status: u16) -> Self {
        Self {
            ranges: vec![StatusCodeRange::Single(status)],
        }
    }

    /// Checks if a status code is expected/valid.
    pub fn contains(&self, status: u16) -> bool {
        self.ranges.iter().any(|range| match range {
            StatusCodeRange::Single(s) => *s == status,
            StatusCodeRange::Inclusive(r) => r.contains(&status),
            StatusCodeRange::Exclusive(r) => r.contains(&status),
        })
    }

    /// Adds a single status code to the existing set (for chaining).
    pub fn add_expected_status(mut self, status: u16) -> Self {
        self.ranges.push(StatusCodeRange::Single(status));
        self
    }

    /// Adds an inclusive range to the existing set (for chaining).
    pub fn add_expected_range(mut self, range: RangeInclusive<u16>) -> Self {
        self.ranges.push(StatusCodeRange::Inclusive(range));
        self
    }
}

impl Default for ExpectedStatusCodes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod status_code_tests {
    use super::*;

    #[test]
    fn test_default_status_codes() {
        let codes = ExpectedStatusCodes::default();

        // Should accept 200..500 range by default
        assert!(codes.contains(200));
        assert!(codes.contains(299));
        assert!(codes.contains(400));
        assert!(codes.contains(499));

        // Should reject outside range
        assert!(!codes.contains(199));
        assert!(!codes.contains(500));
    }

    #[test]
    fn test_single_status_code() {
        let codes = ExpectedStatusCodes::from_single(204);

        assert!(codes.contains(204));
        assert!(!codes.contains(200));
        assert!(!codes.contains(205));
    }

    #[test]
    fn test_inclusive_range() {
        let codes = ExpectedStatusCodes::from_inclusive_range(200..=204);

        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(204));
        assert!(!codes.contains(199));
        assert!(!codes.contains(205));
    }

    #[test]
    fn test_exclusive_range() {
        let codes = ExpectedStatusCodes::from_exclusive_range(200..300);

        assert!(codes.contains(200));
        assert!(codes.contains(299));
        assert!(!codes.contains(199));
        assert!(!codes.contains(300));
    }

    #[test]
    fn test_multiple_ranges() {
        let codes = ExpectedStatusCodes::from_inclusive_range(200..=204)
            .add_expected_status(404)
            .add_expected_range(500..=503);

        // Should accept first range
        assert!(codes.contains(200));
        assert!(codes.contains(204));

        // Should accept single status
        assert!(codes.contains(404));

        // Should accept second range
        assert!(codes.contains(500));
        assert!(codes.contains(503));

        // Should reject outside ranges
        assert!(!codes.contains(205));
        assert!(!codes.contains(405));
        assert!(!codes.contains(504));
    }
}

/// Builder for configuring HTTP API calls with comprehensive status code validation.
///
/// Supports flexible status code expectations through multiple methods:
/// - `set_expected_status(code)` - Set a single expected status code
/// - `set_status_range_inclusive(range)` - Set an inclusive range (e.g., 200..=299)
/// - `set_status_range(range)` - Set an exclusive range (e.g., 200..300)
/// - `add_expected_status(code)` - Add additional expected status codes
/// - `add_expected_status_range_inclusive(range)` - Add additional inclusive ranges
/// - `add_expected_status_range(range)` - Add additional exclusive ranges
///
/// Default behavior accepts status codes 200..500 (exclusive).
// TODO: Add comprehensive documentation for all public APIs - https://github.com/ilaborie/clawspec/issues/34
// TODO: Standardize builder patterns for consistency - https://github.com/ilaborie/clawspec/issues/33
#[derive(derive_more::Debug)]
pub struct ApiCall {
    client: reqwest::Client,
    base_uri: Uri,
    collectors: Arc<RwLock<Collectors>>,

    operation_id: String,
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
            operation_id,
            method,
            path,
            query: CallQuery::default(),
            headers: None,
            body: None,
            expected_status_codes: ExpectedStatusCodes::default(),
        };
        Ok(result)
    }
}

// Builder
impl ApiCall {
    pub fn operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = operation_id.into();
        self
    }

    pub fn query(mut self, query: CallQuery) -> Self {
        self.query = query;
        self
    }

    pub fn headers(mut self, headers: Option<CallHeaders>) -> Self {
        self.headers = match (self.headers.take(), headers) {
            (Some(existing), Some(new)) => Some(existing.merge(new)),
            (existing, new) => existing.or(new),
        };
        self
    }

    /// Sets the expected status codes for this request using an inclusive range.
    ///
    /// By default, status codes 200..500 are considered successful.
    /// Use this method to customize which status codes should be accepted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept only 200 to 201 (inclusive)
    /// let call = client.post("/users")?.set_status_range_inclusive(200..=201);
    ///
    /// // Accept any 2xx status code
    /// let call = client.get("/users")?.set_status_range_inclusive(200..=299);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_status_range_inclusive(mut self, range: RangeInclusive<u16>) -> Self {
        self.expected_status_codes = ExpectedStatusCodes::from_inclusive_range(range);
        self
    }

    /// Sets the expected status codes for this request using an exclusive range.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200 to 299 (200 included, 300 excluded)
    /// let call = client.get("/users")?.set_status_range(200..300);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_status_range(mut self, range: Range<u16>) -> Self {
        self.expected_status_codes = ExpectedStatusCodes::from_exclusive_range(range);
        self
    }

    /// Sets a single expected status code for this request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept only 204 for DELETE operations
    /// let call = client.delete("/users/123")?.set_expected_status(204);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_expected_status(mut self, status: u16) -> Self {
        self.expected_status_codes = ExpectedStatusCodes::from_single(status);
        self
    }

    /// Adds an additional expected status code to the existing set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200..299 and also 404
    /// let call = client.get("/users")?.set_status_range_inclusive(200..=299).add_expected_status(404);
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
    /// # use clawspec_utoipa::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200..=204 and also 400..=402
    /// let call = client.post("/users")?.set_status_range_inclusive(200..=204).add_expected_status_range_inclusive(400..=402);
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
    /// # use clawspec_utoipa::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Accept 200..=204 and also 400..403
    /// let call = client.post("/users")?.set_status_range_inclusive(200..=204).add_expected_status_range(400..403);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_expected_status_range(mut self, range: Range<u16>) -> Self {
        self.expected_status_codes = self.expected_status_codes.add_exclusive_range(range);
        self
    }

    /// Adds headers to the API call, merging with any existing headers.
    ///
    /// This is a convenience method that automatically wraps the headers in Some().
    pub fn with_headers(self, headers: CallHeaders) -> Self {
        self.headers(Some(headers))
    }

    /// Sets the request body to JSON.
    ///
    /// This method serializes the provided data as JSON and sets the
    /// Content-Type header to `application/json`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_utoipa::ApiClient;
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
    /// # use clawspec_utoipa::ApiClient;
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
    /// # use clawspec_utoipa::ApiClient;
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
    /// # use clawspec_utoipa::ApiClient;
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
    /// # use clawspec_utoipa::ApiClient;
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
            operation_id,
            method,
            path,
            query,
            headers,
            body,
            expected_status_codes,
        } = self;

        // Build URL and request
        let url = Self::build_url(&base_uri, &path, &query)?;
        let request = Self::build_request(method.clone(), url, &headers, &body)?;

        // Create operation for OpenAPI documentation
        let mut operation = Self::build_operation(
            &operation_id,
            &method,
            &path,
            query.clone(),
            &headers,
            &body,
        );

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
        operation_id: &str,
        method: &Method,
        path: &CallPath,
        query: CallQuery,
        headers: &Option<CallHeaders>,
        body: &Option<CallBody>,
    ) -> CalledOperation {
        CalledOperation::build(
            operation_id.to_string(),
            method.clone(),
            &path.path,
            path,
            query,
            headers.as_ref(),
            body.as_ref(),
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
