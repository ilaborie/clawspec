use http::{Method, Uri};

use super::call_parameters::OperationMetadata;
use super::openapi::channel::CollectorSender;
use super::response::ExpectedStatusCodes;
use super::security::SecurityRequirement;
use super::{CallBody, CallCookies, CallHeaders, CallPath, CallQuery};

pub(in crate::client) const BODY_MAX_LENGTH: usize = 1024;

mod builder;
mod execution;
#[cfg(test)]
mod tests;

/// Builder for configuring HTTP API calls with comprehensive parameter and validation support.
///
/// `ApiCall` provides a fluent interface for building HTTP requests with automatic OpenAPI schema collection.
/// It supports query parameters, headers, request bodies, and flexible status code validation.
///
/// # Method Groups
///
/// ## Request Body Methods
/// - [`json(data)`](Self::json) - Set JSON request body
/// - [`form(data)`](Self::form) - Set form-encoded request body
/// - [`multipart(form)`](Self::multipart) - Set multipart form request body
/// - [`text(content)`](Self::text) - Set plain text request body
/// - [`raw(bytes)`](Self::raw) - Set raw binary request body
///
/// ## Parameter Methods
/// - [`with_query(query)`](Self::with_query) - Set query parameters
/// - [`with_headers(headers)`](Self::with_headers) - Set request headers
/// - [`with_header(name, value)`](Self::with_header) - Add single header
///
/// ## Status Code Validation
/// - [`with_expected_status_codes(codes)`](Self::with_expected_status_codes) - Set expected status codes
/// - [`with_status_range_inclusive(range)`](Self::with_status_range_inclusive) - Set inclusive range (200..=299)
/// - [`with_status_range(range)`](Self::with_status_range) - Set exclusive range (200..300)
/// - [`add_expected_status(code)`](Self::add_expected_status) - Add single expected status
/// - [`add_expected_status_range_inclusive(range)`](Self::add_expected_status_range_inclusive) - Add inclusive range
/// - [`add_expected_status_range(range)`](Self::add_expected_status_range) - Add exclusive range
/// - [`with_client_errors()`](Self::with_client_errors) - Accept 2xx and 4xx codes
///
/// ## OpenAPI Metadata
/// - [`with_operation_id(id)`](Self::with_operation_id) - Set operation ID
/// - [`with_tags(tags)`](Self::with_tags) - Set operation tags (or use automatic tagging)
/// - [`with_description(desc)`](Self::with_description) - Set operation description (or use automatic description)
///
/// ## Response Descriptions (requires `redaction` feature)
/// - [`with_response_description(desc)`](Self::with_response_description) - Set description for the actual returned status code
///
/// ## Execution
/// - `.await` - Execute the request and return response (⚠️ **must consume result for OpenAPI**)
///
/// # Default Behavior
///
/// - **Status codes**: Accepts 200-499 (inclusive of 200, exclusive of 500)
/// - **Content-Type**: Automatically set based on body type
/// - **Schema collection**: Request/response schemas are automatically captured
/// - **Operation metadata**: Automatically generated if not explicitly set
///
/// ## Automatic OpenAPI Metadata Generation
///
/// When you don't explicitly set operation metadata, `ApiCall` automatically generates:
///
/// ### **Automatic Tags**
/// Tags are extracted from the request path using intelligent parsing:
///
/// ```text
/// Path: /api/v1/users/{id}     → Tags: ["users"]
/// Path: /users                 → Tags: ["users"]
/// Path: /users/export          → Tags: ["users", "export"]
/// Path: /observations/import   → Tags: ["observations", "import"]
/// ```
///
/// **Path Prefix Skipping**: Common API prefixes are automatically skipped:
/// - `api`, `v1`, `v2`, `v3`, `rest`, `service` (and more)
/// - `/api/v1/users` becomes `["users"]`, not `["api", "v1", "users"]`
///
/// **Special Action Detection**: Certain path segments get their own tags:
/// - `import`, `upload`, `export`, `search`, `bulk`
/// - `/users/export` → `["users", "export"]`
///
/// ### **Automatic Descriptions**
/// Descriptions are generated based on HTTP method and path:
///
/// ```text
/// GET /users          → "Retrieve users"
/// GET /users/{id}     → "Retrieve user by ID"
/// POST /users         → "Create user"
/// PUT /users/{id}     → "Update user by ID"
/// DELETE /users/{id}  → "Delete user by ID"
/// ```
///
/// ### **Automatic Operation IDs**
/// Generated from HTTP method and path: `"get-users-id"`, `"post-users"`, etc.
///
/// You can override any of these by calling the corresponding `with_*` methods.
#[derive(derive_more::Debug)]
pub struct ApiCall {
    pub(super) client: reqwest::Client,
    pub(super) base_uri: Uri,
    #[debug(skip)]
    pub(super) collector_sender: CollectorSender,

    pub(super) method: Method,
    pub(super) path: CallPath,
    pub(super) query: CallQuery,
    pub(super) headers: Option<CallHeaders>,

    #[debug(ignore)]
    pub(super) body: Option<CallBody>,

    pub(super) authentication: Option<super::Authentication>,
    pub(super) cookies: Option<CallCookies>,
    /// Expected status codes for this request (default: 200..500)
    pub(super) expected_status_codes: ExpectedStatusCodes,
    /// Operation metadata for OpenAPI documentation
    pub(super) metadata: OperationMetadata,
    /// Response description for the actual returned status code (redaction feature only)
    #[cfg(feature = "redaction")]
    pub(super) response_description: Option<String>,
    /// Whether to skip collection for OpenAPI documentation (default: false)
    pub(super) skip_collection: bool,
    /// Security requirements for this operation (None = inherit from global)
    pub(super) security: Option<Vec<SecurityRequirement>>,
}
