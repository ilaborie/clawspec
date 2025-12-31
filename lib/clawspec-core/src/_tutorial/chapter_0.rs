//! # Chapter 0: Introduction
//!
//! Welcome to Clawspec! This chapter explains what Clawspec is and why you might want to use it.
//!
//! ## What is Clawspec?
//!
//! Clawspec is a Rust library that generates OpenAPI specifications from your HTTP client test code.
//! Instead of writing OpenAPI specifications by hand or using code annotations, you write tests
//! that exercise your API, and Clawspec captures the request/response patterns to generate
//! accurate documentation.
//!
//! ## The Test-Driven Documentation Approach
//!
//! Traditional approaches to OpenAPI documentation have drawbacks:
//!
//! | Approach | Drawback |
//! |----------|----------|
//! | Manual YAML/JSON | Tedious, error-prone, often out of sync with actual API |
//! | Code annotations | Clutters source code, still requires manual maintenance |
//! | Code generation | Generated specs may not reflect actual runtime behavior |
//!
//! Clawspec takes a different approach: **your tests become your documentation source**.
//!
//! Benefits:
//! - **Always accurate**: Documentation is generated from actual API calls
//! - **Test coverage = doc coverage**: If it's tested, it's documented
//! - **Type-safe**: Rust's type system ensures schema correctness
//! - **No code clutter**: Your production code stays clean
//!
//! ## How It Works
//!
//! 1. **Write tests** that make HTTP requests to your API
//! 2. **Use typed responses** with `#[derive(ToSchema)]` for automatic schema capture
//! 3. **Generate OpenAPI** from the collected request/response data
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │  Test Code  │ ──► │  ApiClient  │ ──► │   OpenAPI   │
//! │             │     │  (captures) │     │    Spec     │
//! └─────────────┘     └─────────────┘     └─────────────┘
//! ```
//!
//! ## Prerequisites
//!
//! To follow this tutorial, you should be familiar with:
//!
//! - Rust basics (structs, traits, async/await)
//! - HTTP concepts (methods, status codes, JSON)
//! - Basic understanding of OpenAPI (helpful but not required)
//!
//! ## Dependencies
//!
//! Add these to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! clawspec-core = "0.4"
//! serde = { version = "1", features = ["derive"] }
//! utoipa = { version = "5", features = ["preserve_order"] }
//! tokio = { version = "1", features = ["full"] }
//!
//! [dev-dependencies]
//! tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
//! ```
//!
//! ## What You'll Learn
//!
//! By the end of this tutorial, you'll know how to:
//!
//! - Create and configure an API client
//! - Make GET, POST, PUT, PATCH, and DELETE requests
//! - Use path, query, header, and cookie parameters
//! - Handle different response types and errors
//! - Customize OpenAPI output with tags and descriptions
//! - Use redaction for stable documentation examples
//! - Integrate with test frameworks using `TestClient`
//!
//! Ready to start? Continue to [Chapter 1: Getting Started][super::chapter_1]!
