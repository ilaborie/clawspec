//! # Chapter 7: Test Integration
//!
//! This chapter covers using [`TestClient`][crate::test_client::TestClient] for
//! end-to-end testing with automatic server lifecycle management.
//!
//! ## Why TestClient?
//!
//! While [`ApiClient`][crate::ApiClient] works against any HTTP server,
//! [`TestClient`][crate::test_client::TestClient] provides:
//!
//! - **Automatic server startup** on a random port
//! - **Health checking** with exponential backoff
//! - **Automatic cleanup** when tests complete
//! - **Direct access** to all `ApiClient` methods
//!
//! ## Implementing TestServer
//!
//! First, implement the [`TestServer`][crate::test_client::TestServer] trait for your server:
//!
//! ```rust,no_run
//! use clawspec_core::test_client::{TestServer, TestServerConfig, HealthStatus};
//! use clawspec_core::ApiClient;
//! use std::net::TcpListener;
//!
//! #[derive(Debug)]
//! struct MyAppServer;
//!
//! impl TestServer for MyAppServer {
//!     type Error = std::io::Error;
//!
//!     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//!         // Convert to async listener
//!         listener.set_nonblocking(true)?;
//!         let listener = tokio::net::TcpListener::from_std(listener)?;
//!
//!         // Start your server (Axum, Actix, Warp, etc.)
//!         // my_app::run(listener).await?;
//!
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## Custom Health Checks
//!
//! Override `is_healthy` for custom health checking:
//!
//! ```rust,no_run
//! use clawspec_core::test_client::{TestServer, HealthStatus};
//! use clawspec_core::ApiClient;
//! use std::net::TcpListener;
//!
//! # #[derive(Debug)]
//! # struct MyAppServer;
//! impl TestServer for MyAppServer {
//!     type Error = std::io::Error;
//!
//!     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//!         listener.set_nonblocking(true)?;
//!         let _ = tokio::net::TcpListener::from_std(listener)?;
//!         Ok(())
//!     }
//!
//!     async fn is_healthy(&self, client: &mut ApiClient) -> Result<HealthStatus, Self::Error> {
//!         // Check your actual health endpoint
//!         match client.get("/health")
//!             .expect("valid path")
//!             .without_collection()  // Don't include in OpenAPI
//!             .await
//!         {
//!             Ok(_) => Ok(HealthStatus::Healthy),
//!             Err(_) => Ok(HealthStatus::Unhealthy),
//!         }
//!     }
//! }
//! ```
//!
//! ## Configuring the Test Server
//!
//! Use [`TestServerConfig`][crate::test_client::TestServerConfig] for customization:
//!
//! ```rust,no_run
//! use clawspec_core::test_client::{TestServer, TestServerConfig};
//! use clawspec_core::ApiClient;
//! use utoipa::openapi::{InfoBuilder, ServerBuilder};
//! use std::net::TcpListener;
//! use std::time::Duration;
//!
//! # #[derive(Debug)]
//! # struct MyAppServer;
//! impl TestServer for MyAppServer {
//!     type Error = std::io::Error;
//!
//!     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//!         listener.set_nonblocking(true)?;
//!         let _ = tokio::net::TcpListener::from_std(listener)?;
//!         Ok(())
//!     }
//!
//!     fn config(&self) -> TestServerConfig {
//!         // Configure the API client with metadata
//!         let client_builder = ApiClient::builder()
//!             .with_base_path("/api/v1").expect("valid path")
//!             .with_info(
//!                 InfoBuilder::new()
//!                     .title("My API")
//!                     .version("1.0.0")
//!                     .build()
//!             )
//!             .add_server(
//!                 ServerBuilder::new()
//!                     .url("https://api.example.com")
//!                     .description(Some("Production"))
//!                     .build()
//!             );
//!
//!         TestServerConfig {
//!             api_client: Some(client_builder),
//!             min_backoff_delay: Duration::from_millis(10),
//!             max_backoff_delay: Duration::from_secs(5),
//!             backoff_jitter: true,
//!             max_retry_attempts: 20,
//!         }
//!     }
//! }
//! ```
//!
//! ## Writing Tests
//!
//! Use [`TestClient::start`][crate::test_client::TestClient::start] in your tests:
//!
//! ```rust,no_run
//! use clawspec_core::test_client::TestClient;
//! # use clawspec_core::test_client::TestServer;
//! # use std::net::TcpListener;
//! # #[derive(Debug)]
//! # struct MyAppServer;
//! # impl TestServer for MyAppServer {
//! #     type Error = std::io::Error;
//! #     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//! #         listener.set_nonblocking(true)?;
//! #         let _ = tokio::net::TcpListener::from_std(listener)?;
//! #         Ok(())
//! #     }
//! # }
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Serialize, ToSchema)]
//! struct CreateUser { name: String }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User { id: u64, name: String }
//!
//! #[tokio::test]
//! async fn test_user_crud() -> Result<(), Box<dyn std::error::Error>> {
//!     // Start server and get client
//!     let mut client = TestClient::start(MyAppServer).await?;
//!
//!     // Create user
//!     let user: User = client.post("/users")?
//!         .json(&CreateUser { name: "Alice".to_string() })?
//!         .with_tag("users")
//!         .await?
//!         .as_json()
//!         .await?;
//!
//!     assert_eq!(user.name, "Alice");
//!
//!     // Get user
//!     let fetched: User = client.get(format!("/users/{}", user.id))?
//!         .with_tag("users")
//!         .await?
//!         .as_json()
//!         .await?;
//!
//!     assert_eq!(fetched.id, user.id);
//!
//!     Ok(())
//! }  // Server automatically stops when client is dropped
//! ```
//!
//! ## Generating OpenAPI
//!
//! Use [`write_openapi`][crate::test_client::TestClient::write_openapi] to save the spec:
//!
//! ```rust,no_run
//! use clawspec_core::test_client::TestClient;
//! # use clawspec_core::test_client::TestServer;
//! # use std::net::TcpListener;
//! # #[derive(Debug)]
//! # struct MyAppServer;
//! # impl TestServer for MyAppServer {
//! #     type Error = std::io::Error;
//! #     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//! #         listener.set_nonblocking(true)?;
//! #         let _ = tokio::net::TcpListener::from_std(listener)?;
//! #         Ok(())
//! #     }
//! # }
//!
//! #[tokio::test]
//! async fn generate_openapi() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = TestClient::start(MyAppServer).await?;
//!
//!     // Exercise all your API endpoints...
//!     client.get("/users")?.with_tag("users").await?;
//!     client.post("/users")?.with_tag("users").await?;
//!     client.get("/users/1")?.with_tag("users").await?;
//!     client.delete("/users/1")?.with_tag("users").await?;
//!
//!     // Generate OpenAPI spec (format detected by extension)
//!     client.write_openapi("docs/openapi.yml").await?;
//!     // Or JSON: client.write_openapi("docs/openapi.json").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Test Organization Pattern
//!
//! A common pattern is to have a dedicated test for OpenAPI generation:
//!
//! ```rust,no_run
//! // tests/generate_openapi.rs
//! use clawspec_core::test_client::TestClient;
//! # use clawspec_core::test_client::TestServer;
//! # use std::net::TcpListener;
//! # #[derive(Debug)]
//! # struct MyAppServer;
//! # impl TestServer for MyAppServer {
//! #     type Error = std::io::Error;
//! #     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//! #         listener.set_nonblocking(true)?;
//! #         let _ = tokio::net::TcpListener::from_std(listener)?;
//! #         Ok(())
//! #     }
//! # }
//!
//! #[tokio::test]
//! async fn generate_openapi() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = TestClient::start(MyAppServer).await?;
//!
//!     // Call helper functions that exercise different parts of the API
//!     test_users_endpoints(&mut client).await?;
//!     test_posts_endpoints(&mut client).await?;
//!     test_error_cases(&mut client).await?;
//!
//!     // Generate the spec
//!     client.write_openapi("docs/openapi.yml").await?;
//!
//!     Ok(())
//! }
//!
//! async fn test_users_endpoints(client: &mut TestClient<MyAppServer>) -> Result<(), Box<dyn std::error::Error>> {
//!     client.get("/users")?
//!         .with_tag("users")
//!         .with_description("List all users")
//!         .await?;
//!     // ... more user endpoints
//!     Ok(())
//! }
//! # async fn test_posts_endpoints(client: &mut TestClient<MyAppServer>) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
//! # async fn test_error_cases(client: &mut TestClient<MyAppServer>) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
//! ```
//!
//! ## Accessing the Underlying Client
//!
//! `TestClient` derefs to `ApiClient`, so all methods are available:
//!
//! ```rust,no_run
//! use clawspec_core::test_client::TestClient;
//! # use clawspec_core::test_client::TestServer;
//! # use std::net::TcpListener;
//! # #[derive(Debug)]
//! # struct MyAppServer;
//! # impl TestServer for MyAppServer {
//! #     type Error = std::io::Error;
//! #     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//! #         listener.set_nonblocking(true)?;
//! #         let _ = tokio::net::TcpListener::from_std(listener)?;
//! #         Ok(())
//! #     }
//! # }
//! # use serde::{Serialize, Deserialize};
//! # use utoipa::ToSchema;
//! # #[derive(Serialize, Deserialize, ToSchema)]
//! # struct MySchema { field: String }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = TestClient::start(MyAppServer).await?;
//!
//! // All ApiClient methods work directly
//! client.register_schema::<MySchema>().await;
//! let spec = client.collected_openapi().await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - Implement [`TestServer`][crate::test_client::TestServer] for your web framework
//! - Override `is_healthy()` for custom health checking
//! - Override `config()` for API metadata and timing settings
//! - Use `TestClient::start()` in tests for automatic lifecycle management
//! - Use `write_openapi()` to generate specs in YAML or JSON format
//! - Server stops automatically when `TestClient` is dropped
//!
//! ## Complete Example
//!
//! For a full working example with Axum, see the
//! [axum-example](https://github.com/ilaborie/clawspec/tree/main/examples/axum-example)
//! in the Clawspec repository.
//!
//! ---
//!
//! Congratulations! You've completed the Clawspec tutorial. You now know how to:
//!
//! - Create and configure API clients
//! - Make requests with various parameters
//! - Handle different response types
//! - Customize OpenAPI output
//! - Use redaction for stable examples
//! - Integrate with test frameworks
//!
//! For more details, explore the [API documentation][crate] or check out the
//! [GitHub repository](https://github.com/ilaborie/clawspec).
