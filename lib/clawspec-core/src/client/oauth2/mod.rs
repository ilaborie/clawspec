//! OAuth2 authentication support for API testing.
//!
//! This module provides runtime OAuth2 authentication, enabling API testing
//! with OAuth2-protected endpoints. It integrates with the `oauth2` crate for
//! RFC-compliant token acquisition and management.
//!
//! # Feature Flag
//!
//! This module is only available when the `oauth2` feature is enabled:
//!
//! ```toml
//! [dependencies]
//! clawspec-core = { version = "...", features = ["oauth2"] }
//! ```
//!
//! # Supported Flows
//!
//! - **Client Credentials**: Machine-to-machine authentication (most common for testing)
//! - **Pre-Acquired Token**: Use externally obtained tokens (environment variables, etc.)
//!
//! # Example
//!
//! ```rust,ignore
//! use clawspec_core::{ApiClient, OAuth2Config};
//!
//! let oauth2 = OAuth2Config::client_credentials(
//!     "client-id",
//!     "client-secret",
//!     "https://auth.example.com/token",
//! )
//! .add_scope("read:users")
//! .build();
//!
//! let client = ApiClient::builder()
//!     .with_oauth2(oauth2)
//!     .build()?;
//!
//! // Token acquired automatically on first request
//! client.get("/users")?.await?.as_json::<Vec<User>>().await?;
//! ```

mod config;
mod error;
mod provider;
mod token;

pub use self::config::{OAuth2Config, OAuth2ConfigBuilder, SharedOAuth2Config};
pub use self::error::OAuth2Error;
pub use self::token::OAuth2Token;
