//! # Chapter 5: OpenAPI Customization
//!
//! This chapter covers how to customize the generated OpenAPI specification with
//! tags, descriptions, and metadata.
//!
//! ## Adding Operation Tags
//!
//! Tags help organize operations in the generated documentation:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Single tag
//! client.get("/users")?
//!     .with_tag("users")
//!     .await?;
//!
//! // Multiple tags
//! client.post("/admin/users")?
//!     .with_tags(["users", "admin"])
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! Tags appear in the OpenAPI spec and are used by documentation tools to group
//! related endpoints.
//!
//! ## Operation Descriptions
//!
//! Add descriptions to document what operations do:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! client.get("/users")?
//!     .with_tag("users")
//!     .with_description("List all users with optional pagination")
//!     .await?;
//!
//! client.post("/users")?
//!     .with_tag("users")
//!     .with_description("Create a new user account")
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Response Descriptions
//!
//! Document what responses mean:
//!
//! ```rust,no_run
//! # use clawspec_core::ApiClient;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! client.get("/users/123")?
//!     .with_response_description("User details or 404 if not found")
//!     .await?;
//!
//! client.post("/users")?
//!     .with_response_description("The newly created user with generated ID")
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## API Info Configuration
//!
//! Configure the API's metadata when building the client:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! use utoipa::openapi::{ContactBuilder, InfoBuilder, LicenseBuilder};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let info = InfoBuilder::new()
//!     .title("My API")
//!     .version("1.0.0")
//!     .description(Some("A comprehensive REST API for managing resources"))
//!     .contact(Some(
//!         ContactBuilder::new()
//!             .name(Some("API Support"))
//!             .email(Some("support@example.com"))
//!             .url(Some("https://example.com/support"))
//!             .build(),
//!     ))
//!     .license(Some(
//!         LicenseBuilder::new()
//!             .name("MIT")
//!             .url(Some("https://opensource.org/licenses/MIT"))
//!             .build(),
//!     ))
//!     .build();
//!
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_info(info)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Server Configuration
//!
//! Define servers in the OpenAPI spec:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! use utoipa::openapi::ServerBuilder;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .add_server(
//!         ServerBuilder::new()
//!             .url("https://api.example.com")
//!             .description(Some("Production server"))
//!             .build(),
//!     )
//!     .add_server(
//!         ServerBuilder::new()
//!             .url("https://staging-api.example.com")
//!             .description(Some("Staging server"))
//!             .build(),
//!     )
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Manual Schema Registration
//!
//! Sometimes you need to register schemas that aren't automatically captured:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, register_schemas};
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Serialize, Deserialize, ToSchema)]
//! struct Address {
//!     street: String,
//!     city: String,
//!     country: String,
//! }
//!
//! #[derive(Serialize, Deserialize, ToSchema)]
//! struct User {
//!     id: u64,
//!     name: String,
//!     address: Address,  // Nested schema
//! }
//!
//! #[derive(Serialize, Deserialize, ToSchema)]
//! struct ApiError {
//!     code: String,
//!     message: String,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = ApiClient::builder().build()?;
//!
//! // Register nested schemas and error types
//! register_schemas!(client, User, Address, ApiError).await;
//! # Ok(())
//! # }
//! ```
//!
//! This is particularly useful for:
//! - Nested schemas that might not be fully resolved
//! - Error response types
//! - Schemas used in headers or other non-body locations
//!
//! ## Combining Everything
//!
//! Here's a complete example with all customizations:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, register_schemas};
//! use utoipa::openapi::{ContactBuilder, InfoBuilder, ServerBuilder};
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Serialize, ToSchema)]
//! struct CreateUser { name: String }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User { id: u64, name: String }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct ApiError { code: String, message: String }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure client with full metadata
//! let info = InfoBuilder::new()
//!     .title("User Management API")
//!     .version("2.0.0")
//!     .description(Some("API for managing user accounts"))
//!     .build();
//!
//! let mut client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .with_info(info)
//!     .add_server(
//!         ServerBuilder::new()
//!             .url("https://api.example.com/v2")
//!             .description(Some("Production"))
//!             .build(),
//!     )
//!     .build()?;
//!
//! // Register error schema
//! register_schemas!(client, ApiError).await;
//!
//! // Make requests with full documentation
//! let user: User = client.post("/users")?
//!     .json(&CreateUser { name: "Alice".to_string() })?
//!     .with_tag("users")
//!     .with_description("Create a new user account")
//!     .with_response_description("The created user with assigned ID")
//!     .await?
//!     .as_json()
//!     .await?;
//!
//! // Generate the OpenAPI spec
//! let spec = client.collected_openapi().await;
//! println!("{}", spec.to_pretty_json()?);
//!
//! // Or output as YAML (requires "yaml" feature, see Chapter 1)
//! // use clawspec_core::ToYaml;
//! // println!("{}", spec.to_yaml()?);
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - Use `.with_tag()` and `.with_tags()` to organize operations
//! - Use `.with_description()` to document operations
//! - Configure API info and servers at the client builder level
//! - Use `register_schemas!` for nested or error schemas
//! - For YAML output, enable the `yaml` feature (see [Chapter 1][super::chapter_1])
//!
//! Next: [Chapter 6: Redaction][super::chapter_6] - Creating stable examples with
//! dynamic value redaction.
