//! # Chapter 6: Redaction
//!
//! This chapter covers the redaction feature for creating stable OpenAPI examples.
//!
//! > **Note:** This feature requires the `redaction` feature flag:
//! > ```toml
//! > clawspec-core = { version = "0.2", features = ["redaction"] }
//! > ```
//!
//! ## The Problem with Dynamic Values
//!
//! When generating OpenAPI examples from real API responses, dynamic values like
//! UUIDs, timestamps, and tokens change with every test run:
//!
//! ```json
//! {
//!   "id": "550e8400-e29b-41d4-a716-446655440000",
//!   "created_at": "2024-03-15T10:30:45.123Z",
//!   "session_token": "eyJhbGciOiJIUzI1NiIs..."
//! }
//! ```
//!
//! This causes problems:
//! - **Snapshot tests fail** because examples change each run
//! - **Documentation is inconsistent** across builds
//! - **Sensitive values** might leak into docs
//!
//! ## Solution: Redaction
//!
//! Redaction lets you replace dynamic values with stable placeholders in OpenAPI
//! examples while preserving the real values for your test assertions.
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! use clawspec_core::ApiClient;
//! use serde::Deserialize;
//! use utoipa::ToSchema;
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User {
//!     id: String,
//!     name: String,
//!     created_at: String,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Use as_json_redacted instead of as_json
//! let result = client
//!     .post("/users")?
//!     .json(&serde_json::json!({"name": "Alice"}))?
//!     .await?
//!     .as_json_redacted::<User>()
//!     .await?
//!     // Replace dynamic values with stable placeholders
//!     .redact("/id", "00000000-0000-0000-0000-000000000001")?
//!     .redact("/created_at", "2024-01-01T00:00:00Z")?
//!     .finish()
//!     .await;
//!
//! // result.value has the REAL dynamic values for assertions
//! let user = result.value;
//! assert!(!user.id.is_empty());
//! assert!(!user.created_at.is_empty());
//!
//! // result.redacted has the STABLE values for OpenAPI
//! let redacted = result.redacted;
//! assert_eq!(redacted["id"], "00000000-0000-0000-0000-000000000001");
//! # Ok(())
//! # }
//! ```
//!
//! ## Redaction Operations
//!
//! ### Replace Values
//!
//! Use `redact` to substitute a value:
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Response { token: String, timestamp: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result = client.post("/auth")?
//!     .json(&serde_json::json!({"user": "alice"}))?
//!     .await?
//!     .as_json_redacted::<Response>()
//!     .await?
//!     .redact("/token", "[REDACTED]")?
//!     .redact("/timestamp", "2024-01-01T00:00:00Z")?
//!     .finish()
//!     .await;
//! # Ok(())
//! # }
//! ```
//!
//! ### Remove Values
//!
//! Use `redact_remove` to exclude a field entirely:
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Response { public_id: String, internal_ref: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result = client.get("/data")?
//!     .await?
//!     .as_json_redacted::<Response>()
//!     .await?
//!     .redact("/public_id", "id-001")?
//!     .redact_remove("/internal_ref")?  // Completely remove from example
//!     .finish()
//!     .await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Path Syntax
//!
//! Paths are auto-detected based on their prefix:
//! - Paths starting with `/` use JSON Pointer (RFC 6901) - exact paths only
//! - Paths starting with `$` use JSONPath (RFC 9535) - supports wildcards
//!
//! ## JSON Pointer Syntax
//!
//! [JSON Pointer (RFC 6901)](https://tools.ietf.org/html/rfc6901) uses `/`
//! as a path separator for exact paths:
//!
//! | Pointer | Description |
//! |---------|-------------|
//! | `/id` | Top-level field "id" |
//! | `/user/name` | Nested field "name" inside "user" |
//! | `/items/0` | First element of "items" array |
//! | `/items/0/id` | "id" of first element in "items" |
//! | `/foo~1bar` | Field named "foo/bar" (`/` escaped as `~1`) |
//! | `/foo~0bar` | Field named "foo~bar" (`~` escaped as `~0`) |
//!
//! ### Nested Object Example
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! #[derive(Deserialize, ToSchema)]
//! struct Order {
//!     id: String,
//!     customer: Customer,
//!     items: Vec<Item>,
//! }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct Customer {
//!     id: String,
//!     email: String,
//! }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct Item {
//!     sku: String,
//!     quantity: u32,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result = client.get("/orders/123")?
//!     .await?
//!     .as_json_redacted::<Order>()
//!     .await?
//!     .redact("/id", "order-001")?
//!     .redact("/customer/id", "customer-001")?
//!     .redact("/customer/email", "user@example.com")?
//!     .redact("/items/0/sku", "SKU-001")?
//!     .finish()
//!     .await;
//! # Ok(())
//! # }
//! ```
//!
//! ## JSONPath Wildcards
//!
//! For arrays or deeply nested structures, use [JSONPath (RFC 9535)](https://www.rfc-editor.org/rfc/rfc9535)
//! syntax which starts with `$`:
//!
//! | JSONPath | Description |
//! |----------|-------------|
//! | `$[*].id` | All `id` fields in root array |
//! | `$.items[*].id` | All `id` fields in `items` array |
//! | `$..id` | All `id` fields anywhere (recursive descent) |
//! | `$[0:3]` | First 3 elements of root array |
//!
//! ### Array Redaction Example
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! #[derive(Deserialize, ToSchema)]
//! struct UserList {
//!     users: Vec<User>,
//! }
//!
//! #[derive(Deserialize, ToSchema)]
//! struct User {
//!     id: String,
//!     name: String,
//!     created_at: String,
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result = client.get("/users")?
//!     .await?
//!     .as_json_redacted::<UserList>()
//!     .await?
//!     // Redact ALL user IDs with a single call
//!     .redact("$.users[*].id", "stable-user-id")?
//!     // Redact ALL timestamps
//!     .redact("$.users[*].created_at", "2024-01-01T00:00:00Z")?
//!     .finish()
//!     .await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Function-Based Redaction
//!
//! For dynamic transformations, pass a closure instead of a static value.
//! The closure receives the concrete JSON Pointer path and current value:
//!
//! ### Index-Aware IDs
//!
//! Create stable, distinguishable IDs based on array position:
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use serde_json::Value;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct UserList { users: Vec<User> }
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: String, name: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result = client.get("/users")?
//!     .await?
//!     .as_json_redacted::<UserList>()
//!     .await?
//!     // Closure receives path like "/users/0/id", "/users/1/id", etc.
//!     .redact("$.users[*].id", |path: &str, _val: &Value| {
//!         // Extract index from path: "/users/0/id" -> "0"
//!         let idx = path.split('/').nth(2).unwrap_or("0");
//!         serde_json::json!(format!("user-{idx}"))
//!     })?
//!     .finish()
//!     .await;
//!
//! // Result: user-0, user-1, user-2, etc.
//! # Ok(())
//! # }
//! ```
//!
//! ### Value-Based Transformation
//!
//! Transform based on the current value (path can be ignored):
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use serde_json::Value;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Document { notes: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let result = client.get("/documents")?
//!     .await?
//!     .as_json_redacted::<Vec<Document>>()
//!     .await?
//!     // Redact long notes, keep short ones
//!     .redact("$[*].notes", |_path: &str, val: &Value| {
//!         if val.as_str().map(|s| s.len() > 50).unwrap_or(false) {
//!             serde_json::json!("[REDACTED - TOO LONG]")
//!         } else {
//!             val.clone()
//!         }
//!     })?
//!     .finish()
//!     .await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Handling Optional Fields
//!
//! By default, `redact` returns an error if the path matches nothing.
//! Use `RedactOptions` to allow empty matches for optional fields:
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! use clawspec_core::RedactOptions;
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Response { optional_field: Option<String> }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//!
//! let options = RedactOptions { allow_empty_match: true };
//!
//! let result = client.get("/data")?
//!     .await?
//!     .as_json_redacted::<Response>()
//!     .await?
//!     // Won't error if the path doesn't exist
//!     .redact_with_options("$.optional_field", "redacted", options)?
//!     .finish()
//!     .await;
//! # Ok(())
//! # }
//! ```
//!
//! ## The RedactedResult
//!
//! The `finish()` method returns a `RedactedResult`:
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Debug, Deserialize, ToSchema)]
//! # struct User { id: String, name: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! # let result = client.get("/users/1")?.await?.as_json_redacted::<User>().await?
//! #     .redact("/id", "user-001")?.finish().await;
//! // result.value: The deserialized struct with REAL values
//! let user: User = result.value;
//! println!("Real ID: {}", user.id);  // e.g., "550e8400-e29b-..."
//!
//! // result.redacted: JSON with STABLE values (used in OpenAPI)
//! let json: serde_json::Value = result.redacted;
//! println!("Redacted: {}", json["id"]);  // "user-001"
//! # Ok(())
//! # }
//! ```
//!
//! ## Common Patterns
//!
//! ### UUIDs
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Entity { id: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! # let builder = client.get("/test")?.await?.as_json_redacted::<Entity>().await?;
//! // Use a recognizable placeholder format
//! builder.redact("/id", "00000000-0000-0000-0000-000000000001")?
//! # .finish().await;
//! # Ok(())
//! # }
//! ```
//!
//! ### Timestamps
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Entity { created_at: String, updated_at: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! # let builder = client.get("/test")?.await?.as_json_redacted::<Entity>().await?;
//! // Use ISO 8601 format with a memorable date
//! builder
//!     .redact("/created_at", "2024-01-01T00:00:00Z")?
//!     .redact("/updated_at", "2024-01-01T12:00:00Z")?
//! # .finish().await;
//! # Ok(())
//! # }
//! ```
//!
//! ### Tokens and Secrets
//!
#![cfg_attr(feature = "redaction", doc = "```rust,no_run")]
#![cfg_attr(not(feature = "redaction"), doc = "```rust,ignore")]
//! # use clawspec_core::ApiClient;
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct AuthResponse { access_token: String, refresh_token: String }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! # let builder = client.get("/test")?.await?.as_json_redacted::<AuthResponse>().await?;
//! // Use descriptive placeholders
//! builder
//!     .redact("/access_token", "[ACCESS_TOKEN]")?
//!     .redact("/refresh_token", "[REFRESH_TOKEN]")?
//! # .finish().await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - Enable with `features = ["redaction"]` in Cargo.toml
//! - Use `as_json_redacted()` instead of `as_json()`
//! - Paths are auto-detected:
//!   - `/...` - JSON Pointer (RFC 6901) for exact paths
//!   - `$...` - JSONPath (RFC 9535) for wildcards
//! - Redactors can be:
//!   - Static values: `"stable-value"`
//!   - Closures: `|path, val| serde_json::json!(...)`
//! - `redact()` substitutes values, `redact_remove()` deletes them
//! - `redact_with_options()` allows empty matches for optional fields
//! - `finish()` returns both real values (for tests) and redacted values (for docs)
//!
//! Next: [Chapter 7: Test Integration][super::chapter_7] - Using TestClient for
//! end-to-end testing.
