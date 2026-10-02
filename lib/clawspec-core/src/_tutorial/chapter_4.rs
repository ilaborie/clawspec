//! # Chapter 4: Advanced Parameters
//!
//! This chapter covers headers, cookies, and advanced parameter serialization styles.
//!
//! ## Header Parameters
//!
//! Use [`CallHeaders`][crate::CallHeaders] to add request headers:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallHeaders};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Response { data: String }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let headers = CallHeaders::new()
//!     .add_header("X-Request-ID", "req-12345")
//!     .add_header("X-Client-Version", "1.0.0")
//!     .add_header("Accept-Language", "en-US");
//!
//! let response: Response = client
//!     .get("/data")?
//!     .with_headers(headers)
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! Headers are documented in the OpenAPI specification as header parameters.
//!
//! ## Cookie Parameters
//!
//! Use [`CallCookies`][crate::CallCookies] for cookie-based parameters:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallCookies};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Profile { name: String }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let cookies = CallCookies::new()
//!     .add_cookie("session_id", "abc123")
//!     .add_cookie("user_id", 42)
//!     .add_cookie("preferences", vec!["dark_mode", "compact"]);
//!
//! let profile: Profile = client
//!     .get("/profile")?
//!     .with_cookies(cookies)
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! Clawspec sends all cookies in one `Cookie` header, as `name=value` pairs joined by `; `,
//! without percent-encoding, and with array values joined by commas (the style of a
//! `ParamValue` is ignored for cookies). Each cookie parameter
//! is documented with `style: cookie` and `explode: false` to describe this serialization.
//! For an older OpenAPI output, the style and explode are removed.
//!
//! ## Authentication
//!
//! Clawspec supports several authentication methods:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, Authentication};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Bearer token (most common for APIs)
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_authentication(Authentication::Bearer("your-api-token".into()))
//!     .build()?;
//!
//! // Basic authentication
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_authentication(Authentication::Basic {
//!         username: "user".to_string(),
//!         password: "secret".into(),
//!     })
//!     .build()?;
//!
//! // API key in header
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_authentication(Authentication::ApiKey {
//!         header_name: "X-API-Key".to_string(),
//!         key: "your-api-key".into(),
//!     })
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! You can override authentication per-request:
//!
//! ```rust,no_run
//! # use clawspec_core::{ApiClient, Authentication};
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_authentication(Authentication::Bearer("default-token".into()))
//!     .build()?;
//!
//! // Use different auth for admin endpoints
//! client.get("/admin/users")?
//!     .with_authentication(Authentication::Bearer("admin-token".into()))
//!     .await?;
//!
//! // Remove auth for public endpoints
//! client.get("/public/status")?
//!     .with_authentication_none()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Parameter Styles
//!
//! OpenAPI defines various serialization styles for parameters. Use [`ParamStyle`][crate::ParamStyle]:
//!
//! ### Path Parameter Styles
//!
//! ```rust
//! use clawspec_core::{CallPath, ParamValue, ParamStyle};
//!
//! // Simple (default): /users/123
//! let path = CallPath::from("/users/{id}")
//!     .add_param("id", ParamValue::new(123));
//!
//! // Label style: /users/.123
//! let path = CallPath::from("/users/{id}")
//!     .add_param("id", ParamValue::with_style(123, ParamStyle::Label));
//!
//! // Matrix style: /users/;id=123
//! let path = CallPath::from("/users/{id}")
//!     .add_param("id", ParamValue::with_style(123, ParamStyle::Matrix));
//! ```
//!
//! ### Query Parameter Styles
//!
//! ```rust
//! use clawspec_core::{CallQuery, ParamValue, ParamStyle};
//!
//! let tags = vec!["rust", "web", "api"];
//!
//! // Form (default): ?tags=rust&tags=web&tags=api
//! let query = CallQuery::new()
//!     .add_param("tags", ParamValue::new(tags.clone()));
//!
//! // Space delimited: ?tags=rust%20web%20api
//! let query = CallQuery::new()
//!     .add_param("tags", ParamValue::with_style(tags.clone(), ParamStyle::SpaceDelimited));
//!
//! // Pipe delimited: ?tags=rust|web|api
//! let query = CallQuery::new()
//!     .add_param("tags", ParamValue::with_style(tags, ParamStyle::PipeDelimited));
//! ```
//!
//! ### Deep Object Style
//!
//! For nested objects in query parameters:
//!
//! ```rust
//! use clawspec_core::{CallQuery, ParamValue, ParamStyle};
//!
//! // Deep object: ?filter[status]=active&filter[type]=premium
//! let filter = serde_json::json!({
//!     "status": "active",
//!     "type": "premium"
//! });
//!
//! let query = CallQuery::new()
//!     .add_param("filter", ParamValue::with_style(filter, ParamStyle::DeepObject));
//! ```
//!
//! ### Whole Query String
//!
//! Use [`with_querystring`][crate::ApiCall::with_querystring] when one type describes
//! the whole query string. The value is form-encoded: `None` fields are skipped and
//! nested objects are rejected.
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! # use serde::{Deserialize, Serialize};
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct UserList { users: Vec<String> }
//!
//! #[derive(Serialize, ToSchema)]
//! struct ListOptions {
//!     search: Option<String>,
//!     limit: Option<u32>,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Sends ?search=alice&limit=10
//! let users: UserList = client
//!     .get("/users")?
//!     .with_querystring(&ListOptions {
//!         search: Some("alice".to_string()),
//!         limit: Some(10),
//!     })?
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! The operation gets a single `querystring` parameter whose
//! `application/x-www-form-urlencoded` content references the `ListOptions` schema.
//! For an older OpenAPI output, it becomes a `query` parameter with `style: form` and
//! `explode: true`, which describes the same query string.
//!
//! A call cannot combine [`with_query`][crate::ApiCall::with_query] and `with_querystring`:
//! the request fails with [`ApiClientError::ConflictingQueryParameters`][crate::ApiClientError::ConflictingQueryParameters].
//! When calls to the same operation use both, the `querystring` parameter is kept and the
//! `query` parameters are dropped from the specification, with a warning.
//!
//! ## Alternative Content Types
//!
//! Besides JSON, you can send other content types:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! use headers::ContentType;
//! # use serde::Serialize;
//! # use utoipa::ToSchema;
//!
//! #[derive(Serialize, ToSchema)]
//! struct FormData {
//!     name: String,
//!     email: String,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Form-encoded data
//! let data = FormData {
//!     name: "Alice".to_string(),
//!     email: "alice@example.com".to_string(),
//! };
//! client.post("/form")?.form(&data)?.await?;
//!
//! // Raw bytes with custom content type
//! let xml = r#"<user><name>Alice</name></user>"#;
//! client.post("/xml")?
//!     .raw(xml.as_bytes().to_vec(), ContentType::xml())
//!     .await?;
//!
//! // Multipart form data
//! let files = vec![
//!     ("file1", r#"{"data": "content1"}"#),
//!     ("file2", r#"{"data": "content2"}"#),
//! ];
//! client.post("/upload")?.multipart(files).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - [`CallHeaders`][crate::CallHeaders] and [`CallCookies`][crate::CallCookies] add
//!   documented parameters
//! - Authentication can be set at client or request level
//! - Parameter styles control OpenAPI serialization documentation
//! - `with_querystring` describes the whole query string with one type
//! - Multiple content types are supported (JSON, form, XML, multipart)
//!
//! Next: [Chapter 5: OpenAPI Customization][super::chapter_5] - Tags, descriptions,
//! and metadata.
