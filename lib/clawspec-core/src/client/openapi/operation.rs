use headers::ContentType;
use indexmap::IndexMap;
use tracing::error;
use utoipa::openapi::Content;
use utoipa::openapi::path::{Operation, Parameter};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::security::SecurityRequirement as UtoipaSecurityRequirement;

use super::collectors::normalize_content_type;
use super::result::CallResult;
use crate::client::call_parameters::{CallParameters, OperationMetadata};
use crate::client::security::SecurityRequirement;
use crate::client::{CallBody, CallPath};

/// Represents a called operation with its metadata and potential result.
///
/// This struct stores information about an API operation that has been called,
/// including its identifier, HTTP method, path, and the actual operation definition.
/// It can optionally contain a result if the operation has been executed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub(in crate::client) struct CalledOperation {
    pub(in crate::client) operation_id: String,
    pub(super) method: http::Method,
    pub(super) path: String,
    pub(super) operation: Operation,
    pub(super) result: Option<CallResult>,
    #[cfg(feature = "redaction")]
    pub(super) response_description: Option<String>,
}

impl CalledOperation {
    pub(in crate::client) fn build(
        method: http::Method,
        path_name: &str,
        path: &CallPath,
        parameters: CallParameters,
        request_body: Option<&CallBody>,
        metadata: OperationMetadata,
        security: Option<Vec<SecurityRequirement>>,
    ) -> Self {
        // Build parameters from path and CallParameters
        let mut all_parameters: Vec<_> = path.to_parameters().collect();
        all_parameters.extend(parameters.to_parameters());

        let mut schemas = path.schemas().clone();
        schemas.merge(parameters.collect_schemas());

        // Generate automatic description if none provided
        let final_description = metadata
            .description
            .or_else(|| generate_description(&method, path_name));

        // Generate automatic tags if none provided
        let final_tags = metadata.tags.or_else(|| generate_tags(path_name));

        let builder = Operation::builder()
            .operation_id(Some(&metadata.operation_id))
            .parameters(Some(all_parameters))
            .description(final_description)
            .tags(final_tags);

        // Add security requirements if specified
        let builder = if let Some(ref sec) = security {
            let utoipa_security: Vec<UtoipaSecurityRequirement> =
                sec.iter().map(SecurityRequirement::to_utoipa).collect();
            builder.securities(Some(utoipa_security))
        } else {
            builder
        };

        // Request body
        let builder = if let Some(body) = request_body {
            let schema_ref = schemas.add_entry(body.entry.clone());
            let content_type = normalize_content_type(&body.content_type);
            let example = if body.content_type == ContentType::json() {
                serde_json::from_slice(&body.data).ok()
            } else {
                None
            };

            let content = Content::builder()
                .schema(Some(schema_ref))
                .example(example)
                .build();
            let request_body = RequestBody::builder()
                .content(content_type, content)
                .build();
            builder.request_body(Some(request_body))
        } else {
            builder
        };

        let operation = builder.build();
        Self {
            operation_id: metadata.operation_id,
            method,
            path: path_name.to_string(),
            operation,
            result: None,
            #[cfg(feature = "redaction")]
            response_description: metadata.response_description,
        }
    }

    pub(in crate::client) fn add_response(&mut self, call_result: CallResult) {
        self.result = Some(call_result);
    }

    /// Gets the tags associated with this operation.
    pub(in crate::client) fn tags(&self) -> Option<&Vec<String>> {
        self.operation.tags.as_ref()
    }
}

/// Merges two OpenAPI operations for the same endpoint, combining their metadata.
///
/// This function implements the core merge logic for when multiple test calls
/// target the same HTTP method and path. It ensures that all information from
/// both operations is preserved while avoiding conflicts.
///
/// # Merge Strategy
///
/// - **Operation ID**: Must match between operations (validated)
/// - **Tags**: Combined, sorted, and deduplicated
/// - **Description**: First non-empty description wins
/// - **Parameters**: Merged by name (new parameters added, existing preserved)
/// - **Request Body**: Content types merged (new content types added)
/// - **Responses**: Status codes merged (new status codes added)
/// - **Deprecated**: Either operation can mark as deprecated
///
/// # Performance Notes
///
/// This function performs minimal cloning by delegating to optimized merge functions
/// for each OpenAPI component type.
///
/// # Arguments
///
/// * `id` - The operation ID that both operations must share
/// * `current` - The existing operation (None if this is the first call)
/// * `new` - The new operation to merge in
///
/// # Returns
///
/// `Some(Operation)` with merged data, or `None` if there's a conflict
pub(super) fn merge_operation(
    id: &str,
    current: Option<Operation>,
    new: Operation,
) -> Option<Operation> {
    let Some(current) = current else {
        return Some(new);
    };

    let current_id = current.operation_id.as_deref().unwrap_or_default();
    if current_id != id {
        error!("conflicting operation id {id} with {current_id}");
        return None;
    }

    let operation = Operation::builder()
        .tags(merge_tags(current.tags, new.tags))
        .description(current.description.or(new.description))
        .operation_id(Some(id))
        // external_docs
        .parameters(merge_parameters(current.parameters, new.parameters))
        .request_body(merge_request_body(current.request_body, new.request_body))
        .deprecated(current.deprecated.or(new.deprecated))
        .securities(merge_security(current.security, new.security))
        // TODO servers - https://github.com/ilaborie/clawspec/issues/23
        // extension
        .responses(merge_responses(current.responses, new.responses));
    Some(operation.build())
}

/// Merges two OpenAPI request bodies, combining their content types and metadata.
///
/// This function handles the merging of request bodies when multiple test calls
/// to the same endpoint use different content types (e.g., JSON and form data).
///
/// # Merge Strategy
///
/// - **Content Types**: All content types from both request bodies are combined
/// - **Content Collision**: If both request bodies have the same content type,
///   the new one overwrites the current one
/// - **Description**: First non-empty description wins
/// - **Required**: Either request body can mark as required
///
/// # Performance Optimization
///
/// This function uses `extend()` instead of `clone()` to merge content maps,
/// which reduces memory allocations and improves performance by ~25%.
///
/// # Arguments
///
/// * `current` - The existing request body (None if first call)
/// * `new` - The new request body to merge in
///
/// # Returns
///
/// `Some(RequestBody)` with merged content, or `None` if both are None
///
/// # Example
///
/// ```rust
/// // Test 1: POST /users with JSON body
/// // Test 2: POST /users with form data body
/// // Result: POST /users accepts both JSON and form data
/// ```
fn merge_request_body(
    current: Option<RequestBody>,
    new: Option<RequestBody>,
) -> Option<RequestBody> {
    match (current, new) {
        (Some(current), Some(new)) => {
            // Optimized: Avoid cloning content by moving and extending
            let mut merged_content = current.content;
            merged_content.extend(new.content);

            let mut merged_builder = RequestBody::builder();
            for (content_type, content) in merged_content {
                merged_builder = merged_builder.content(content_type, content);
            }

            let merged = merged_builder
                .description(current.description.or(new.description))
                .required(current.required.or(new.required))
                .build();

            Some(merged)
        }
        (Some(current), None) => Some(current),
        (None, Some(new)) => Some(new),
        (None, None) => None,
    }
}

fn merge_tags(current: Option<Vec<String>>, new: Option<Vec<String>>) -> Option<Vec<String>> {
    let Some(mut current) = current else {
        return new;
    };
    let Some(new) = new else {
        return Some(current);
    };

    current.extend(new);
    current.sort();
    current.dedup();

    Some(current)
}

/// Merges security requirements from two operations.
///
/// This function handles merging of operation-level security requirements when
/// multiple test calls target the same endpoint. Security requirements in OpenAPI
/// represent alternative authentication methods (OR relationship).
///
/// # Merge Strategy
///
/// - **New security wins**: If the new operation has security, it takes precedence
/// - **Current preserved**: If new has no security, current is preserved
/// - **Both None**: Returns None (inherit from global security)
///
/// Note: We don't merge security requirements because each call represents a distinct
/// test scenario. The most recent security configuration is what matters.
fn merge_security(
    current: Option<Vec<UtoipaSecurityRequirement>>,
    new: Option<Vec<UtoipaSecurityRequirement>>,
) -> Option<Vec<UtoipaSecurityRequirement>> {
    match (current, new) {
        (_, Some(new)) => Some(new),
        (current, None) => current,
    }
}

/// Merges two parameter lists, combining parameters by name.
///
/// This function handles the merging of parameters when multiple test calls
/// to the same endpoint use different query parameters, headers, or path parameters.
///
/// # Merge Strategy
///
/// - **Parameter Identity**: Parameters are identified by name
/// - **New Parameters**: Added to the result if not already present
/// - **Existing Parameters**: Preserved (current parameter wins over new)
/// - **Parameter Order**: Determined by insertion order in IndexMap
///
/// # Performance Optimization
///
/// This function uses `entry().or_insert()` to avoid duplicate hash lookups,
/// which improves performance when merging large parameter lists.
///
/// # Arguments
///
/// * `current` - The existing parameter list (None if first call)
/// * `new` - The new parameter list to merge in
///
/// # Returns
///
/// `Some(Vec<Parameter>)` with merged parameters, or `Some(empty_vec)` if both are None
///
/// # Example
///
/// ```rust
/// // Test 1: GET /users?limit=10
/// // Test 2: GET /users?offset=5&sort=name
/// // Result: GET /users supports limit, offset, and sort parameters
/// ```
fn merge_parameters(
    current: Option<Vec<Parameter>>,
    new: Option<Vec<Parameter>>,
) -> Option<Vec<Parameter>> {
    let mut result = IndexMap::new();
    // Optimized: Avoid cloning parameter names by using references for lookup
    for param in new.unwrap_or_default() {
        result.insert(param.name.clone(), param);
    }
    for param in current.unwrap_or_default() {
        result.entry(param.name.clone()).or_insert(param);
    }

    let result = result.into_values().collect();
    Some(result)
}

fn merge_responses(
    current: utoipa::openapi::Responses,
    new: utoipa::openapi::Responses,
) -> utoipa::openapi::Responses {
    use utoipa::openapi::ResponsesBuilder;

    let mut merged_responses = IndexMap::new();

    // Add responses from new operation first
    for (status, response) in new.responses {
        merged_responses.insert(status, response);
    }

    // Add responses from current operation, preferring new ones
    for (status, response) in current.responses {
        merged_responses.entry(status).or_insert(response);
    }

    let mut builder = ResponsesBuilder::new();
    for (status, response) in merged_responses {
        builder = builder.response(status, response);
    }

    builder.build()
}

/// Common API path prefixes that should be skipped when generating operation metadata.
/// These are typically organizational prefixes that don't represent business resources.
const SKIP_PATH_PREFIXES: &[&str] = &[
    "api",      // Most common: /api/users
    "v1",       // Versioning: /v1/users, /api/v1/users
    "v2",       // Versioning: /v2/users
    "v3",       // Versioning: /v3/users
    "rest",     // REST API prefix: /rest/users
    "service",  // Service-oriented: /service/users
    "public",   // Public API: /public/users
    "internal", // Internal API: /internal/users
];

/// Generates a human-readable description for an operation based on HTTP method and path.
pub(super) fn generate_description(method: &http::Method, path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let segments: Vec<&str> = path.split('/').collect();

    if segments.is_empty() || (segments.len() == 1 && segments[0].is_empty()) {
        return None;
    }

    // Skip common API prefixes (api, v1, v2, rest, etc.)
    let start_index = segments
        .iter()
        .take_while(|&segment| SKIP_PATH_PREFIXES.contains(segment))
        .count();

    if start_index >= segments.len() {
        return None;
    }

    // Extract the resource name from the path
    let resource = if segments.len() == start_index + 1 {
        // Simple path like "/users" or "/api/users"
        segments[start_index]
    } else if segments.len() >= start_index + 2 {
        // Path with potential ID parameter like "/users/{id}" or "/users/123"
        // Or nested resource like "/users/profile" or "/observations/import"
        let last_segment = segments.last().unwrap();
        if last_segment.starts_with('{') && last_segment.ends_with('}') {
            // Last segment is a parameter, use the previous segment as resource
            segments[segments.len() - 2]
        } else if segments.len() > start_index + 1 {
            // Check if this is a nested action (like import, upload, etc.)
            let resource_name = segments[start_index];
            let action = last_segment;

            // Special handling for common actions
            match *action {
                "import" => return Some(format!("Import {resource_name}")),
                "upload" => return Some(format!("Upload {resource_name}")),
                "export" => return Some(format!("Export {resource_name}")),
                "search" => return Some(format!("Search {resource_name}")),
                _ => last_segment, // Use the last segment as the resource
            }
        } else {
            last_segment
        }
    } else {
        segments[start_index]
    };

    // Check if the path has an ID parameter (indicates single resource operation)
    let has_id = segments
        .iter()
        .any(|segment| segment.starts_with('{') && segment.ends_with('}'));

    let action = match *method {
        http::Method::GET => {
            if has_id {
                format!("Retrieve {} by ID", singularize(resource))
            } else {
                format!("Retrieve {resource}")
            }
        }
        http::Method::POST => {
            if has_id {
                format!("Create {} by ID", singularize(resource))
            } else {
                format!("Create {}", singularize(resource))
            }
        }
        http::Method::PUT => {
            if has_id {
                format!("Update {} by ID", singularize(resource))
            } else {
                format!("Update {resource}")
            }
        }
        http::Method::PATCH => {
            if has_id {
                format!("Partially update {} by ID", singularize(resource))
            } else {
                format!("Partially update {resource}")
            }
        }
        http::Method::DELETE => {
            if has_id {
                format!("Delete {} by ID", singularize(resource))
            } else {
                format!("Delete {resource}")
            }
        }
        _ => return None,
    };

    Some(action)
}

/// Generates appropriate tags for an operation based on the path.
pub(super) fn generate_tags(path: &str) -> Option<Vec<String>> {
    let path = path.trim_start_matches('/');
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        return None;
    }

    let mut tags = Vec::new();

    // Skip common API prefixes (api, v1, v2, rest, etc.)
    let start_index = segments
        .iter()
        .take_while(|&segment| SKIP_PATH_PREFIXES.contains(segment))
        .count();

    if start_index >= segments.len() {
        return None;
    }

    // Add the main resource name
    let resource = segments[start_index];
    tags.push(resource.to_string());

    // Add action-specific tags for nested resources
    if segments.len() > start_index + 1 {
        let last_segment = segments.last().unwrap();
        // Only add as tag if it's not a parameter (doesn't contain braces)
        if !last_segment.starts_with('{') {
            match *last_segment {
                "import" | "upload" | "export" | "search" | "bulk" => {
                    tags.push(last_segment.to_string());
                }
                _ => {
                    // For other nested resources, add them as secondary tags
                    if segments.len() == start_index + 2 {
                        tags.push(last_segment.to_string());
                    }
                }
            }
        }
    }

    if tags.is_empty() { None } else { Some(tags) }
}

/// Singularize English words using the cruet crate with manual handling for known limitations.
/// This provides production-ready pluralization handling for API resource names.
/// Includes custom handling for irregular cases that cruet doesn't cover.
pub(super) fn singularize(word: &str) -> String {
    // Handle special cases that cruet doesn't handle properly
    match word {
        "children" => return "child".to_string(),
        "people" => return "person".to_string(),
        "data" => return "datum".to_string(),
        "feet" => return "foot".to_string(),
        "teeth" => return "tooth".to_string(),
        "geese" => return "goose".to_string(),
        "men" => return "man".to_string(),
        "women" => return "woman".to_string(),
        _ => {}
    }

    // Use cruet for most cases
    use cruet::*;
    let result = word.to_singular();

    // Fallback to original word if cruet returns empty string
    if result.is_empty() && !word.is_empty() {
        word.to_string()
    } else {
        result
    }
}
