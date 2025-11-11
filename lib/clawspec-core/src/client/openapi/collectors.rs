use headers::ContentType;
use http::Method;
use indexmap::IndexMap;
use tracing::warn;
use utoipa::openapi::{PathItem, RefOr, Schema};

use super::operation::{CalledOperation, merge_operation};
use super::schema::Schemas;

/// Normalizes content types for OpenAPI specification by removing parameters
/// that are implementation details (like multipart boundaries, charset, etc.).
pub(super) fn normalize_content_type(content_type: &ContentType) -> String {
    let content_type_str = content_type.to_string();

    // Strip all parameters by truncating at the first semicolon
    if let Some(semicolon_pos) = content_type_str.find(';') {
        content_type_str[..semicolon_pos].to_string()
    } else {
        content_type_str
    }
}

/// Collects and merges OpenAPI operations and schemas from API test executions.
///
/// # Schema Merge Behavior
///
/// The `Collectors` struct implements intelligent merging behavior for OpenAPI operations
/// and schemas to handle multiple test calls to the same endpoint with different parameters,
/// headers, or request bodies.
///
/// ## Operation Merging
///
/// When multiple tests call the same endpoint (same HTTP method and path), the operations
/// are merged using the following rules:
///
/// - **Parameters**: New parameters are added; existing parameters are preserved by name
/// - **Request Bodies**: Content types are merged; same content type overwrites previous
/// - **Responses**: New response status codes are added; existing status codes are preserved
/// - **Tags**: Tags from all operations are combined, sorted, and deduplicated
/// - **Description**: First non-empty description is used
///
/// ## Schema Merging
///
/// Schemas are merged by TypeId to ensure type safety:
///
/// - **Type Identity**: Same Rust type (TypeId) maps to same schema entry
/// - **Examples**: Examples from all usages are collected and deduplicated
/// - **Primitive Types**: Inlined directly (String, i32, etc.)
/// - **Complex Types**: Referenced in components/schemas section
///
/// ## Performance Optimizations
///
/// The merge operations have been optimized to reduce memory allocations:
///
/// - **Request Body Merging**: Uses `extend()` instead of `clone()` for content maps
/// - **Parameter Merging**: Uses `entry().or_insert()` to avoid duplicate lookups
/// - **Schema Merging**: Direct insertion by TypeId for O(1) lookup
///
/// ## Example Usage
///
/// ```rust,ignore
/// // Internal usage - not exposed in public API
/// let mut collectors = Collectors::default();
///
/// // Schemas from different test calls are merged
/// collectors.collect_schemas(schemas_from_test_1);
/// collectors.collect_schemas(schemas_from_test_2);
///
/// // Operations with same endpoint are merged
/// collectors.collect_operation(get_users_operation);
/// collectors.collect_operation(get_users_with_params_operation);
/// ```
#[derive(Debug, Clone, Default)]
pub(in crate::client) struct Collectors {
    pub(super) operations: IndexMap<String, Vec<CalledOperation>>,
    pub(in crate::client) schemas: Schemas,
}

impl Collectors {
    pub(in crate::client) fn collect_schemas(&mut self, schemas: Schemas) {
        self.schemas.merge(schemas);
    }

    pub(in crate::client) fn collect_schema_entry(&mut self, entry: super::schema::SchemaEntry) {
        self.schemas.add_entry(entry);
    }

    pub(in crate::client) fn collect_operation(
        &mut self,
        operation: CalledOperation,
    ) -> Option<&mut CalledOperation> {
        let operation_id = operation.operation_id.clone();
        let operations = self.operations.entry(operation_id).or_default();

        operations.push(operation);
        operations.last_mut()
    }

    pub(in crate::client) fn schemas(&self) -> Vec<(String, RefOr<Schema>)> {
        self.schemas.schema_vec()
    }

    /// Returns an iterator over collected operations.
    ///
    /// This method provides access to all operations that have been collected
    /// during API calls, which is useful for tag computation and analysis.
    pub(in crate::client) fn operations(&self) -> impl Iterator<Item = &CalledOperation> {
        self.operations.values().flatten()
    }

    pub(in crate::client) fn as_map(&mut self, base_path: &str) -> IndexMap<String, PathItem> {
        let mut result = IndexMap::<String, PathItem>::new();
        for (operation_id, calls) in &self.operations {
            debug_assert!(!calls.is_empty(), "having at least a call");
            let path = format!("{base_path}/{}", calls[0].path.trim_start_matches('/'));
            let item = result.entry(path.clone()).or_default();
            for call in calls {
                let method = call.method.clone();
                match method {
                    Method::GET => {
                        item.get =
                            merge_operation(operation_id, item.get.clone(), call.operation.clone());
                    }
                    Method::PUT => {
                        item.put =
                            merge_operation(operation_id, item.put.clone(), call.operation.clone());
                    }
                    Method::POST => {
                        item.post = merge_operation(
                            operation_id,
                            item.post.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::DELETE => {
                        item.delete = merge_operation(
                            operation_id,
                            item.delete.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::OPTIONS => {
                        item.options = merge_operation(
                            operation_id,
                            item.options.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::HEAD => {
                        item.head = merge_operation(
                            operation_id,
                            item.head.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::PATCH => {
                        item.patch = merge_operation(
                            operation_id,
                            item.patch.clone(),
                            call.operation.clone(),
                        );
                    }
                    Method::TRACE => {
                        item.trace = merge_operation(
                            operation_id,
                            item.trace.clone(),
                            call.operation.clone(),
                        );
                    }
                    _ => {
                        warn!(%method, "unsupported method");
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod operation_metadata_tests {
    use super::super::operation::{generate_description, generate_tags, singularize};
    use super::*;
    use http::Method;

    #[test]
    fn test_generate_description_simple_paths() {
        assert_eq!(
            generate_description(&Method::GET, "/users"),
            Some("Retrieve users".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/users"),
            Some("Create user".to_string())
        );
        assert_eq!(
            generate_description(&Method::PUT, "/users"),
            Some("Update users".to_string())
        );
        assert_eq!(
            generate_description(&Method::DELETE, "/users"),
            Some("Delete users".to_string())
        );
        assert_eq!(
            generate_description(&Method::PATCH, "/users"),
            Some("Partially update users".to_string())
        );
    }

    #[test]
    fn test_generate_description_with_id_parameter() {
        assert_eq!(
            generate_description(&Method::GET, "/users/{id}"),
            Some("Retrieve user by ID".to_string())
        );
        assert_eq!(
            generate_description(&Method::PUT, "/users/{id}"),
            Some("Update user by ID".to_string())
        );
        assert_eq!(
            generate_description(&Method::DELETE, "/users/{id}"),
            Some("Delete user by ID".to_string())
        );
        assert_eq!(
            generate_description(&Method::PATCH, "/users/{id}"),
            Some("Partially update user by ID".to_string())
        );
    }

    #[test]
    fn test_generate_description_special_actions() {
        assert_eq!(
            generate_description(&Method::POST, "/observations/import"),
            Some("Import observations".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/observations/upload"),
            Some("Upload observations".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/users/export"),
            Some("Export users".to_string())
        );
        assert_eq!(
            generate_description(&Method::GET, "/users/search"),
            Some("Search users".to_string())
        );
    }

    #[test]
    fn test_generate_description_api_prefix() {
        assert_eq!(
            generate_description(&Method::GET, "/api/observations"),
            Some("Retrieve observations".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/api/observations/import"),
            Some("Import observations".to_string())
        );
        // Test multiple prefixes
        assert_eq!(
            generate_description(&Method::GET, "/api/v1/users"),
            Some("Retrieve users".to_string())
        );
        assert_eq!(
            generate_description(&Method::POST, "/rest/service/items"),
            Some("Create item".to_string())
        );
    }

    #[test]
    fn test_generate_tags_simple_paths() {
        assert_eq!(generate_tags("/users"), Some(vec!["users".to_string()]));
        assert_eq!(
            generate_tags("/observations"),
            Some(vec!["observations".to_string()])
        );
    }

    #[test]
    fn test_generate_tags_with_api_prefix() {
        assert_eq!(generate_tags("/api/users"), Some(vec!["users".to_string()]));
        assert_eq!(
            generate_tags("/api/observations"),
            Some(vec!["observations".to_string()])
        );
        // Test multiple prefixes
        assert_eq!(
            generate_tags("/api/v1/users"),
            Some(vec!["users".to_string()])
        );
        assert_eq!(
            generate_tags("/rest/service/items"),
            Some(vec!["items".to_string()])
        );
    }

    #[test]
    fn test_generate_tags_with_special_actions() {
        assert_eq!(
            generate_tags("/api/observations/import"),
            Some(vec!["observations".to_string(), "import".to_string()])
        );
        assert_eq!(
            generate_tags("/api/observations/upload"),
            Some(vec!["observations".to_string(), "upload".to_string()])
        );
        assert_eq!(
            generate_tags("/users/export"),
            Some(vec!["users".to_string(), "export".to_string()])
        );
    }

    #[test]
    fn test_generate_tags_with_id_parameter() {
        assert_eq!(
            generate_tags("/api/observations/{id}"),
            Some(vec!["observations".to_string()])
        );
        assert_eq!(
            generate_tags("/users/{user_id}"),
            Some(vec!["users".to_string()])
        );
    }

    #[test]
    fn test_singularize() {
        // Regular plurals that cruet handles well
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("observations"), "observation");
        assert_eq!(singularize("items"), "item");

        // Irregular plurals - handled by manual overrides + cruet
        assert_eq!(singularize("mice"), "mouse"); // cruet handles this
        assert_eq!(singularize("children"), "child"); // manual override
        assert_eq!(singularize("people"), "person"); // manual override
        assert_eq!(singularize("feet"), "foot"); // manual override
        assert_eq!(singularize("teeth"), "tooth"); // manual override
        assert_eq!(singularize("geese"), "goose"); // manual override
        assert_eq!(singularize("men"), "man"); // manual override
        assert_eq!(singularize("women"), "woman"); // manual override
        assert_eq!(singularize("data"), "datum"); // manual override

        // Words ending in 'es'
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("watches"), "watch");

        // Already singular - cruet handles these gracefully
        assert_eq!(singularize("user"), "user");
        assert_eq!(singularize("child"), "child");

        // Edge cases - with fallback protection
        assert_eq!(singularize("s"), "s"); // Falls back to original when cruet returns empty
        assert_eq!(singularize(""), ""); // Empty string stays empty

        // Complex cases that cruet handles well
        assert_eq!(singularize("categories"), "category");
        assert_eq!(singularize("companies"), "company");
        assert_eq!(singularize("libraries"), "library");

        // Additional cases cruet handles
        assert_eq!(singularize("stories"), "story");
        assert_eq!(singularize("cities"), "city");
    }

    #[test]
    fn test_normalize_json_content_type() {
        let content_type = ContentType::json();
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/json");
    }

    #[test]
    fn test_normalize_multipart_content_type() {
        // Create a multipart content type with boundary
        let content_type_str = "multipart/form-data; boundary=----formdata-clawspec-12345";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "multipart/form-data");
    }

    #[test]
    fn test_normalize_form_urlencoded_content_type() {
        let content_type = ContentType::form_url_encoded();
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/x-www-form-urlencoded");
    }

    #[test]
    fn test_normalize_content_type_with_charset() {
        // Test content type with charset parameter
        let content_type_str = "application/json; charset=utf-8";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/json");
    }

    #[test]
    fn test_normalize_content_type_with_multiple_parameters() {
        // Test content type with multiple parameters
        let content_type_str = "text/html; charset=utf-8; boundary=something";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "text/html");
    }

    #[test]
    fn test_normalize_content_type_without_parameters() {
        // Test content type without parameters (should remain unchanged)
        let content_type_str = "application/xml";
        let content_type = ContentType::from(content_type_str.parse::<mime::Mime>().unwrap());
        let normalized = normalize_content_type(&content_type);
        assert_eq!(normalized, "application/xml");
    }
}
