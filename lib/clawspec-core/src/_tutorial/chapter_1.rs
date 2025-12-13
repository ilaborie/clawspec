//! # Chapter 1: Getting Started
//!
//! In this chapter, you'll learn how to create an API client and make your first request.
//!
//! ## Creating the Client
//!
//! The [`ApiClient`][crate::ApiClient] is your main entry point for making requests.
//! Use the builder pattern to configure it:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Minimal client (connects to localhost:80)
//! let client = ApiClient::builder().build()?;
//!
//! // Client with custom host
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .build()?;
//!
//! // Client with host and port
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_port(8080)
//!     .build()?;
//!
//! // Client with base path (all requests will be prefixed)
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_base_path("/api/v1")?
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Your First GET Request
//!
//! Let's make a simple GET request. You'll need a response type that implements
//! [`Deserialize`][serde::Deserialize] and [`ToSchema`][utoipa::ToSchema]:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! use serde::Deserialize;
//! use utoipa::ToSchema;
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User {
//!     id: u64,
//!     name: String,
//!     email: String,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .build()?;
//!
//! // Make a GET request
//! let user: User = client
//!     .get("/users/123")?   // Create the request
//!     .await?               // Send it (using IntoFuture)
//!     .as_json()            // Parse response as JSON
//!     .await?;
//!
//! println!("Got user: {} ({})", user.name, user.email);
//! # Ok(())
//! # }
//! ```
//!
//! ## Understanding the Flow
//!
//! Let's break down what happens:
//!
//! 1. **`client.get("/users/123")?`** - Creates an [`ApiCall`][crate::ApiCall] builder
//! 2. **`.await?`** - Sends the request (via [`IntoFuture`])
//! 3. **`.as_json().await?`** - Parses the response body as JSON
//!
//! The schema for `User` is automatically captured when you call `.as_json()`.
//!
//! ## Generating the OpenAPI Spec
//!
//! After making requests, you can generate the OpenAPI specification:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ApiClient::builder().build()?;
//!
//! // ... make some requests ...
//!
//! // Get the collected OpenAPI spec
//! let openapi = client.collected_openapi().await;
//!
//! // Output as YAML
//! println!("{}", openapi.to_yaml()?);
//!
//! // Or as JSON
//! println!("{}", openapi.to_pretty_json()?);
//! # Ok(())
//! # }
//! ```
//!
//! ## Response Without Parsing
//!
//! Sometimes you don't need to parse the response body:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Get raw response details
//! let raw = client.get("/health")?.await?.as_raw().await?;
//! println!("Status: {}", raw.status_code());
//! println!("Body: {:?}", raw.text());
//!
//! // Or just consume the response without reading the body
//! client.get("/ping")?.await?.as_empty().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - Use [`ApiClient::builder()`][crate::ApiClient::builder] to create clients
//! - Response types need `#[derive(Deserialize, ToSchema)]`
//! - Call `.as_json()` to parse responses and capture schemas
//! - Use `collected_openapi()` to get the generated spec
//!
//! Next: [Chapter 2: Request Building][super::chapter_2] - Learn about POST requests and parameters.
