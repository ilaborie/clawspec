//! Procedural macros for clawspec API generation.
//!
//! This crate provides procedural macros to make it easier to generate
//! OpenAPI specifications from annotated Rust code.
//!
//! # Future Plans
//!
//! This crate is intended to provide derive macros for generating API client
//! implementations from annotated traits, similar to how other HTTP client
//! libraries work.
//!
//! Example future usage:
//! ```ignore
//! use clawspec_macro::ApiClient;
//!
//! #[derive(ApiClient)]
//! #[api(base_url = "https://api.example.com")]
//! trait ExampleApi {
//!     #[get("/users/{id}")]
//!     async fn get_user(&self, id: u32) -> Result<User, Error>;
//! }
//! ```

// For now, this is a placeholder that will be expanded with actual macros
// when the API design is finalized.
