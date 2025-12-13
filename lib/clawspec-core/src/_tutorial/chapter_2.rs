//! # Chapter 2: Request Building
//!
//! This chapter covers building more complex requests with POST, path parameters,
//! and query parameters.
//!
//! ## POST Requests with JSON Body
//!
//! To send JSON data, use the `.json()` method. Your request type needs
//! [`Serialize`][serde::Serialize] and [`ToSchema`][utoipa::ToSchema]:
//!
//! ```rust,no_run
//! use clawspec_core::ApiClient;
//! use serde::{Deserialize, Serialize};
//! use utoipa::ToSchema;
//!
//! #[derive(Serialize, ToSchema)]
//! struct CreateUser {
//!     name: String,
//!     email: String,
//! }
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
//! let new_user = CreateUser {
//!     name: "Alice".to_string(),
//!     email: "alice@example.com".to_string(),
//! };
//!
//! let created: User = client
//!     .post("/users")?
//!     .json(&new_user)?     // Serialize as JSON
//!     .await?
//!     .as_json()
//!     .await?;
//!
//! println!("Created user with ID: {}", created.id);
//! # Ok(())
//! # }
//! ```
//!
//! Both request and response schemas are captured automatically.
//!
//! ## Path Parameters
//!
//! Use [`CallPath`][crate::CallPath] for templated paths with parameters:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallPath, ParamValue};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: u64 }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! // Define path with parameter placeholder
//! let path = CallPath::from("/users/{user_id}")
//!     .add_param("user_id", ParamValue::new(123));
//!
//! let user: User = client
//!     .get(path)?
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! The generated OpenAPI will show `/users/{user_id}` with the parameter documented.
//!
//! ## Query Parameters
//!
//! Use [`CallQuery`][crate::CallQuery] for query string parameters:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallQuery};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct UserList { users: Vec<String> }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let query = CallQuery::new()
//!     .add_param("page", 1)
//!     .add_param("limit", 20)
//!     .add_param("sort", "name");
//!
//! let users: UserList = client
//!     .get("/users")?
//!     .with_query(query)
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! This generates a request to `/users?page=1&limit=20&sort=name`.
//!
//! ## Combining Path and Query Parameters
//!
//! You can use both together:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallPath, CallQuery, ParamValue};
//! # use serde::Deserialize;
//! # use utoipa::ToSchema;
//! # #[derive(Deserialize, ToSchema)]
//! # struct Posts { posts: Vec<String> }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let path = CallPath::from("/users/{user_id}/posts")
//!     .add_param("user_id", ParamValue::new(42));
//!
//! let query = CallQuery::new()
//!     .add_param("status", "published")
//!     .add_param("limit", 10);
//!
//! let posts: Posts = client
//!     .get(path)?
//!     .with_query(query)
//!     .await?
//!     .as_json()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Other HTTP Methods
//!
//! All standard HTTP methods are supported:
//!
//! ```rust,no_run
//! use clawspec_core::{ApiClient, CallPath, ParamValue};
//! # use serde::{Serialize, Deserialize};
//! # use utoipa::ToSchema;
//! # #[derive(Serialize, ToSchema)]
//! # struct UpdateUser { name: String }
//! # #[derive(Serialize, ToSchema)]
//! # struct PatchUser { name: Option<String> }
//! # #[derive(Deserialize, ToSchema)]
//! # struct User { id: u64 }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut client = ApiClient::builder().build()?;
//! let path = CallPath::from("/users/{id}").add_param("id", ParamValue::new(1));
//!
//! // PUT - Full replacement
//! client.put(path.clone())?
//!     .json(&UpdateUser { name: "Bob".to_string() })?
//!     .await?
//!     .as_empty()
//!     .await?;
//!
//! // PATCH - Partial update
//! let updated: User = client.patch(path.clone())?
//!     .json(&PatchUser { name: Some("Robert".to_string()) })?
//!     .await?
//!     .as_json()
//!     .await?;
//!
//! // DELETE
//! client.delete(path)?
//!     .await?
//!     .as_empty()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Key Points
//!
//! - Use `.json(&data)?` to send JSON request bodies
//! - [`CallPath`][crate::CallPath] handles path parameters like `/users/{id}`
//! - [`CallQuery`][crate::CallQuery] handles query parameters
//! - All HTTP methods are available: `get`, `post`, `put`, `patch`, `delete`
//!
//! Next: [Chapter 3: Response Handling][super::chapter_3] - Learn about different
//! response handling strategies.
