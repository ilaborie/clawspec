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
