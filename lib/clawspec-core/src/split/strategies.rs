//! Built-in splitting strategies for OpenAPI specifications.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use utoipa::openapi::path::{Operation, PathItem};
use utoipa::openapi::{Components, OpenApi, Ref, RefOr};

use super::{Fragment, OpenApiSplitter, SplitResult};

/// Helper to iterate over all operations in a PathItem.
fn iter_operations(path_item: &PathItem) -> impl Iterator<Item = &Operation> {
    [
        path_item.get.as_ref(),
        path_item.put.as_ref(),
        path_item.post.as_ref(),
        path_item.delete.as_ref(),
        path_item.options.as_ref(),
        path_item.head.as_ref(),
        path_item.patch.as_ref(),
        path_item.trace.as_ref(),
    ]
    .into_iter()
    .flatten()
}

/// Splits schemas based on which tags use them.
///
/// This splitter analyzes which schemas are referenced by operations with specific tags
/// and organizes them into separate files:
///
/// - Schemas used by only one tag go into a file named after that tag
/// - Schemas used by multiple tags go into a common file
///
/// # Example
///
/// ```rust,ignore
/// use clawspec_core::split::{OpenApiSplitter, SplitSchemasByTag};
///
/// let splitter = SplitSchemasByTag::new("common-types.yaml");
/// let result = splitter.split(spec);
///
/// // Result might contain:
/// // - main openapi.yaml with $refs to external files
/// // - users.yaml with User, CreateUser schemas
/// // - orders.yaml with Order, OrderItem schemas
/// // - common-types.yaml with Error, Pagination schemas used by both
/// ```
#[derive(Debug, Clone)]
pub struct SplitSchemasByTag {
    /// Path for schemas used by multiple tags.
    common_file: PathBuf,
    /// Optional directory prefix for tag-specific files.
    schemas_dir: Option<PathBuf>,
}

impl SplitSchemasByTag {
    /// Creates a new splitter with the specified common file path.
    ///
    /// Tag-specific files will be created in the same directory as the common file.
    pub fn new(common_file: impl Into<PathBuf>) -> Self {
        Self {
            common_file: common_file.into(),
            schemas_dir: None,
        }
    }

    /// Sets the directory for schema files.
    ///
    /// Both tag-specific and common files will be placed in this directory.
    pub fn with_schemas_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.schemas_dir = Some(dir.into());
        self
    }

    /// Analyzes which tags reference which schemas.
    fn analyze_schema_usage(&self, spec: &OpenApi) -> BTreeMap<String, BTreeSet<String>> {
        let mut schema_to_tags: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        // Iterate through all paths and operations
        for (_path, path_item) in spec.paths.paths.iter() {
            for operation in iter_operations(path_item) {
                let tags = operation.tags.clone().unwrap_or_default();
                if tags.is_empty() {
                    continue;
                }

                // Collect schema references from request body
                if let Some(ref request_body) = operation.request_body {
                    for content in request_body.content.values() {
                        if let Some(ref schema) = content.schema {
                            self.collect_schema_refs(schema, &tags, &mut schema_to_tags);
                        }
                    }
                }

                // Collect schema references from responses
                for response in operation.responses.responses.values() {
                    if let RefOr::T(resp) = response {
                        for content in resp.content.values() {
                            if let Some(ref schema) = content.schema {
                                self.collect_schema_refs(schema, &tags, &mut schema_to_tags);
                            }
                        }
                    }
                }

                // Collect schema references from parameters
                if let Some(ref parameters) = operation.parameters {
                    for param in parameters {
                        if let Some(ref schema) = param.schema {
                            self.collect_schema_refs(schema, &tags, &mut schema_to_tags);
                        }
                    }
                }
            }
        }

        schema_to_tags
    }

    /// Collects schema references from a schema, adding tag associations.
    fn collect_schema_refs(
        &self,
        schema: &RefOr<utoipa::openapi::Schema>,
        tags: &[String],
        schema_to_tags: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        match schema {
            RefOr::Ref(r) => {
                if let Some(name) = extract_schema_name(&r.ref_location) {
                    let entry = schema_to_tags.entry(name).or_default();
                    for tag in tags {
                        entry.insert(tag.clone());
                    }
                }
            }
            RefOr::T(_) => {
                // Inline schema, no reference to extract
            }
        }
    }

    /// Determines the target file for a schema based on its tag usage.
    fn target_file_for_schema(&self, _schema_name: &str, tags: &BTreeSet<String>) -> PathBuf {
        let base_dir = self.schemas_dir.clone().unwrap_or_default();

        if tags.len() == 1 {
            // Schema used by only one tag - put in tag-specific file
            let tag = tags.iter().next().expect("checked len == 1");
            base_dir.join(format!("{tag}.yaml"))
        } else {
            // Schema used by multiple tags or no tags - put in common file
            if self.schemas_dir.is_some() {
                base_dir.join(&self.common_file)
            } else {
                self.common_file.clone()
            }
        }
    }

    /// Creates external reference string for a schema in a file.
    fn create_external_ref(file_path: &std::path::Path, schema_name: &str) -> String {
        format!(
            "{}#/components/schemas/{}",
            file_path.display(),
            schema_name
        )
    }
}

impl OpenApiSplitter for SplitSchemasByTag {
    type Fragment = Components;

    fn split(&self, mut spec: OpenApi) -> SplitResult<Self::Fragment> {
        let schema_to_tags = self.analyze_schema_usage(&spec);

        // Group schemas by their target file
        let mut file_to_schemas: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
        for (schema_name, tags) in &schema_to_tags {
            let target = self.target_file_for_schema(schema_name, tags);
            file_to_schemas
                .entry(target)
                .or_default()
                .insert(schema_name.clone());
        }

        // If all schemas go to one file or no schemas, no splitting needed
        if file_to_schemas.len() <= 1 {
            return SplitResult::new(spec);
        }

        let mut result = SplitResult::new(spec.clone());

        // Extract schemas and create fragments
        let original_components = spec.components.take().unwrap_or_default();
        let mut remaining_schemas = original_components.schemas.clone();

        for (file_path, schema_names) in &file_to_schemas {
            let mut fragment_components = Components::new();

            for schema_name in schema_names {
                if let Some(schema) = remaining_schemas.remove(schema_name) {
                    fragment_components
                        .schemas
                        .insert(schema_name.clone(), schema);
                }
            }

            if !fragment_components.schemas.is_empty() {
                result.add_fragment(Fragment::new(file_path.clone(), fragment_components));
            }
        }

        // Update the main spec's schema references to point to external files
        let mut new_components = Components::new();

        // Add external references for extracted schemas
        for (file_path, schema_names) in &file_to_schemas {
            for schema_name in schema_names {
                let external_ref = Self::create_external_ref(file_path, schema_name);
                new_components
                    .schemas
                    .insert(schema_name.clone(), RefOr::Ref(Ref::new(external_ref)));
            }
        }

        // Keep any remaining schemas that weren't extracted
        for (name, schema) in remaining_schemas {
            new_components.schemas.insert(name, schema);
        }

        // Preserve security schemes and responses
        new_components.security_schemes = original_components.security_schemes;
        new_components.responses = original_components.responses;

        result.main.components = Some(new_components);
        result
    }
}

/// Extracts schemas matching a predicate into a separate file.
///
/// This splitter allows fine-grained control over which schemas are extracted
/// by providing a predicate function that determines whether a schema should
/// be moved to the external file.
///
/// # Example
///
/// ```rust,ignore
/// use clawspec_core::split::{OpenApiSplitter, ExtractSchemasByPredicate};
///
/// // Extract all error-related schemas
/// let splitter = ExtractSchemasByPredicate::new(
///     "errors.yaml",
///     |name| name.contains("Error") || name.contains("Exception"),
/// );
/// let result = splitter.split(spec);
/// ```
#[derive(Clone)]
pub struct ExtractSchemasByPredicate<F>
where
    F: Fn(&str) -> bool,
{
    /// Path for the extracted schemas file.
    target_file: PathBuf,
    /// Predicate function that returns true for schemas to extract.
    predicate: F,
}

impl<F> ExtractSchemasByPredicate<F>
where
    F: Fn(&str) -> bool,
{
    /// Creates a new splitter with the specified target file and predicate.
    ///
    /// The predicate receives the schema name and should return `true`
    /// if the schema should be extracted to the target file.
    pub fn new(target_file: impl Into<PathBuf>, predicate: F) -> Self {
        Self {
            target_file: target_file.into(),
            predicate,
        }
    }
}

impl<F> OpenApiSplitter for ExtractSchemasByPredicate<F>
where
    F: Fn(&str) -> bool,
{
    type Fragment = Components;

    fn split(&self, mut spec: OpenApi) -> SplitResult<Self::Fragment> {
        let Some(mut components) = spec.components.take() else {
            return SplitResult::new(spec);
        };

        // Find schemas to extract (collect names first to avoid borrowing issues)
        let schemas_to_extract: Vec<String> = components
            .schemas
            .keys()
            .filter(|name| (self.predicate)(name))
            .cloned()
            .collect();

        // If nothing to extract, return unchanged
        if schemas_to_extract.is_empty() {
            spec.components = Some(components);
            return SplitResult::new(spec);
        }

        // Extract matching schemas
        let mut extracted = Components::new();
        for name in &schemas_to_extract {
            if let Some(schema) = components.schemas.remove(name) {
                extracted.schemas.insert(name.clone(), schema);
            }
        }

        // Create external references for extracted schemas
        for name in &schemas_to_extract {
            let external_ref = format!(
                "{}#/components/schemas/{}",
                self.target_file.display(),
                name
            );
            components
                .schemas
                .insert(name.clone(), RefOr::Ref(Ref::new(external_ref)));
        }

        spec.components = Some(components);

        let mut result = SplitResult::new(spec);
        result.add_fragment(Fragment::new(self.target_file.clone(), extracted));
        result
    }
}

/// Extracts the schema name from a $ref string.
///
/// # Example
///
/// ```rust,ignore
/// assert_eq!(extract_schema_name("#/components/schemas/User"), Some("User".to_string()));
/// ```
fn extract_schema_name(ref_location: &str) -> Option<String> {
    const SCHEMA_PREFIX: &str = "#/components/schemas/";
    ref_location
        .strip_prefix(SCHEMA_PREFIX)
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::openapi::path::OperationBuilder;
    use utoipa::openapi::path::PathItemBuilder;
    use utoipa::openapi::{ContentBuilder, ObjectBuilder, OpenApiBuilder, ResponseBuilder};

    fn create_test_spec() -> OpenApi {
        let user_schema = ObjectBuilder::new()
            .property(
                "id",
                ObjectBuilder::new().schema_type(utoipa::openapi::Type::Integer),
            )
            .property(
                "name",
                ObjectBuilder::new().schema_type(utoipa::openapi::Type::String),
            )
            .build();

        let error_schema = ObjectBuilder::new()
            .property(
                "code",
                ObjectBuilder::new().schema_type(utoipa::openapi::Type::Integer),
            )
            .property(
                "message",
                ObjectBuilder::new().schema_type(utoipa::openapi::Type::String),
            )
            .build();

        let order_schema = ObjectBuilder::new()
            .property(
                "id",
                ObjectBuilder::new().schema_type(utoipa::openapi::Type::Integer),
            )
            .property(
                "total",
                ObjectBuilder::new().schema_type(utoipa::openapi::Type::Number),
            )
            .build();

        let mut components = Components::new();
        components
            .schemas
            .insert("User".to_string(), RefOr::T(user_schema.into()));
        components
            .schemas
            .insert("Error".to_string(), RefOr::T(error_schema.into()));
        components
            .schemas
            .insert("Order".to_string(), RefOr::T(order_schema.into()));

        // Create operations with tags
        let get_users = OperationBuilder::new()
            .tags(Some(vec!["users".to_string()]))
            .response(
                "200",
                ResponseBuilder::new()
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(RefOr::Ref(Ref::new("#/components/schemas/User"))))
                            .build(),
                    )
                    .build(),
            )
            .build();

        let get_orders = OperationBuilder::new()
            .tags(Some(vec!["orders".to_string()]))
            .response(
                "200",
                ResponseBuilder::new()
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(RefOr::Ref(Ref::new("#/components/schemas/Order"))))
                            .build(),
                    )
                    .build(),
            )
            .response(
                "400",
                ResponseBuilder::new()
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(RefOr::Ref(Ref::new("#/components/schemas/Error"))))
                            .build(),
                    )
                    .build(),
            )
            .build();

        let get_user_orders = OperationBuilder::new()
            .tags(Some(vec!["users".to_string(), "orders".to_string()]))
            .response(
                "400",
                ResponseBuilder::new()
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(RefOr::Ref(Ref::new("#/components/schemas/Error"))))
                            .build(),
                    )
                    .build(),
            )
            .build();

        let mut paths = utoipa::openapi::Paths::new();
        paths.paths.insert(
            "/users".to_string(),
            PathItemBuilder::new()
                .operation(utoipa::openapi::HttpMethod::Get, get_users)
                .build(),
        );
        paths.paths.insert(
            "/orders".to_string(),
            PathItemBuilder::new()
                .operation(utoipa::openapi::HttpMethod::Get, get_orders)
                .build(),
        );
        paths.paths.insert(
            "/users/{id}/orders".to_string(),
            PathItemBuilder::new()
                .operation(utoipa::openapi::HttpMethod::Get, get_user_orders)
                .build(),
        );

        OpenApiBuilder::new()
            .paths(paths)
            .components(Some(components))
            .build()
    }

    #[test]
    fn should_extract_schema_name() {
        assert_eq!(
            extract_schema_name("#/components/schemas/User"),
            Some("User".to_string())
        );
        assert_eq!(
            extract_schema_name("#/components/schemas/MyError"),
            Some("MyError".to_string())
        );
        assert_eq!(extract_schema_name("#/components/responses/Error"), None);
        assert_eq!(extract_schema_name("User"), None);
    }

    #[test]
    fn should_split_by_predicate() {
        let spec = create_test_spec();

        let splitter = ExtractSchemasByPredicate::new("errors.yaml", |name| name.contains("Error"));
        let result = splitter.split(spec);

        assert_eq!(result.fragment_count(), 1);
        let fragment = &result.fragments[0];
        assert_eq!(fragment.path, PathBuf::from("errors.yaml"));
        assert!(fragment.content.schemas.contains_key("Error"));
        assert!(!fragment.content.schemas.contains_key("User"));
        assert!(!fragment.content.schemas.contains_key("Order"));

        // Main spec should have external reference for Error
        let main_components = result
            .main
            .components
            .as_ref()
            .expect("should have components");
        match main_components.schemas.get("Error") {
            Some(RefOr::Ref(r)) => {
                assert!(r.ref_location.contains("errors.yaml"));
            }
            _ => panic!("Expected external reference for Error"),
        }
    }

    #[test]
    fn should_not_split_when_predicate_matches_nothing() {
        let spec = create_test_spec();

        let splitter =
            ExtractSchemasByPredicate::new("nothing.yaml", |name| name.contains("NonExistent"));
        let result = splitter.split(spec);

        assert!(result.is_unsplit());
    }

    #[test]
    fn should_analyze_schema_usage() {
        let spec = create_test_spec();
        let splitter = SplitSchemasByTag::new("common.yaml");

        let usage = splitter.analyze_schema_usage(&spec);

        // User is used by "users" tag
        assert!(
            usage
                .get("User")
                .map(|t| t.contains("users"))
                .unwrap_or(false)
        );

        // Order is used by "orders" tag
        assert!(
            usage
                .get("Order")
                .map(|t| t.contains("orders"))
                .unwrap_or(false)
        );

        // Error is used by both "users" and "orders" tags
        let error_tags = usage.get("Error").expect("Error should be tracked");
        assert!(error_tags.contains("orders"));
    }
}
