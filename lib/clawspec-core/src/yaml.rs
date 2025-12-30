//! YAML serialization support using serde-saphyr.
//!
//! This module provides YAML serialization capabilities for OpenAPI specifications
//! and split results. It is only available when the `yaml` feature is enabled.
//!
//! # Example
//!
//! ```rust,ignore
//! use clawspec_core::{ApiClient, ToYaml};
//!
//! let mut client = ApiClient::builder()
//!     .with_host("api.example.com")
//!     .build()?;
//!
//! // ... make API calls ...
//!
//! let spec = client.collected_openapi().await;
//! let yaml_string = spec.to_yaml()?;
//!
//! std::fs::write("openapi.yml", yaml_string)?;
//! ```

use serde::Serialize;

/// Error type for YAML serialization operations.
pub type YamlError = serde_saphyr::ser_error::Error;

/// Extension trait for serializing types to YAML.
///
/// This trait is implemented for all types that implement [`Serialize`].
/// It provides a convenient `to_yaml()` method for generating YAML strings.
///
/// # Example
///
/// ```rust,ignore
/// use clawspec_core::ToYaml;
/// use utoipa::openapi::OpenApi;
///
/// let spec: OpenApi = /* ... */;
/// let yaml = spec.to_yaml()?;
/// println!("{yaml}");
/// ```
pub trait ToYaml: Serialize + Sized {
    /// Serializes this value to a YAML string.
    ///
    /// # Errors
    ///
    /// Returns a [`YamlError`] if serialization fails.
    fn to_yaml(&self) -> Result<String, YamlError> {
        serde_saphyr::to_string(self)
    }
}

impl<T: Serialize + Sized> ToYaml for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use utoipa::openapi::{InfoBuilder, OpenApiBuilder};

    #[test]
    fn should_serialize_openapi_to_yaml() {
        let spec = OpenApiBuilder::new()
            .info(
                InfoBuilder::new()
                    .title("Test API")
                    .version("1.0.0")
                    .build(),
            )
            .build();

        let yaml = spec.to_yaml().expect("should serialize to YAML");

        assert_snapshot!(yaml, @r"
        openapi: 3.1.0
        info:
          title: Test API
          version: 1.0.0
        paths: {}
        ");
    }

    #[test]
    fn should_serialize_simple_struct_to_yaml() {
        #[derive(Serialize)]
        struct Config {
            name: String,
            version: u32,
        }

        let config = Config {
            name: "test".to_string(),
            version: 1,
        };

        let yaml = config.to_yaml().expect("should serialize to YAML");

        assert_snapshot!(yaml, @r"
        name: test
        version: 1
        ");
    }
}
