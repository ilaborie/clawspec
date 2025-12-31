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

/// HTTP client for API testing with automatic OpenAPI schema collection.
///
/// `ApiClient` captures request/response schemas during test execution to generate
/// OpenAPI specifications. Use [`ApiClientBuilder`] to create instances.
///
/// # Example
///
/// ```rust,no_run
/// use clawspec_core::ApiClient;
/// # use serde::Deserialize;
/// # use utoipa::ToSchema;
/// # #[derive(Deserialize, ToSchema)]
/// # struct User { id: u32, name: String }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = ApiClient::builder()
///     .with_host("api.example.com")
///     .build()?;
///
/// // Schemas are captured automatically
/// let user: User = client.get("/users/123")?.await?.as_json().await?;
///
/// // Generate OpenAPI spec
/// let spec = client.collected_openapi().await;
/// # Ok(())
/// # }
/// ```
///
/// See the [crate documentation](crate) for detailed usage and the
/// [Tutorial](crate::_tutorial) for a step-by-step guide.
///
/// # Thread Safety
///
/// Schema collection is protected by async locks, allowing concurrent request execution.
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
