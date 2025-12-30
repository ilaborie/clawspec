//! The OpenApiSplitter trait for splitting OpenAPI specifications.

use serde::Serialize;
use utoipa::openapi::OpenApi;

use super::SplitResult;

/// Trait for splitting an OpenAPI specification into multiple files.
///
/// Implementations of this trait define different strategies for organizing
/// schemas and other components into separate files for better modularity.
///
/// # Type Parameters
///
/// The associated type `Fragment` defines what type of content is extracted
/// into separate files. Common choices include:
///
/// * [`Components`](utoipa::openapi::Components) - Extract just the components section
/// * [`OpenApi`] - Extract complete sub-specifications
/// * Custom types implementing [`Serialize`]
///
/// # Implementing Custom Splitters
///
/// ```rust,ignore
/// use clawspec_core::split::{OpenApiSplitter, SplitResult, Fragment};
/// use utoipa::openapi::{Components, OpenApi};
/// use std::path::PathBuf;
///
/// struct MyCustomSplitter {
///     output_dir: PathBuf,
/// }
///
/// impl OpenApiSplitter for MyCustomSplitter {
///     type Fragment = Components;
///
///     fn split(&self, spec: OpenApi) -> SplitResult<Self::Fragment> {
///         // Analyze the spec and extract schemas
///         // Update refs in main spec to point to external files
///         // Return the modified main spec and fragments
///         todo!()
///     }
/// }
/// ```
///
/// # Built-in Implementations
///
/// * [`SplitSchemasByTag`](super::SplitSchemasByTag) - Split schemas based on which tags use them
/// * [`ExtractSchemasByPredicate`](super::ExtractSchemasByPredicate) - Extract schemas matching a predicate
pub trait OpenApiSplitter {
    /// The type of content extracted into fragments.
    type Fragment: Serialize;

    /// Splits the OpenAPI specification into a main spec and fragments.
    ///
    /// This method consumes the input specification and returns:
    /// - A modified main spec with `$ref` pointing to external files
    /// - A collection of fragments to be written to separate files
    ///
    /// # Arguments
    ///
    /// * `spec` - The OpenAPI specification to split
    ///
    /// # Returns
    ///
    /// A [`SplitResult`] containing the main spec and extracted fragments.
    fn split(&self, spec: OpenApi) -> SplitResult<Self::Fragment>;
}

/// Extension trait for convenient splitting of OpenAPI specifications.
pub trait OpenApiSplitExt {
    /// Splits this specification using the provided splitter.
    ///
    /// This is a convenience method that calls `splitter.split(self)`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use clawspec_core::split::{OpenApiSplitExt, SplitSchemasByTag};
    ///
    /// let spec: OpenApi = /* ... */;
    /// let result = spec.split_with(SplitSchemasByTag::new("common.yaml"));
    /// ```
    fn split_with<S: OpenApiSplitter>(self, splitter: S) -> SplitResult<S::Fragment>;
}

impl OpenApiSplitExt for OpenApi {
    fn split_with<S: OpenApiSplitter>(self, splitter: S) -> SplitResult<S::Fragment> {
        splitter.split(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::openapi::OpenApiBuilder;

    struct NoOpSplitter;

    impl OpenApiSplitter for NoOpSplitter {
        type Fragment = String;

        fn split(&self, spec: OpenApi) -> SplitResult<Self::Fragment> {
            SplitResult::new(spec)
        }
    }

    #[test]
    fn should_implement_split_ext() {
        let spec = OpenApiBuilder::new().build();
        let result = spec.split_with(NoOpSplitter);

        assert!(result.is_unsplit());
    }
}
