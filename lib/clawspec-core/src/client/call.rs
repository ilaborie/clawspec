use std::future::{Future, IntoFuture};
use std::ops::{Range, RangeInclusive};
use std::pin::Pin;
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

use super::call_parameters::{CallParameters, OperationMetadata};
use super::openapi::{CalledOperation, Collectors};
use super::parameters::{ParameterValue, PathResolved};
use super::response::ExpectedStatusCodes;
use super::{
    ApiClientError, CallBody, CallCookies, CallHeaders, CallPath, CallQuery, CallResult, ParamValue,
};

const BODY_MAX_LENGTH: usize = 1024;

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
/// - [`with_tags(tags)`](Self::with_tags) - Set operation tags (or use automatic tagging)
/// - [`with_description(desc)`](Self::with_description) - Set operation description (or use automatic description)
///
/// ## Response Descriptions
/// - [`with_response_description(desc)`](Self::with_response_description) - Set description for the actual returned status code
///
/// ## Execution
/// - `.await` - Execute the request and return response (⚠️ **must consume result for OpenAPI**)
///
/// # Default Behavior
///
/// - **Status codes**: Accepts 200-499 (inclusive of 200, exclusive of 500)
/// - **Content-Type**: Automatically set based on body type
/// - **Schema collection**: Request/response schemas are automatically captured
/// - **Operation metadata**: Automatically generated if not explicitly set
///
/// ## Automatic OpenAPI Metadata Generation
///
/// When you don't explicitly set operation metadata, `ApiCall` automatically generates:
///
/// ### **Automatic Tags**
/// Tags are extracted from the request path using intelligent parsing:
///
/// ```text
/// Path: /api/v1/users/{id}     → Tags: ["users"]
/// Path: /users                 → Tags: ["users"]
/// Path: /users/export          → Tags: ["users", "export"]
/// Path: /observations/import   → Tags: ["observations", "import"]
/// ```
///
/// **Path Prefix Skipping**: Common API prefixes are automatically skipped:
/// - `api`, `v1`, `v2`, `v3`, `rest`, `service` (and more)
/// - `/api/v1/users` becomes `["users"]`, not `["api", "v1", "users"]`
///
/// **Special Action Detection**: Certain path segments get their own tags:
/// - `import`, `upload`, `export`, `search`, `bulk`
/// - `/users/export` → `["users", "export"]`
///
/// ### **Automatic Descriptions**
/// Descriptions are generated based on HTTP method and path:
///
/// ```text
/// GET /users          → "Retrieve users"
/// GET /users/{id}     → "Retrieve user by ID"  
/// POST /users         → "Create user"
/// PUT /users/{id}     → "Update user by ID"
/// DELETE /users/{id}  → "Delete user by ID"
/// ```
///
/// ### **Automatic Operation IDs**
/// Generated from HTTP method and path: `"get-users-id"`, `"post-users"`, etc.
///
/// You can override any of these by calling the corresponding `with_*` methods.
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

    authentication: Option<super::Authentication>,
    cookies: Option<CallCookies>,
    /// Expected status codes for this request (default: 200..500)
    expected_status_codes: ExpectedStatusCodes,
    /// Operation metadata for OpenAPI documentation
    metadata: OperationMetadata,
    /// Response description for the actual returned status code
    response_description: Option<String>,
    /// Whether to skip collection for OpenAPI documentation (default: false)
    skip_collection: bool,
}

impl ApiCall {
    pub(super) fn build(
        client: reqwest::Client,
        base_uri: Uri,
        collectors: Arc<RwLock<Collectors>>,
        method: Method,
        path: CallPath,
        authentication: Option<super::Authentication>,
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
            authentication,
            cookies: None,
            expected_status_codes: ExpectedStatusCodes::default(),
            metadata: OperationMetadata {
                operation_id,
                tags: None,
                description: None,
                response_description: None,
            },
            response_description: None,
            skip_collection: false,
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

    /// Sets a response description for the actual returned status code.
    ///
    /// This method allows you to document what the response means for your API endpoint.
    /// The description will be applied to whatever status code is actually returned by the server
    /// and included in the generated OpenAPI specification.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/users/{id}")?
    ///     .with_response_description("User details if found, or error information");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_response_description(mut self, description: impl Into<String>) -> Self {
        self.response_description = Some(description.into());
        self
    }

    /// Excludes this API call from OpenAPI collection and documentation generation.
    ///
    /// When called, this API call will be executed normally but will not appear
    /// in the generated OpenAPI specification. This is useful for:
    /// - Health check endpoints
    /// - Debug/diagnostic endpoints  
    /// - Authentication/session management calls
    /// - Test setup/teardown calls
    /// - Internal utility endpoints
    /// - Administrative endpoints not part of public API
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Health check that won't appear in OpenAPI spec
    /// client
    ///     .get("/health")?
    ///     .without_collection()
    ///     .await?
    ///     .as_empty()
    ///     .await?;
    ///
    /// // Debug endpoint excluded from documentation
    /// client
    ///     .get("/debug/status")?
    ///     .without_collection()
    ///     .await?
    ///     .as_text()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn without_collection(mut self) -> Self {
        self.skip_collection = true;
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

    /// Adds cookies to the API call, merging with any existing cookies.
    ///
    /// This method accepts a `CallCookies` instance and merges it with any existing
    /// cookies on the request. Cookies are sent in the HTTP Cookie header and can
    /// be used for session management, authentication, and storing user preferences.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clawspec_core::{ApiClient, CallCookies};
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let cookies = CallCookies::new()
    ///     .add_cookie("session_id", "abc123")
    ///     .add_cookie("user_id", 456);
    ///
    /// let call = client.get("/dashboard")?
    ///     .with_cookies(cookies);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_cookies(mut self, cookies: CallCookies) -> Self {
        self.cookies = match self.cookies.take() {
            Some(existing) => Some(existing.merge(cookies)),
            None => Some(cookies),
        };
        self
    }

    /// Convenience method to add a single cookie.
    ///
    /// This method automatically handles type conversion and merges with existing cookies.
    /// If a cookie with the same name already exists, the new value will override it.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    /// ```rust
    /// # use clawspec_core::ApiClient;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    /// let call = client.get("/dashboard")?
    ///     .with_cookie("session_id", "abc123")
    ///     .with_cookie("user_id", 456);
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
    /// let call = client.get("/preferences")?
    ///     .with_cookie("theme", "dark")                    // String values
    ///     .with_cookie("user_id", 12345_u64)              // Numeric values
    ///     .with_cookie("is_premium", true)                // Boolean values
    ///     .with_cookie("selected_tags", vec!["rust", "web"]); // Array values
    ///
    /// // Cookies can be chained and overridden
    /// let call = client.get("/profile")?
    ///     .with_cookie("session_id", "old-session")
    ///     .with_cookie("session_id", "new-session");      // Overrides previous value
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_cookie<T: ParameterValue>(
        self,
        name: impl Into<String>,
        value: impl Into<ParamValue<T>>,
    ) -> Self {
        let cookies = CallCookies::new().add_cookie(name, value);
        self.with_cookies(cookies)
    }

    /// Overrides the authentication for this specific request.
    ///
    /// This method allows you to use different authentication for a specific request,
    /// overriding the default authentication configured on the API client.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::{ApiClient, Authentication};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Client with default authentication
    /// let mut client = ApiClient::builder()
    ///     .with_authentication(Authentication::Bearer("default-token".into()))
    ///     .build()?;
    ///
    /// // Use different authentication for a specific request
    /// let response = client
    ///     .get("/admin/users")?
    ///     .with_authentication(Authentication::Bearer("admin-token".into()))
    ///     .await?;
    ///
    /// // Remove authentication for a public endpoint
    /// let response = client
    ///     .get("/public/health")?
    ///     .with_authentication_none()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_authentication(mut self, authentication: super::Authentication) -> Self {
        self.authentication = Some(authentication);
        self
    }

    /// Removes authentication for this specific request.
    ///
    /// This is useful when making requests to public endpoints that don't require
    /// authentication, even when the client has default authentication configured.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::{ApiClient, Authentication};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Client with default authentication
    /// let mut client = ApiClient::builder()
    ///     .with_authentication(Authentication::Bearer("token".into()))
    ///     .build()?;
    ///
    /// // Remove authentication for public endpoint
    /// let response = client
    ///     .get("/public/status")?
    ///     .with_authentication_none()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_authentication_none(mut self) -> Self {
        self.authentication = None;
        self
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
    /// Executes the HTTP request and returns a result that must be consumed for OpenAPI generation.
    ///
    /// This method sends the configured HTTP request to the server and returns a [`CallResult`]
    /// that contains the response. **You must call one of the response processing methods**
    /// on the returned `CallResult` to ensure proper OpenAPI documentation generation.
    ///
    /// # ⚠️ Important: Response Consumption Required
    ///
    /// Simply calling `exchange()` is not sufficient! You must consume the [`CallResult`] by
    /// calling one of these methods:
    ///
    /// - [`CallResult::as_empty()`] - For empty responses (204 No Content, DELETE operations, etc.)
    /// - [`CallResult::as_json::<T>()`] - For JSON responses that should be deserialized
    /// - [`CallResult::as_text()`] - For plain text responses
    /// - [`CallResult::as_bytes()`] - For binary responses
    /// - [`CallResult::as_raw()`] - For complete raw response access (status, content-type, body)
    ///
    /// # Example
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
    /// // ✅ CORRECT: Always consume the result
    /// let user: User = client
    ///     .get("/users/123")?
    ///     .await?
    ///     .as_json()  // ← Required for OpenAPI generation!
    ///     .await?;
    ///
    /// // ✅ CORRECT: For operations returning empty responses
    /// client
    ///     .delete("/users/123")?
    ///     .await?
    ///     .as_empty()  // ← Required for OpenAPI generation!
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network issues, timeouts, etc.)
    /// - The response status code is not in the expected range
    /// - Request building fails (invalid URLs, malformed headers, etc.)
    ///
    /// # OpenAPI Documentation
    ///
    /// This method automatically collects operation metadata for OpenAPI generation,
    /// but the response schema and examples are only captured when the [`CallResult`]
    /// is properly consumed with one of the `as_*` methods.
    // TODO: Abstract client implementation to support multiple clients - https://github.com/ilaborie/clawspec/issues/78
    async fn exchange(self) -> Result<CallResult, ApiClientError> {
        let Self {
            client,
            base_uri,
            collectors,
            method,
            path,
            query,
            headers,
            body,
            authentication,
            cookies,
            expected_status_codes,
            metadata,
            response_description,
            skip_collection,
        } = self;

        // Build URL and request
        let url = Self::build_url(&base_uri, &path, &query)?;
        let parameters = CallParameters::with_all(query.clone(), headers.clone(), cookies.clone());
        let request =
            Self::build_request(method.clone(), url, &parameters, &body, &authentication)?;

        // Create operation for OpenAPI documentation
        let operation_id = metadata.operation_id.clone();
        let mut operation = Self::build_operation(
            metadata,
            &method,
            &path,
            parameters.clone(),
            &body,
            response_description,
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

        // Process response and collect schemas (only if collection is enabled)
        let call_result = if skip_collection {
            CallResult::new_without_collection(response).await?
        } else {
            let call_result =
                CallResult::new(operation_id, Arc::clone(&collectors), response).await?;
            operation.add_response(call_result.clone());
            Self::collect_schemas_and_operation(collectors, &path, &parameters, &body, operation)
                .await;
            call_result
        };

        Ok(call_result)
    }

    fn build_url(
        base_uri: &Uri,
        path: &CallPath,
        query: &CallQuery,
    ) -> Result<Url, ApiClientError> {
        let path_resolved = PathResolved::try_from(path.clone())?;
        let base_uri = base_uri.to_string();
        let url = format!(
            "{}/{}",
            base_uri.trim_end_matches('/'),
            path_resolved.path.trim_start_matches('/')
        );
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
        parameters: &CallParameters,
        body: &Option<CallBody>,
        authentication: &Option<super::Authentication>,
    ) -> Result<Request, ApiClientError> {
        let mut request = Request::new(method, url);
        let req_headers = request.headers_mut();

        // Add authentication header if present
        if let Some(auth) = authentication {
            let (header_name, header_value) = auth.to_header()?;
            req_headers.insert(header_name, header_value);
        }

        // Add custom headers
        for (name, value) in parameters.to_http_headers()? {
            req_headers.insert(
                HeaderName::from_bytes(name.as_bytes())?,
                HeaderValue::from_str(&value)?,
            );
        }

        // Add cookies as Cookie header
        let cookie_header = parameters.to_cookie_header()?;
        if !cookie_header.is_empty() {
            req_headers.insert(
                HeaderName::from_static("cookie"),
                HeaderValue::from_str(&cookie_header)?,
            );
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
        parameters: CallParameters,
        body: &Option<CallBody>,
        response_description: Option<String>,
    ) -> CalledOperation {
        let OperationMetadata {
            operation_id,
            tags,
            description,
            response_description: _,
        } = metadata;

        CalledOperation::build(
            method.clone(),
            &path.path,
            path,
            parameters,
            body.as_ref(),
            OperationMetadata {
                operation_id: operation_id.to_string(),
                tags,
                description,
                response_description,
            },
        )
    }

    async fn collect_schemas_and_operation(
        collectors: Arc<RwLock<Collectors>>,
        path: &CallPath,
        parameters: &CallParameters,
        body: &Option<CallBody>,
        operation: CalledOperation,
    ) {
        let mut cs = collectors.write().await;
        cs.collect_schemas(path.schemas().clone());
        cs.collect_schemas(parameters.collect_schemas());
        if let Some(body) = body {
            cs.collect_schema_entry(body.entry.clone());
        }
        cs.collect_operation(operation);
    }
}

/// Implement IntoFuture for ApiCall to enable direct .await syntax
///
/// This provides a more ergonomic API by allowing direct `.await` on ApiCall:
/// ```rust,no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut client = clawspec_core::ApiClient::builder().build()?;
/// let response = client.get("/users")?.await?;
/// # Ok(())
/// # }
/// ```
impl IntoFuture for ApiCall {
    type Output = Result<CallResult, ApiClientError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.exchange())
    }
}

#[cfg(test)]
mod tests {
    use crate::{CallPath, CallQuery};

    use super::*;
    use http::{Method, StatusCode};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use utoipa::ToSchema;

    #[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
    }

    // Helper function to create a basic ApiCall for testing
    fn create_test_api_call() -> ApiCall {
        let client = reqwest::Client::new();
        let base_uri = "http://localhost:8080".parse().unwrap();
        let collectors = Arc::new(RwLock::new(Collectors::default()));
        let method = Method::GET;
        let path = CallPath::from("/test");

        ApiCall::build(client, base_uri, collectors, method, path, None).unwrap()
    }

    // Test OperationMetadata creation and defaults
    #[test]
    fn test_operation_metadata_default() {
        let metadata = OperationMetadata::default();
        assert!(metadata.operation_id.is_empty());
        assert!(metadata.tags.is_none());
        assert!(metadata.description.is_none());
    }

    #[test]
    fn test_operation_metadata_creation() {
        let metadata = OperationMetadata {
            operation_id: "test-operation".to_string(),
            tags: Some(vec!["users".to_string(), "admin".to_string()]),
            description: Some("Test operation description".to_string()),
            response_description: Some("Test response description".to_string()),
        };

        assert_eq!(metadata.operation_id, "test-operation");
        assert_eq!(
            metadata.tags,
            Some(vec!["users".to_string(), "admin".to_string()])
        );
        assert_eq!(
            metadata.description,
            Some("Test operation description".to_string())
        );
    }

    // Test ApiCall creation and builder methods
    #[test]
    fn test_api_call_build_success() {
        let call = create_test_api_call();
        assert_eq!(call.method, Method::GET);
        assert_eq!(call.path.path, "/test");
        assert!(call.query.is_empty());
        assert!(call.headers.is_none());
        assert!(call.body.is_none());
    }

    #[test]
    fn test_api_call_with_operation_id() {
        let call = create_test_api_call().with_operation_id("custom-operation-id");

        assert_eq!(call.metadata.operation_id, "custom-operation-id");
    }

    #[test]
    fn test_api_call_with_description() {
        let call = create_test_api_call().with_description("Custom description");

        assert_eq!(
            call.metadata.description,
            Some("Custom description".to_string())
        );
    }

    #[test]
    fn test_api_call_with_tags_vec() {
        let tags = vec!["users", "admin", "api"];
        let call = create_test_api_call().with_tags(tags.clone());

        let expected_tags: Vec<String> = tags.into_iter().map(|s| s.to_string()).collect();
        assert_eq!(call.metadata.tags, Some(expected_tags));
    }

    #[test]
    fn test_api_call_with_tags_array() {
        let call = create_test_api_call().with_tags(["users", "admin"]);

        assert_eq!(
            call.metadata.tags,
            Some(vec!["users".to_string(), "admin".to_string()])
        );
    }

    #[test]
    fn test_api_call_with_tag_single() {
        let call = create_test_api_call().with_tag("users").with_tag("admin");

        assert_eq!(
            call.metadata.tags,
            Some(vec!["users".to_string(), "admin".to_string()])
        );
    }

    #[test]
    fn test_api_call_with_tag_on_empty_tags() {
        let call = create_test_api_call().with_tag("users");

        assert_eq!(call.metadata.tags, Some(vec!["users".to_string()]));
    }

    // Test query parameter methods
    #[test]
    fn test_api_call_with_query() {
        let query = CallQuery::new()
            .add_param("page", ParamValue::new(1))
            .add_param("limit", ParamValue::new(10));

        let call = create_test_api_call().with_query(query.clone());

        // Test that the query was set (we can't access private fields, but we can test the behavior)
        assert!(!call.query.is_empty());
    }

    // Test header methods
    #[test]
    fn test_api_call_with_headers() {
        let headers = CallHeaders::new().add_header("Authorization", "Bearer token");

        let call = create_test_api_call().with_headers(headers);

        assert!(call.headers.is_some());
    }

    #[test]
    fn test_api_call_with_header_single() {
        let call = create_test_api_call()
            .with_header("Authorization", "Bearer token")
            .with_header("Content-Type", "application/json");

        assert!(call.headers.is_some());
        // We can test that headers were set without accessing private fields
        // The presence of headers confirms the functionality works
    }

    #[test]
    fn test_api_call_with_header_merge() {
        let initial_headers = CallHeaders::new().add_header("X-Request-ID", "abc123");

        let call = create_test_api_call()
            .with_headers(initial_headers)
            .with_header("Authorization", "Bearer token");

        assert!(call.headers.is_some());
        // Test that merging worked by confirming headers exist
        let _headers = call.headers.unwrap();
    }

    // Test status code validation methods
    #[test]
    fn test_api_call_with_expected_status() {
        let call = create_test_api_call().with_expected_status(201);

        assert!(call.expected_status_codes.contains(201));
        assert!(!call.expected_status_codes.contains(200));
    }

    #[test]
    fn test_api_call_with_status_range_inclusive() {
        let call = create_test_api_call().with_status_range_inclusive(200..=299);

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(250));
        assert!(call.expected_status_codes.contains(299));
        assert!(!call.expected_status_codes.contains(300));
    }

    #[test]
    fn test_api_call_with_status_range_exclusive() {
        let call = create_test_api_call().with_status_range(200..300);

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(299));
        assert!(!call.expected_status_codes.contains(300));
    }

    #[test]
    fn test_api_call_add_expected_status() {
        let call = create_test_api_call()
            .with_status_range_inclusive(200..=299)
            .add_expected_status(404);

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(299));
        assert!(call.expected_status_codes.contains(404));
        assert!(!call.expected_status_codes.contains(405));
    }

    #[test]
    fn test_api_call_add_expected_status_range_inclusive() {
        let call = create_test_api_call()
            .with_status_range_inclusive(200..=204)
            .add_expected_status_range_inclusive(400..=404);

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(204));
        assert!(call.expected_status_codes.contains(400));
        assert!(call.expected_status_codes.contains(404));
        assert!(!call.expected_status_codes.contains(205));
        assert!(!call.expected_status_codes.contains(405));
    }

    #[test]
    fn test_api_call_add_expected_status_range_exclusive() {
        let call = create_test_api_call()
            .with_status_range_inclusive(200..=204)
            .add_expected_status_range(400..404);

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(204));
        assert!(call.expected_status_codes.contains(400));
        assert!(call.expected_status_codes.contains(403));
        assert!(!call.expected_status_codes.contains(404));
    }

    #[test]
    fn test_api_call_with_success_only() {
        let call = create_test_api_call().with_success_only();

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(299));
        assert!(!call.expected_status_codes.contains(300));
        assert!(!call.expected_status_codes.contains(400));
    }

    #[test]
    fn test_api_call_with_client_errors() {
        let call = create_test_api_call().with_client_errors();

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(299));
        assert!(call.expected_status_codes.contains(400));
        assert!(call.expected_status_codes.contains(499));
        assert!(!call.expected_status_codes.contains(300));
        assert!(!call.expected_status_codes.contains(500));
    }

    #[test]
    fn test_api_call_with_expected_status_codes() {
        let codes = ExpectedStatusCodes::from_single(201).add_expected_status(404);

        let call = create_test_api_call().with_expected_status_codes(codes);

        assert!(call.expected_status_codes.contains(201));
        assert!(call.expected_status_codes.contains(404));
        assert!(!call.expected_status_codes.contains(200));
    }

    #[test]
    fn test_api_call_with_expected_status_code_http() {
        let call = create_test_api_call().with_expected_status_code(StatusCode::CREATED);

        assert!(call.expected_status_codes.contains(201));
        assert!(!call.expected_status_codes.contains(200));
    }

    #[test]
    fn test_api_call_with_expected_status_code_range_http() {
        let call = create_test_api_call()
            .with_expected_status_code_range(StatusCode::OK..=StatusCode::NO_CONTENT);

        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(204));
        assert!(!call.expected_status_codes.contains(205));
    }

    // Test request body methods
    #[test]
    fn test_api_call_json_body() {
        let test_data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        let call = create_test_api_call()
            .json(&test_data)
            .expect("should set JSON body");

        assert!(call.body.is_some());
        let body = call.body.unwrap();
        assert_eq!(body.content_type, headers::ContentType::json());

        // Verify the JSON data can be deserialized back
        let parsed: TestData = serde_json::from_slice(&body.data).expect("should parse JSON");
        assert_eq!(parsed, test_data);
    }

    #[test]
    fn test_api_call_form_body() {
        let test_data = TestData {
            id: 42,
            name: "form test".to_string(),
        };

        let call = create_test_api_call()
            .form(&test_data)
            .expect("should set form body");

        assert!(call.body.is_some());
        let body = call.body.unwrap();
        assert_eq!(body.content_type, headers::ContentType::form_url_encoded());
    }

    #[test]
    fn test_api_call_text_body() {
        let text_content = "Hello, World!";

        let call = create_test_api_call().text(text_content);

        assert!(call.body.is_some());
        let body = call.body.unwrap();
        assert_eq!(body.content_type, headers::ContentType::text());
        assert_eq!(body.data, text_content.as_bytes());
    }

    #[test]
    fn test_api_call_raw_body() {
        let binary_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
        let content_type = headers::ContentType::octet_stream();

        let call = create_test_api_call().raw(binary_data.clone(), content_type.clone());

        assert!(call.body.is_some());
        let body = call.body.unwrap();
        assert_eq!(body.content_type, content_type);
        assert_eq!(body.data, binary_data);
    }

    #[test]
    fn test_api_call_multipart_body() {
        let parts = vec![("title", "My Document"), ("description", "A test document")];

        let call = create_test_api_call().multipart(parts);

        assert!(call.body.is_some());
        let body = call.body.unwrap();
        // Content type should be multipart/form-data with boundary
        assert!(
            body.content_type
                .to_string()
                .starts_with("multipart/form-data")
        );
    }

    // Test URL building (helper function tests)
    #[test]
    fn test_build_url_simple_path() {
        let base_uri: Uri = "http://localhost:8080".parse().unwrap();
        let path = CallPath::from("/users");
        let query = CallQuery::default();

        let url = ApiCall::build_url(&base_uri, &path, &query).expect("should build URL");
        // The actual implementation results in double slash due to URI parsing
        assert_eq!(url.to_string(), "http://localhost:8080/users");
    }

    #[test]
    fn test_build_url_with_query() {
        let base_uri: Uri = "http://localhost:8080".parse().unwrap();
        let path = CallPath::from("/users");
        let query = CallQuery::new()
            .add_param("page", ParamValue::new(1))
            .add_param("limit", ParamValue::new(10));

        let url = ApiCall::build_url(&base_uri, &path, &query).expect("should build URL");
        // Query order might vary, so check both possibilities
        let url_str = url.to_string();
        assert!(url_str.starts_with("http://localhost:8080/users?"));
        assert!(url_str.contains("page=1"));
        assert!(url_str.contains("limit=10"));
    }

    #[test]
    fn test_build_url_with_path_params() {
        let base_uri: Uri = "http://localhost:8080".parse().unwrap();
        let path = CallPath::from("/users/{id}").add_param("id", ParamValue::new(123));
        let query = CallQuery::default();

        let url = ApiCall::build_url(&base_uri, &path, &query).expect("should build URL");
        assert_eq!(url.to_string(), "http://localhost:8080/users/123");
    }

    // Test request building (helper function tests)
    #[test]
    fn test_build_request_simple() {
        let method = Method::GET;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let body = None;
        let parameters = CallParameters::default();

        let request =
            ApiCall::build_request(method.clone(), url.clone(), &parameters, &body, &None)
                .expect("should build request");

        assert_eq!(request.method(), &method);
        assert_eq!(request.url(), &url);
        assert!(request.body().is_none());
    }

    #[test]
    fn test_build_request_with_headers() {
        let method = Method::GET;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let headers = Some(CallHeaders::new().add_header("Authorization", "Bearer token"));
        let body = None;
        let parameters = CallParameters::with_all(CallQuery::new(), headers, None);

        let request = ApiCall::build_request(method, url, &parameters, &body, &None)
            .expect("should build request");

        assert!(request.headers().get("authorization").is_some());
    }

    #[test]
    fn test_build_request_with_body() {
        let method = Method::POST;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let test_data = TestData {
            id: 1,
            name: "test".to_string(),
        };
        let body = Some(CallBody::json(&test_data).expect("should create JSON body"));
        let parameters = CallParameters::default();

        let request = ApiCall::build_request(method, url, &parameters, &body, &None)
            .expect("should build request");

        assert!(request.body().is_some());
        assert_eq!(
            request.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    // Test method chaining
    #[test]
    fn test_api_call_method_chaining() {
        let test_data = TestData {
            id: 1,
            name: "chaining test".to_string(),
        };

        let call = create_test_api_call()
            .with_operation_id("test-chain")
            .with_description("Method chaining test")
            .with_tag("test")
            .with_tag("chaining")
            .with_header("Authorization", "Bearer token")
            .with_header("X-Request-ID", "test-123")
            .with_status_range_inclusive(200..=201)
            .add_expected_status(404)
            .json(&test_data)
            .expect("should set JSON body");

        // Verify all settings were applied
        assert_eq!(call.metadata.operation_id, "test-chain");
        assert_eq!(
            call.metadata.description,
            Some("Method chaining test".to_string())
        );
        assert_eq!(
            call.metadata.tags,
            Some(vec!["test".to_string(), "chaining".to_string()])
        );
        assert!(call.headers.is_some());
        assert!(call.body.is_some());
        assert!(call.expected_status_codes.contains(200));
        assert!(call.expected_status_codes.contains(201));
        assert!(call.expected_status_codes.contains(404));
    }

    // Test edge cases and error conditions
    #[test]
    fn test_api_call_json_serialization_error() {
        // This would test JSON serialization errors, but TestData is always serializable
        // In a real scenario, you'd test with a type that fails to serialize
        // For now, we'll test the success case
        let test_data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        let result = create_test_api_call().json(&test_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_api_call_form_serialization_error() {
        // Similar to JSON test - TestData is always serializable
        let test_data = TestData {
            id: 1,
            name: "test".to_string(),
        };

        let result = create_test_api_call().form(&test_data);
        assert!(result.is_ok());
    }

    // Test constants
    #[test]
    fn test_body_max_length_constant() {
        assert_eq!(BODY_MAX_LENGTH, 1024);
    }

    // Test collection exclusion functionality
    #[test]
    fn test_without_collection_sets_flag() {
        let call = create_test_api_call().without_collection();
        assert!(call.skip_collection);
    }

    #[test]
    fn test_default_collection_flag() {
        let call = create_test_api_call();
        assert!(!call.skip_collection);
    }

    #[test]
    fn test_without_collection_chaining() {
        let call = create_test_api_call()
            .with_operation_id("test-operation")
            .with_description("Test operation")
            .without_collection()
            .with_header("Authorization", "Bearer token");

        assert!(call.skip_collection);
        assert_eq!(call.metadata.operation_id, "test-operation");
        assert_eq!(
            call.metadata.description,
            Some("Test operation".to_string())
        );
        assert!(call.headers.is_some());
    }

    // Test IntoFuture implementation
    #[test]
    fn test_api_call_into_future_type_requirements() {
        // Test that ApiCall implements IntoFuture with the correct associated types
        use std::future::IntoFuture;

        fn assert_into_future<T>(_: T)
        where
            T: IntoFuture<Output = Result<CallResult, ApiClientError>>,
            T::IntoFuture: Send,
        {
        }

        let call = create_test_api_call();
        assert_into_future(call);
    }

    #[tokio::test]
    async fn test_api_call_into_future_equivalence() {
        // Test that ApiCall.await works correctly by testing the IntoFuture implementation
        // This is a compile-time test that verifies the future type structure is correct

        use std::future::IntoFuture;

        let call1 = create_test_api_call();
        let call2 = create_test_api_call();

        // Test that both direct await and explicit into_future produce the same type
        let _future1 = call1.into_future();
        let _future2 = call2.into_future();

        // Both should be Send futures
        fn assert_send<T: Send>(_: T) {}
        assert_send(_future1);
        assert_send(_future2);
    }

    #[test]
    fn test_into_future_api_demonstration() {
        // This test demonstrates the new API usage patterns
        // Note: This is a compile-time test showing the API ergonomics

        use crate::ApiClient;
        use std::future::IntoFuture;

        // Demonstrate the new API pattern compiles correctly
        fn assert_new_api_compiles() {
            async fn _example() -> Result<(), ApiClientError> {
                let client = ApiClient::builder().build()?;

                // Create path with parameters
                let path = CallPath::from("/users/{id}").add_param("id", 123);

                let query = CallQuery::new().add_param("include_details", true);

                // Direct .await API (using IntoFuture)
                let _response = client
                    .get(path)?
                    .with_query(query)
                    .with_header("Authorization", "Bearer token")
                    .await?; // Direct await

                Ok(())
            }
        }

        // Test that the function compiles
        assert_new_api_compiles();

        // Demonstrate that ApiCall implements IntoFuture with correct types
        let call = create_test_api_call();
        #[allow(clippy::let_underscore_future)]
        let _: Pin<Box<dyn Future<Output = Result<CallResult, ApiClientError>> + Send>> =
            call.into_future();
    }

    #[test]
    fn test_api_call_with_response_description() {
        let call = create_test_api_call();
        let call = call.with_response_description("Success response");
        assert_eq!(
            call.response_description,
            Some("Success response".to_string())
        );
    }

    #[test]
    fn test_api_call_response_description_method_chaining() {
        let call = create_test_api_call();
        let call = call
            .with_response_description("Original description")
            .with_response_description("Overridden description");

        // Latest description should override the previous one
        assert_eq!(
            call.response_description,
            Some("Overridden description".to_string())
        );
    }

    #[test]
    fn test_api_call_response_description_none_by_default() {
        let call = create_test_api_call();
        assert_eq!(call.response_description, None);
    }

    #[test]
    fn test_api_call_with_authentication_bearer() {
        let mut call = create_test_api_call();
        call = call.with_authentication(super::super::Authentication::Bearer("test-token".into()));

        assert!(matches!(
            call.authentication,
            Some(super::super::Authentication::Bearer(ref token)) if token.equals_str("test-token")
        ));
    }

    #[test]
    fn test_api_call_with_authentication_basic() {
        let mut call = create_test_api_call();
        call = call.with_authentication(super::super::Authentication::Basic {
            username: "user".to_string(),
            password: "pass".into(),
        });

        assert!(matches!(
            call.authentication,
            Some(super::super::Authentication::Basic { ref username, ref password })
                if username == "user" && password.equals_str("pass")
        ));
    }

    #[test]
    fn test_api_call_with_authentication_api_key() {
        let mut call = create_test_api_call();
        call = call.with_authentication(super::super::Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key".into(),
        });

        assert!(matches!(
            call.authentication,
            Some(super::super::Authentication::ApiKey { ref header_name, ref key })
                if header_name == "X-API-Key" && key.equals_str("secret-key")
        ));
    }

    #[test]
    fn test_api_call_with_authentication_none() {
        let mut call = create_test_api_call();
        // First set authentication
        call = call.with_authentication(super::super::Authentication::Bearer("token".into()));
        assert!(call.authentication.is_some());

        // Then remove it
        call = call.with_authentication_none();
        assert!(call.authentication.is_none());
    }

    #[test]
    fn test_build_request_with_bearer_auth() {
        let method = Method::GET;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let parameters = CallParameters::default();
        let body = None;
        let auth = Some(super::super::Authentication::Bearer("test-token".into()));

        let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
            .expect("should build request");

        let auth_header = request.headers().get("authorization");
        assert!(auth_header.is_some());
        assert_eq!(auth_header.unwrap(), "Bearer test-token");
    }

    #[test]
    fn test_build_request_with_basic_auth() {
        let method = Method::GET;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let parameters = CallParameters::default();
        let body = None;
        let auth = Some(super::super::Authentication::Basic {
            username: "user".to_string(),
            password: "pass".into(),
        });

        let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
            .expect("should build request");

        let auth_header = request.headers().get("authorization");
        assert!(auth_header.is_some());
        // "user:pass" base64 encoded is "dXNlcjpwYXNz"
        assert_eq!(auth_header.unwrap(), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn test_build_request_with_api_key_auth() {
        let method = Method::GET;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let parameters = CallParameters::default();
        let body = None;
        let auth = Some(super::super::Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key-123".into(),
        });

        let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
            .expect("should build request");

        let api_key_header = request.headers().get("X-API-Key");
        assert!(api_key_header.is_some());
        assert_eq!(api_key_header.unwrap(), "secret-key-123");
    }

    #[test]
    fn test_build_request_without_auth() {
        let method = Method::GET;
        let url: Url = "http://localhost:8080/users".parse().unwrap();
        let parameters = CallParameters::default();
        let body = None;
        let auth = None;

        let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
            .expect("should build request");

        assert!(request.headers().get("authorization").is_none());
        assert!(request.headers().get("X-API-Key").is_none());
    }

    #[test]
    fn test_authentication_override_in_method_chaining() {
        let mut call = create_test_api_call();

        // Start with no authentication
        assert!(call.authentication.is_none());

        // Add bearer authentication
        call = call.with_authentication(super::super::Authentication::Bearer("token1".into()));
        assert!(matches!(
            call.authentication,
            Some(super::super::Authentication::Bearer(ref token)) if token.equals_str("token1")
        ));

        // Override with basic authentication
        call = call.with_authentication(super::super::Authentication::Basic {
            username: "user".to_string(),
            password: "pass".into(),
        });
        assert!(matches!(
            call.authentication,
            Some(super::super::Authentication::Basic { .. })
        ));

        // Remove authentication
        call = call.with_authentication_none();
        assert!(call.authentication.is_none());
    }
}
