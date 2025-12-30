//! OpenAPI specification splitting utilities.
//!
//! This module provides tools for splitting a single OpenAPI specification into multiple files,
//! allowing for better organization and reusability of schemas across different API specifications.
//!
//! # Overview
//!
//! When working with large APIs, it's common to want to:
//! - Extract common schemas (like error types) into a shared file
//! - Organize schemas by domain or tag
//! - Share schemas across multiple API specifications
//!
//! This module provides the [`OpenApiSplitter`] trait and several implementations for common
//! splitting strategies.
//!
//! # Example
//!
//! ## With the `yaml` feature (recommended)
//!
#![cfg_attr(feature = "yaml", doc = "```rust,ignore")]
#![cfg_attr(not(feature = "yaml"), doc = "```rust,ignore")]
//! use clawspec_core::split::{OpenApiSplitter, SplitSchemasByTag};
//! use std::path::PathBuf;
//!
//! let spec: OpenApi = /* your collected OpenAPI spec */;
//!
//! // Split schemas by their tag usage
//! let splitter = SplitSchemasByTag::new(PathBuf::from("common-types.yaml"));
//! let result = splitter.split(spec);
//!
//! // Write fragments using the convenient to_yaml() method
//! for fragment in &result.fragments {
//!     let content = fragment.to_yaml()?;
//!     std::fs::write(&fragment.path, content)?;
//! }
//!
//! // Write main spec
//! let main_content = result.main_to_yaml()?;
//! std::fs::write("openapi.yaml", main_content)?;
//! ```
//!
//! ## Using ToYaml trait directly
//!
#![cfg_attr(feature = "yaml", doc = "```rust,ignore")]
#![cfg_attr(not(feature = "yaml"), doc = "```rust,ignore")]
//! use clawspec_core::{ToYaml, split::{OpenApiSplitter, SplitSchemasByTag}};
//!
//! let result = splitter.split(spec);
//!
//! // The ToYaml trait is implemented for all Serialize types
//! let main_yaml = result.main.to_yaml()?;
//! ```

mod fragment;
mod splitter;
mod strategies;

pub use fragment::{Fragment, SplitResult};
pub use splitter::{OpenApiSplitExt, OpenApiSplitter};
pub use strategies::{ExtractSchemasByPredicate, SplitSchemasByTag};
