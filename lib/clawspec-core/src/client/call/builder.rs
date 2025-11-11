use std::ops::{Range, RangeInclusive};

use serde::Serialize;
use utoipa::ToSchema;

use super::ApiCall;
use crate::client::parameters::{ParamValue, ParameterValue};
use crate::client::response::ExpectedStatusCodes;
use crate::client::{ApiClientError, CallBody, CallCookies, CallHeaders, CallQuery};

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
    pub fn with_authentication(mut self, authentication: crate::client::Authentication) -> Self {
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
