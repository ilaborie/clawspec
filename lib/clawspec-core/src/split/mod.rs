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
//! ```rust,ignore
//! use clawspec_core::split::{OpenApiSplitter, SplitSchemasByTag};
//! use std::path::PathBuf;
//!
//! let spec: OpenApi = /* your collected OpenAPI spec */;
//!
//! // Split schemas by their tag usage
//! let splitter = SplitSchemasByTag::new(PathBuf::from("common-types.yaml"));
//! let result = splitter.split(spec);
//!
//! // Write the main spec and fragments to files
//! for fragment in result.fragments {
//!     let content = serde_yaml::to_string(&fragment.content)?;
//!     std::fs::write(&fragment.path, content)?;
//! }
//!
//! let main_content = serde_yaml::to_string(&result.main)?;
//! std::fs::write("openapi.yaml", main_content)?;
//! ```

mod fragment;
mod splitter;
mod strategies;

pub use fragment::{Fragment, SplitResult};
pub use splitter::{OpenApiSplitExt, OpenApiSplitter};
pub use strategies::{ExtractSchemasByPredicate, SplitSchemasByTag};
