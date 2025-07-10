use std::sync::Arc;

use headers::HeaderMapExt;
use http::header::{HeaderName, HeaderValue};
use http::{Method, Uri};
use reqwest::{Body, Request};
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::debug;
use url::Url;
use utoipa::ToSchema;

use super::collectors::{CalledOperation, Collectors};
use super::path::PathResolved;
use super::{ApiClientError, CallBody, CallHeaders, CallPath, CallQuery, CallResult};

// TODO: Add comprehensive documentation for all public APIs - https://github.com/ilaborie/clawspec/issues/34
// TODO: Standardize builder patterns for consistency - https://github.com/ilaborie/clawspec/issues/33
#[derive(derive_more::Debug)]
pub struct ApiCall {
    client: reqwest::Client,
    base_uri: Uri,
    collectors: Arc<RwLock<Collectors>>,

    operation_id: String,
    method: Method,
    path: CallPath,
    query: CallQuery,
    headers: Option<CallHeaders>,

    #[debug(ignore)]
    body: Option<CallBody>,
    // TODO auth - https://github.com/ilaborie/clawspec/issues/17
    // TODO cookiess - https://github.com/ilaborie/clawspec/issues/18
}

impl ApiCall {
    pub(super) fn build(
        client: reqwest::Client,
        base_uri: Uri,
        collectors: Arc<RwLock<Collectors>>,
        method: Method,
        path: CallPath,
    ) -> Result<Self, ApiClientError> {
        let operation_id = slug::slugify(format!("{method} {}", path.path));

        let result = Self {
            client,
            base_uri,
            collectors,
            operation_id,
            method,
            path,
            query: CallQuery::default(),
            headers: None,
            body: None,
        };
        Ok(result)
    }
}

// Builder
impl ApiCall {
    pub fn operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = operation_id.into();
        self
    }

    pub fn query(mut self, query: CallQuery) -> Self {
        self.query = query;
        self
    }

    pub fn headers(mut self, headers: Option<CallHeaders>) -> Self {
        self.headers = match (self.headers.take(), headers) {
            (Some(existing), Some(new)) => Some(existing.merge(new)),
            (existing, new) => existing.or(new),
        };
        self
    }

    /// Adds headers to the API call, merging with any existing headers.
    ///
    /// This is a convenience method that automatically wraps the headers in Some().
    pub fn with_headers(self, headers: CallHeaders) -> Self {
        self.headers(Some(headers))
    }

    pub fn json<T>(mut self, t: &T) -> Result<Self, ApiClientError>
    where
        T: Serialize + ToSchema + 'static,
    {
        let body = CallBody::json(t)?;
        self.body = Some(body);
        Ok(self)
    }
    // TODO more generic bodies - https://github.com/ilaborie/clawspec/issues/19
}

// Call
impl ApiCall {
    // XXX code to abstract if we want multiple client
    pub async fn exchange(self) -> Result<CallResult, ApiClientError> {
        let Self {
            client,
            base_uri,
            collectors,
            operation_id,
            method,
            path,
            query,
            headers,
            body,
        } = self;

        // Build URL and request
        let url = Self::build_url(&base_uri, &path, &query)?;
        let request = Self::build_request(method.clone(), url, &headers, &body)?;

        // Create operation for OpenAPI documentation
        let mut operation = Self::build_operation(
            &operation_id,
            &method,
            &path,
            query.clone(),
            &headers,
            &body,
        );

        // Execute HTTP request
        debug!(?request, "sending...");
        let response = client.execute(request).await?;
        debug!(?response, "...receiving");
        // TODO fail if status code is not accepted (default: 200-400) - https://github.com/ilaborie/clawspec/issues/22

        // Process response and collect schemas
        let call_result = CallResult::new(operation_id, Arc::clone(&collectors), response).await?;
        operation.add_response(call_result.clone());

        Self::collect_schemas_and_operation(collectors, &path, &headers, operation).await;

        Ok(call_result)
    }

    fn build_url(
        base_uri: &Uri,
        path: &CallPath,
        query: &CallQuery,
    ) -> Result<Url, ApiClientError> {
        let path_resolved = PathResolved::try_from(path.clone())?;
        let url = format!("{base_uri}/{}", path_resolved.path.trim_start_matches('/'));
        let mut url = url.parse::<Url>()?;

        if !query.is_empty() {
            let query_string = query.to_query_string()?;
            url.set_query(Some(&query_string));
        }

        Ok(url)
    }

    fn build_request(
        method: Method,
        url: Url,
        headers: &Option<CallHeaders>,
        body: &Option<CallBody>,
    ) -> Result<Request, ApiClientError> {
        let mut request = Request::new(method, url);
        let req_headers = request.headers_mut();

        // Add custom headers
        if let Some(headers) = headers {
            for (name, value) in headers.to_http_headers()? {
                req_headers.insert(
                    HeaderName::from_bytes(name.as_bytes())?,
                    HeaderValue::from_str(&value)?,
                );
            }
        }

        // Set body
        if let Some(body) = body {
            req_headers.typed_insert(body.content_type.clone());
            let req_body = request.body_mut();
            *req_body = Some(Body::from(body.data.clone()));
        }

        Ok(request)
    }

    fn build_operation(
        operation_id: &str,
        method: &Method,
        path: &CallPath,
        query: CallQuery,
        headers: &Option<CallHeaders>,
        body: &Option<CallBody>,
    ) -> CalledOperation {
        CalledOperation::build(
            operation_id.to_string(),
            method.clone(),
            &path.path,
            path,
            query,
            headers.as_ref(),
            body.as_ref(),
        )
    }

    async fn collect_schemas_and_operation(
        collectors: Arc<RwLock<Collectors>>,
        path: &CallPath,
        headers: &Option<CallHeaders>,
        operation: CalledOperation,
    ) {
        let mut cs = collectors.write().await;
        cs.collect_schemas(path.schemas().clone());
        if let Some(headers) = headers {
            cs.collect_schemas(headers.schemas().clone());
        }
        cs.collect_operation(operation);
    }
}
