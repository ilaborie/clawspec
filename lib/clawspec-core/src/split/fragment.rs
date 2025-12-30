//! Fragment types for split OpenAPI specifications.

use std::path::PathBuf;

use serde::Serialize;
use utoipa::openapi::OpenApi;

/// A fragment extracted from an OpenAPI specification.
///
/// Represents a piece of the original specification that should be written to a separate file.
/// The content can be any serializable type, typically [`Components`](utoipa::openapi::Components),
/// [`OpenApi`], or a custom subset of schemas.
///
/// # Type Parameters
///
/// * `T` - The type of content in this fragment. Must implement [`Serialize`] for file output.
///
/// # Example
///
/// ```rust,ignore
/// use clawspec_core::split::Fragment;
/// use std::path::PathBuf;
/// use utoipa::openapi::Components;
///
/// let fragment = Fragment {
///     path: PathBuf::from("schemas/common.yaml"),
///     content: Components::new(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct Fragment<T: Serialize> {
    /// Relative path where this fragment should be written.
    ///
    /// This path is relative to the main OpenAPI specification file.
    /// The main spec will use `$ref` pointing to this path.
    pub path: PathBuf,

    /// The content to serialize into the fragment file.
    pub content: T,
}

impl<T: Serialize> Fragment<T> {
    /// Creates a new fragment with the given path and content.
    pub fn new(path: impl Into<PathBuf>, content: T) -> Self {
        Self {
            path: path.into(),
            content,
        }
    }

    /// Serializes the fragment content to a YAML string.
    ///
    /// *Requires the `yaml` feature.*
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use clawspec_core::split::Fragment;
    ///
    /// let fragment = Fragment::new("common.yaml", components);
    /// let yaml = fragment.to_yaml()?;
    /// std::fs::write(&fragment.path, yaml)?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`YamlError`](crate::YamlError) if serialization fails.
    #[cfg(feature = "yaml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "yaml")))]
    pub fn to_yaml(&self) -> Result<String, crate::YamlError> {
        crate::ToYaml::to_yaml(&self.content)
    }
}

/// The result of splitting an OpenAPI specification.
///
/// Contains the main specification (with `$ref` references to external files)
/// and a collection of fragments to be written to separate files.
///
/// # Type Parameters
///
/// * `T` - The type of content in the fragments. Must implement [`Serialize`].
///
/// # Example
///
/// ```rust,ignore
/// use clawspec_core::split::{OpenApiSplitter, SplitSchemasByTag, SplitResult};
///
/// let splitter = SplitSchemasByTag::new("common.yaml");
/// let result: SplitResult<_> = splitter.split(spec);
///
/// // Write fragments to files
/// for fragment in &result.fragments {
///     let yaml = serde_yaml::to_string(&fragment.content)?;
///     std::fs::write(&fragment.path, yaml)?;
/// }
///
/// // Write main spec
/// let main_yaml = serde_yaml::to_string(&result.main)?;
/// std::fs::write("openapi.yaml", main_yaml)?;
/// ```
#[derive(Debug, Clone)]
pub struct SplitResult<T: Serialize> {
    /// The main OpenAPI specification with `$ref` references to extracted fragments.
    pub main: OpenApi,

    /// Extracted fragments to be written to separate files.
    pub fragments: Vec<Fragment<T>>,
}

impl<T: Serialize> SplitResult<T> {
    /// Creates a new split result with no fragments.
    pub fn new(main: OpenApi) -> Self {
        Self {
            main,
            fragments: Vec::new(),
        }
    }

    /// Adds a fragment to the result.
    pub fn add_fragment(&mut self, fragment: Fragment<T>) {
        self.fragments.push(fragment);
    }

    /// Returns `true` if there are no fragments (no splitting occurred).
    pub fn is_unsplit(&self) -> bool {
        self.fragments.is_empty()
    }

    /// Returns the number of fragments.
    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }

    /// Serializes the main OpenAPI specification to a YAML string.
    ///
    /// *Requires the `yaml` feature.*
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use clawspec_core::split::{OpenApiSplitter, SplitSchemasByTag};
    ///
    /// let result = splitter.split(spec);
    /// let main_yaml = result.main_to_yaml()?;
    /// std::fs::write("openapi.yaml", main_yaml)?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`YamlError`](crate::YamlError) if serialization fails.
    #[cfg(feature = "yaml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "yaml")))]
    pub fn main_to_yaml(&self) -> Result<String, crate::YamlError> {
        crate::ToYaml::to_yaml(&self.main)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::openapi::{Components, OpenApiBuilder};

    #[test]
    fn should_create_fragment() {
        let components = Components::new();
        let fragment = Fragment::new("schemas/common.yaml", components);

        assert_eq!(fragment.path, PathBuf::from("schemas/common.yaml"));
    }

    #[test]
    fn should_create_split_result() {
        let spec = OpenApiBuilder::new().build();
        let result: SplitResult<Components> = SplitResult::new(spec);

        assert!(result.is_unsplit());
        assert_eq!(result.fragment_count(), 0);
    }

    #[test]
    fn should_add_fragments() {
        let spec = OpenApiBuilder::new().build();
        let mut result: SplitResult<Components> = SplitResult::new(spec);

        result.add_fragment(Fragment::new("common.yaml", Components::new()));
        result.add_fragment(Fragment::new("errors.yaml", Components::new()));

        assert!(!result.is_unsplit());
        assert_eq!(result.fragment_count(), 2);
    }
}

#[cfg(all(test, feature = "yaml"))]
mod yaml_tests {
    use super::*;
    use utoipa::openapi::{Components, InfoBuilder, OpenApiBuilder};

    #[test]
    fn should_serialize_fragment_to_yaml() {
        let components = Components::new();
        let fragment = Fragment::new("schemas/common.yaml", components);

        let yaml = fragment.to_yaml().expect("should serialize to YAML");

        // serde_saphyr serializes empty objects without braces
        assert!(yaml.trim().is_empty() || yaml.trim() == "{}");
    }

    #[test]
    fn should_serialize_main_spec_to_yaml() {
        let spec = OpenApiBuilder::new()
            .info(
                InfoBuilder::new()
                    .title("Test API")
                    .version("1.0.0")
                    .build(),
            )
            .build();
        let result: SplitResult<Components> = SplitResult::new(spec);

        let yaml = result.main_to_yaml().expect("should serialize to YAML");

        // Verify the essential structure is present
        assert!(yaml.contains("openapi: 3.1.0"));
        assert!(yaml.contains("title: Test API"));
        assert!(yaml.contains("version: 1.0.0"));
        assert!(yaml.contains("paths"));
    }

    #[test]
    fn should_serialize_split_result_with_fragments() {
        let spec = OpenApiBuilder::new()
            .info(
                InfoBuilder::new()
                    .title("Split API")
                    .version("2.0.0")
                    .build(),
            )
            .build();
        let mut result: SplitResult<Components> = SplitResult::new(spec);
        result.add_fragment(Fragment::new("common.yaml", Components::new()));

        let main_yaml = result
            .main_to_yaml()
            .expect("should serialize main to YAML");
        let fragment_yaml = result.fragments[0]
            .to_yaml()
            .expect("should serialize fragment to YAML");

        // Verify the essential structure is present
        assert!(main_yaml.contains("openapi: 3.1.0"));
        assert!(main_yaml.contains("title: Split API"));
        assert!(main_yaml.contains("version: 2.0.0"));
        assert!(main_yaml.contains("paths"));

        // serde_saphyr serializes empty objects without braces
        assert!(fragment_yaml.trim().is_empty() || fragment_yaml.trim() == "{}");
    }
}
