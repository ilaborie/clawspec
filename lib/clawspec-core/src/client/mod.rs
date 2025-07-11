use std::mem;
use std::sync::Arc;

use http::{Method, Uri};
use tokio::sync::RwLock;
use utoipa::openapi::{Components, OpenApi, Paths};

mod builder;
pub use self::builder::ApiClientBuilder;

mod call;
pub use self::call::{ApiCall, ExpectedStatusCodes};

mod param;
pub use self::param::{ParamStyle, ParamValue, ParameterValue};

mod path;
pub use self::path::CallPath;

mod query;
pub use self::query::CallQuery;

mod headers;
pub use self::headers::CallHeaders;

mod body;
pub use self::body::CallBody;

mod schema;

mod error;
pub use self::error::ApiClientError;

mod collectors;
// CallResult is public API, but CalledOperation and Collectors are internal
pub use self::collectors::CallResult;

#[cfg(test)]
mod integration_tests;

mod output;

// TODO: Add comprehensive documentation for all public APIs - https://github.com/ilaborie/clawspec/issues/34
#[derive(Debug, Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_uri: Uri,
    base_path: String,
    collectors: Arc<RwLock<collectors::Collectors>>,
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
        let mut collectors = self.collectors.write().await;
        for (path, item) in collectors.as_map(&self.base_path) {
            builder = builder.path(path, item);
        }
        mem::drop(collectors);

        builder.build()
    }

    pub async fn collected_openapi(&mut self) -> OpenApi {
        let mut builder = OpenApi::builder();

        // Path
        builder = builder.paths(self.collected_paths().await);

        // Components
        let collectors = self.collectors.read().await;
        let components = Components::builder()
            .schemas_from_iter(collectors.schemas())
            .build();
        mem::drop(collectors);
        let builder = builder.components(Some(components));

        builder.build()
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
        let mut schemas = schema::Schemas::default();
        schemas.add::<T>();

        let mut collectors = self.collectors.write().await;
        collectors.collect_schemas(schemas);
    }
}

impl ApiClient {
    pub fn call(&self, method: Method, path: CallPath) -> Result<ApiCall, ApiClientError> {
        ApiCall::build(
            self.client.clone(),
            self.base_uri.clone(),
            Arc::clone(&self.collectors),
            method,
            path,
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
