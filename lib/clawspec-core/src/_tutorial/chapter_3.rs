//! # Chapter 3: Response Handling
//!
//! This chapter covers the various ways to handle API responses, including
//! error handling patterns.
//!
//! ## Response Methods Overview
//!
//! After sending a request, you have several options for handling the response:
//!
//! | Method | Returns | Use Case |
//! |--------|---------|----------|
//! | `as_json::<T>()` | `T` | Standard JSON response |
//! | `as_optional_json::<T>()` | `Option<T>` | Resource that may not exist (404 → None) |
//! | `as_result_json::<T, E>()` | `Result<T, E>` | API with typed error responses |
//! | `as_result_option_json::<T, E>()` | `Result<Option<T>, E>` | Combined: 404 → Ok(None), errors → Err |
//! | `as_raw()` | `RawResult` | Access status code and raw body |
//! | `as_empty()` | `()` | Responses with no body (204, etc.) |
//! | `as_text()` | `String` | Plain text responses |
//!
//! ## Standard JSON Response
//!
//! The most common pattern - parse JSON and fail on errors:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: u64 }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let user: User = client
//!     .get("/users/123")?
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Optional JSON (404 as None)
//!
//! Use `as_optional_json()` when a resource might not exist:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: u64, name: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let user: Option<User> = client
//!     .get("/users/999")?
//!     .add_expected_status(404)  // Tell client 404 is expected
//!     .await?
//!     .as_optional_json()
//!     .await?;
//!
//! match user {
//!     Some(u) => println!("Found: {}", u.name),
//!     None => println!("User not found"),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Result JSON (Typed Errors)
//!
//! When your API returns structured error responses:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Debug, Deserialize, ToSchema)]
//! # struct User { id: u64 }
//! #[derive(Debug, Deserialize, ToSchema)]
//! struct ApiError {
//!     code: String,
//!     message: String,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result: Result<User, ApiError> = client
//!     .get("/users/123")?
//!     .add_expected_status(404)
//!     .await?
//!     .as_result_json()
//!     .await?;
//!
//! match result {
//!     Ok(user) => println!("Got user: {:?}", user),
//!     Err(error) => println!("API error: {} - {}", error.code, error.message),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Result Option JSON (404 as Ok(None))
//!
//! Combines optional resources with typed errors:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Debug, Deserialize, ToSchema)]
//! # struct User { id: u64, name: String }
//! # #[derive(Debug, Deserialize, ToSchema)]
//! # struct ApiError { message: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // 2xx → Ok(Some(T))
//! // 404 → Ok(None)
//! // Other 4xx/5xx → Err(E)
//! let result: Result<Option<User>, ApiError> = client
//!     .get("/users/maybe-exists")?
//!     .add_expected_status(404)
//!     .await?
//!     .as_result_option_json()
//!     .await?;
//!
//! match result {
//!     Ok(Some(user)) => println!("Found: {}", user.name),
//!     Ok(None) => println!("Not found (but not an error)"),
//!     Err(e) => println!("Actual error: {}", e.message),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Expected Status Codes
//!
//! By default, Clawspec expects 2xx-4xx status codes. Use these methods to
//! customize expectations:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, expected_status_codes};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: u64 }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Add a single expected status
//! client.get("/users/123")?
//!     .add_expected_status(404)
//!     .await?;
//!
//! // Use specific status code
//! client.post("/users")?
//!     .with_expected_status(201)
//!     .await?;
//!
//! // Use the macro for complex patterns
//! client.get("/resource")?
//!     .with_expected_status_codes(expected_status_codes!(200, 201, 204))
//!     .await?;
//!
//! // Ranges are supported
//! client.get("/resource")?
//!     .with_expected_status_codes(expected_status_codes!(200-299, 404))
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Raw Response Access
//!
//! When you need full control over the response:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = ApiClient::builder().build()?;
//! let raw = client
//!     .get("/health")?
//!     .await?
//!     .as_raw()
//!     .await?;
//!
//! println!("Status: {}", raw.status_code());
//! println!("Body: {:?}", raw.text());
//!
//! // Access as bytes
//! let bytes: Option<&[u8]> = raw.bytes();
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling
//!
//! Clawspec uses [`ApiClientError`][crate::ApiClientError] for client-level errors:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, ApiClientError};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = ApiClient::builder().build()?;
//! match client.get("/users/123")?.with_expected_status(200).await {
//!     Ok(response) => {
//!         // Handle success
//!     }
//!     Err(ApiClientError::UnexpectedStatusCode { status_code, body }) => {
//!         println!("Got status {}: {}", status_code, body);
//!     }
//!     Err(e) => {
//!         println!("Other error: {}", e);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - Choose the response method that matches your API's behavior
//! - Use `add_expected_status()` to tell Clawspec about expected non-2xx codes
//! - `as_optional_json()` is great for "get or not found" patterns
//! - `as_result_json()` captures typed error schemas in OpenAPI
//!
//! Next: [Chapter 4: Advanced Parameters][super::chapter_4] - Headers, cookies,
//! and parameter styles.
