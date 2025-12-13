//! # Tutorial: Getting Started with Clawspec
//!
//! Welcome to the Clawspec tutorial! This guide will walk you through using Clawspec
//! to generate OpenAPI specifications from your test code.
//!
//! ## Learning Path
//!
//! This tutorial is organized into chapters that build upon each other:
//!
//! 1. **[Introduction][chapter_0]** - What is Clawspec and why use it
//! 2. **[Getting Started][chapter_1]** - Setting up the client and making your first request
//! 3. **[Request Building][chapter_2]** - POST requests, path and query parameters
//! 4. **[Response Handling][chapter_3]** - Processing responses and error handling
//! 5. **[Advanced Parameters][chapter_4]** - Headers, cookies, and parameter styles
//! 6. **[OpenAPI Customization][chapter_5]** - Tags, descriptions, and metadata
//! 7. **[Redaction][chapter_6]** - Stable examples with dynamic value redaction
//! 8. **[Test Integration][chapter_7]** - Using TestClient for end-to-end testing
//!
//! ## Quick Example
//!
//! Here's a taste of what you'll learn:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Serialize, ToSchema)]
//! struct CreateUser { name: String }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User { id: u64, name: String }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client
//! let mut client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .build()?;
//!
//! // Make a request - schema is captured automatically
//! let user: User = client
//!     .post("/users")?
//!     .json(&CreateUser { name: "Alice".to_string() })?
//!     .await?
//!     .as_json()
//!     .await?;
//!
//! // Generate OpenAPI specification
//! let spec = client.collected_openapi().await;
//! println!("{}", spec.to_pretty_json()?);
//! # Ok(())
//! # }
//! ```
//!
//! Ready to start? Head to [Chapter 0: Introduction][chapter_0]!

pub mod chapter_0;
pub mod chapter_1;
pub mod chapter_2;
pub mod chapter_3;
pub mod chapter_4;
pub mod chapter_5;
pub mod chapter_6;
pub mod chapter_7;
