use std::collections::BTreeSet;
use std::mem;

use http::{Method, Uri};
use utoipa::openapi::{Components, Info, OpenApi, Paths, Server, Tag};

mod builder;
use crate::client::openapi::channel::{CollectorHandle, CollectorMessage};
use crate::client::openapi::schema::Schemas;

pub use self::builder::ApiClientBuilder;

mod call;
pub use self::call::ApiCall;

mod parameters;
pub use self::parameters::{
    CallBody, CallCookies, CallHeaders, CallPath, CallQuery, ParamStyle, ParamValue, ParameterValue,
};

mod response;
pub use self::response::ExpectedStatusCodes;
#[cfg(feature = "redaction")]
pub use self::response::{
    RedactOptions, RedactedResult, RedactionBuilder, Redactor, ValueRedactionBuilder, redact_value,
};

mod auth;
pub use self::auth::{Authentication, AuthenticationError, SecureString};

#[cfg(feature = "oauth2")]
pub mod oauth2;
#[cfg(feature = "oauth2")]
pub use self::oauth2::{OAuth2Config, OAuth2ConfigBuilder, OAuth2Error, OAuth2Token};

mod security;
pub use self::security::{
    ApiKeyLocation, OAuth2Flow, OAuth2Flows, OAuth2ImplicitFlow, SecurityRequirement,
    SecurityScheme,
};

mod call_parameters;

mod openapi;
// CallResult, RawResult, and RawBody are public API, but CalledOperation and Collectors are internal
pub use self::openapi::{CallResult, RawBody, RawResult};

mod error;
pub use self::error::ApiClientError;

#[cfg(test)]
mod integration_tests;

/// A type-safe HTTP client for API testing and OpenAPI documentation generation.
///
/// `ApiClient` is the core component of clawspec that enables you to make HTTP requests
/// while automatically capturing request/response schemas for OpenAPI specification generation.
/// It provides a fluent API for building requests with comprehensive parameter support,
/// status code validation, and automatic schema collection.
///
/// # Key Features
///
/// - **Test-Driven Documentation**: Automatically generates OpenAPI specifications from test execution
/// - **Type Safety**: Compile-time guarantees for API parameters and response types
/// - **Flexible Status Code Validation**: Support for ranges, specific codes, and custom patterns
/// - **Comprehensive Parameter Support**: Path, query, and header parameters with multiple styles
/// - **Request Body Formats**: JSON, form-encoded, multipart, and raw binary data
/// - **Schema Collection**: Automatic detection and collection of request/response schemas
/// - **OpenAPI Metadata**: Configurable API info, servers, and operation documentation
///
/// # Basic Usage
///
/// ```rust,no_run
/// use clawspec_core::ApiClient;
/// use serde::{Deserialize, Serialize};
/// use utoipa::ToSchema;
///
/// #[derive(Debug, Deserialize, ToSchema)]
/// struct User {
///     id: u32,
///     name: String,
///     email: String,
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create an API client
///     let mut client = ApiClient::builder()
///         .with_host("api.example.com")
///         .with_base_path("/v1")?
///         .build()?;
///
///     // Make a request and capture the schema
///     let user: User = client
///         .get("/users/123")?
///
///         .await?
///         .as_json()
///         .await?;
///
///     println!("User: {:?}", user);
///
///     // Generate OpenAPI specification from collected data
///     let openapi_spec = client.collected_openapi().await;
///     let yaml = serde_saphyr::to_string(&openapi_spec)?;
///     println!("{yaml}");
///
///     Ok(())
/// }
/// ```
///
/// # Builder Pattern
///
/// The client is created using a builder pattern. For simple cases, use the simplified methods:
///
/// ```rust
/// use clawspec_core::ApiClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = ApiClient::builder()
///     .with_https()
///     .with_host("api.github.com")
///     .with_port(443)
///     .with_base_path("/api/v3")?
///     .with_info_simple("GitHub API Client", "1.0.0")
///     .with_description("Auto-generated from tests")
///     .add_server_simple("https://api.github.com/api/v3", "GitHub API v3")
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// For advanced configuration, use the builder types (re-exported from clawspec_core):
///
/// ```rust
/// use clawspec_core::{ApiClient, InfoBuilder, ServerBuilder};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = ApiClient::builder()
///     .with_https()
///     .with_host("api.github.com")
///     .with_port(443)
///     .with_base_path("/api/v3")?
///     .with_info(
///         InfoBuilder::new()
///             .title("GitHub API Client")
///             .version("1.0.0")
///             .description(Some("Auto-generated from tests"))
///             .build()
///     )
///     .add_server(
///         ServerBuilder::new()
///             .url("https://api.github.com/api/v3")
///             .description(Some("GitHub API v3"))
///             .build()
///     )
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// # Making Requests
///
/// The client supports all standard HTTP methods with a fluent API:
///
/// ```rust
/// use clawspec_core::{ApiClient, expected_status_codes, CallQuery, CallHeaders, ParamValue};
/// use serde::{Serialize, Deserialize};
/// use utoipa::ToSchema;
///
/// #[derive(Serialize, Deserialize, ToSchema)]
/// struct UserData { name: String }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
/// let user_data = UserData { name: "John".to_string() };
///
/// // GET request with query parameters and headers
/// let users = client
///     .get("/users")?
///     .with_query(
///         CallQuery::new()
///             .add_param("page", ParamValue::new(1))
///             .add_param("per_page", ParamValue::new(50))
///     )
///     .with_header("Authorization", "Bearer token123")
///     .with_expected_status_codes(expected_status_codes!(200, 404))
///
///     .await?
///     .as_json::<Vec<UserData>>()
///     .await?;
///
/// // POST request with JSON body
/// let new_user = client
///     .post("/users")?
///     .json(&user_data)?
///     .with_expected_status_codes(expected_status_codes!(201, 409))
///
///     .await?
///     .as_json::<UserData>()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// # Schema Registration
///
/// For types that aren't automatically detected, you can manually register them:
///
/// ```rust
/// use clawspec_core::{ApiClient, register_schemas};
/// # use utoipa::ToSchema;
/// # use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// struct ErrorType { message: String }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// struct NestedType { value: i32 }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
///
/// // Register multiple schemas at once
/// register_schemas!(client, ErrorType, NestedType);
///
/// // Or register individually
/// client.register_schema::<ErrorType>().await;
/// # Ok(())
/// # }
/// ```
///
/// # OpenAPI Generation
///
/// The client automatically collects information during test execution and can generate
/// comprehensive OpenAPI specifications:
///
/// ```rust
/// # use clawspec_core::ApiClient;
/// # use serde::{Serialize, Deserialize};
/// # use utoipa::ToSchema;
/// # #[derive(Serialize, Deserialize, ToSchema)]
/// # struct UserData { name: String }
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder().build()?;
/// let user_data = UserData { name: "John".to_string() };
///
/// // Make some API calls...
/// client.get("/users")?.await?.as_json::<Vec<UserData>>().await?;
/// client.post("/users")?.json(&user_data)?.await?.as_json::<UserData>().await?;
///
/// // Generate OpenAPI specification
/// let openapi = client.collected_openapi().await;
///
/// // Convert to YAML or JSON
/// let yaml = serde_saphyr::to_string(&openapi)?;
/// let json = serde_json::to_string_pretty(&openapi)?;
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// The client provides comprehensive error handling for various scenarios:
///
/// ```rust
/// use clawspec_core::{ApiClient, ApiClientError};
///
/// # async fn example() -> Result<(), ApiClientError> {
/// let mut client = ApiClient::builder().build()?;
///
/// match client.get("/users/999")?.await {
///     Ok(response) => {
///         // Handle successful response
///         println!("Success!");
///     }
///     Err(ApiClientError::UnexpectedStatusCode { status_code, body }) => {
///         // Handle HTTP errors
///         println!("HTTP {} error: {}", status_code, body);
///     }
///     Err(ApiClientError::ReqwestError(source)) => {
///         // Handle network/request errors
///         println!("Request failed: {}", source);
///     }
///     Err(err) => {
///         // Handle other errors
///         println!("Other error: {}", err);
///     }
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Thread Safety
///
/// `ApiClient` is designed to be safe to use across multiple threads. The internal schema
/// collection is protected by async locks, allowing concurrent request execution while
/// maintaining data consistency.
///
/// # Performance Considerations
///
/// - Schema collection has minimal runtime overhead
/// - Request bodies are streamed when possible
/// - Response processing is lazy - schemas are only collected when responses are consumed
/// - Internal caching reduces redundant schema processing
use indexmap::IndexMap;

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_uri: Uri,
    base_path: String,
    info: Option<Info>,
    servers: Vec<Server>,
    collector_handle: CollectorHandle,
    authentication: Option<Authentication>,
    security_schemes: IndexMap<String, SecurityScheme>,
    default_security: Vec<SecurityRequirement>,
}

// Create
impl ApiClient {
    pub fn builder() -> ApiClientBuilder {
        ApiClientBuilder::default()
    }
}

// Collected
impl ApiClient {
    pub async fn collected_paths(&mut self) -> Paths {
        let mut builder = Paths::builder();
        let mut collectors = self.collector_handle.get_collectors().await;
        for (path, item) in collectors.as_map(&self.base_path) {
            builder = builder.path(path, item);
        }
        mem::drop(collectors);

        builder.build()
    }

    /// Generates a complete OpenAPI specification from collected request/response data.
    ///
    /// This method aggregates all the information collected during API calls and produces
    /// a comprehensive OpenAPI 3.1 specification including paths, components, schemas,
    /// operation metadata, and server information.
    ///
    /// # Features
    ///
    /// - **Automatic Path Collection**: All endpoint calls are automatically documented
    /// - **Schema Generation**: Request/response schemas are extracted from Rust types
    /// - **Operation Metadata**: Includes operation IDs, descriptions, and tags
    /// - **Server Information**: Configurable server URLs and descriptions
    /// - **Tag Collection**: Automatically computed from all operations
    /// - **Component Schemas**: Reusable schema definitions with proper references
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{ApiClient, ToSchema};
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize, ToSchema)]
    /// struct UserData { name: String }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder()
    ///     .with_host("api.example.com")
    ///     .with_info_simple("My API", "1.0.0")
    ///     .add_server_simple("https://api.example.com", "Production server")
    ///     .build()?;
    ///
    /// let user_data = UserData { name: "John".to_string() };
    ///
    /// // Make some API calls to collect data
    /// client.get("/users")?.await?.as_json::<Vec<UserData>>().await?;
    /// client.post("/users")?.json(&user_data)?.await?.as_json::<UserData>().await?;
    ///
    /// // Generate complete OpenAPI specification
    /// let openapi = client.collected_openapi().await;
    ///
    /// // The generated spec includes:
    /// // - API info (title, version, description)
    /// // - Server definitions
    /// // - All paths with operations
    /// // - Component schemas
    /// // - Computed tags from operations
    ///
    /// // Export to different formats
    /// let yaml = serde_saphyr::to_string(&openapi)?;
    /// let json = serde_json::to_string_pretty(&openapi)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Generated Content
    ///
    /// The generated OpenAPI specification includes:
    ///
    /// - **Info**: API metadata (title, version, description) if configured
    /// - **Servers**: Server URLs and descriptions if configured
    /// - **Paths**: All documented endpoints with operations
    /// - **Components**: Reusable schema definitions
    /// - **Tags**: Automatically computed from operation tags
    ///
    /// # Tag Generation
    ///
    /// Tags are automatically computed from all operations and include:
    /// - Explicit tags set on operations
    /// - Auto-generated tags based on path patterns
    /// - Deduplicated and sorted alphabetically
    ///
    /// # Performance Notes
    ///
    /// - This method acquires read locks on internal collections
    /// - Schema processing is cached to avoid redundant work
    /// - Tags are computed on-demand from operation metadata
    pub async fn collected_openapi(&mut self) -> OpenApi {
        let mut builder = OpenApi::builder();

        // Add API info if configured
        if let Some(ref info) = self.info {
            builder = builder.info(info.clone());
        }

        // Add servers if configured
        if !self.servers.is_empty() {
            builder = builder.servers(Some(self.servers.clone()));
        }

        // Add paths
        builder = builder.paths(self.collected_paths().await);

        // Add components with schemas and security schemes
        let collectors = self.collector_handle.get_collectors().await;
        let mut components_builder = Components::builder().schemas_from_iter(collectors.schemas());

        // Add security schemes to components
        for (name, scheme) in &self.security_schemes {
            components_builder = components_builder.security_scheme(name, scheme.to_utoipa());
        }

        let components = components_builder.build();

        // Compute tags from all operations
        let tags = self.compute_tags(&collectors).await;
        mem::drop(collectors);

        let builder = builder.components(Some(components));

        // Add computed tags if any exist
        let builder = if tags.is_empty() {
            builder
        } else {
            builder.tags(Some(tags))
        };

        // Add default security requirements if configured
        let builder = if self.default_security.is_empty() {
            builder
        } else {
            let security: Vec<_> = self
                .default_security
                .iter()
                .map(SecurityRequirement::to_utoipa)
                .collect();
            builder.security(Some(security))
        };

        builder.build()
    }

    /// Computes the list of unique tags from all collected operations.
    async fn compute_tags(&self, collectors: &openapi::Collectors) -> Vec<Tag> {
        let mut tag_names = BTreeSet::new();

        // Collect all unique tag names from operations
        for operation in collectors.operations() {
            if let Some(tags) = operation.tags() {
                for tag in tags {
                    tag_names.insert(tag.clone());
                }
            }
        }

        // Convert to Tag objects
        tag_names.into_iter().map(Tag::new).collect()
    }

    /// Manually registers a type in the schema collection.
    ///
    /// This method allows you to explicitly add types to the OpenAPI schema collection
    /// that might not be automatically detected. This is useful for types that are
    /// referenced indirectly, such as nested types.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to register, must implement `ToSchema` and `'static`
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// # use utoipa::ToSchema;
    /// # use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    /// struct NestedErrorType {
    ///     message: String,
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // Register the nested type that might not be automatically detected
    /// client.register_schema::<NestedErrorType>().await;
    ///
    /// // Now when you generate the OpenAPI spec, NestedErrorType will be included
    /// let openapi = client.collected_openapi().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_schema<T>(&mut self)
    where
        T: utoipa::ToSchema + 'static,
    {
        let mut schemas = Schemas::default();
        schemas.add::<T>();

        self.collector_handle
            .sender()
            .send(CollectorMessage::AddSchemas(schemas))
            .await;
    }
}

impl ApiClient {
    pub fn call(&self, method: Method, path: CallPath) -> Result<ApiCall, ApiClientError> {
        // Convert default_security to Option only if not empty
        let default_security = if self.default_security.is_empty() {
            None
        } else {
            Some(self.default_security.clone())
        };

        ApiCall::build(
            self.client.clone(),
            self.base_uri.clone(),
            self.collector_handle.sender(),
            method,
            path,
            self.authentication.clone(),
            default_security,
        )
    }

    pub fn get(&self, path: impl Into<CallPath>) -> Result<ApiCall, ApiClientError> {
        self.call(Method::GET, path.into())
    }

    pub fn post(&self, path: impl Into<CallPath>) -> Result<ApiCall, ApiClientError> {
        self.call(Method::POST, path.into())
    }

    pub fn put(&self, path: impl Into<CallPath>) -> Result<ApiCall, ApiClientError> {
        self.call(Method::PUT, path.into())
    }

    pub fn delete(&self, path: impl Into<CallPath>) -> Result<ApiCall, ApiClientError> {
        self.call(Method::DELETE, path.into())
    }

    pub fn patch(&self, path: impl Into<CallPath>) -> Result<ApiCall, ApiClientError> {
        self.call(Method::PATCH, path.into())
    }
}
