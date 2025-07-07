use std::mem;
use std::sync::Arc;

use headers::HeaderMapExt;
use http::{Method, Uri};
use reqwest::{Body, Request};
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::debug;
use url::Url;
use utoipa::ToSchema;

use super::{
    ApiClientError, CallBody, CallHeaders, CallPath, CallQuery, CallResult, CalledOperation,
    Collectors, PathResolved,
};

// TODO: Add comprehensive documentation for all public APIs - https://github.com/ilaborie/clawspec/issues/34
// TODO: Standardize builder patterns for consistency - https://github.com/ilaborie/clawspec/issues/33
#[derive(derive_more::Debug)]
pub struct ApiCall {
    client: reqwest::Client,
    base_uri: Uri,
    collectors: Arc<RwLock<Collectors>>,

    operation_id: String,
    method: Method,
    path: (String, PathResolved),
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
        let initial = path.path.clone();
        let operation_id = slug::slugify(format!("{method} {initial}"));
        let path_resolved = PathResolved::try_from(path)?;

        let result = Self {
            client,
            base_uri,
            collectors,
            operation_id,
            method,
            path: (initial, path_resolved),
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
        let headers = match (self.headers.clone(), headers) {
            (Some(h1), Some(h2)) => Some(h1.merge(h2)),
            (Some(h), None) | (None, Some(h)) => Some(h),
            (None, None) => None,
        };
        self.headers = headers;
        self
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

        // Handle path
        let (path_name, path_resoloved) = path;
        let PathResolved {
            path,
            params,
            schemas,
        } = path_resoloved;

        // Build URL
        let url = format!("{}/{}", base_uri, path.trim_start_matches('/'));
        let mut url = url.parse::<Url>()?;

        // Append query parameters to URL
        if !query.is_empty() {
            let query_string = query.to_query_string()?;
            url.set_query(Some(&query_string));
        }

        // Create opration
        let mut operation = CalledOperation::build(
            operation_id.clone(),
            method.clone(),
            &path_name,
            &params,
            query,
            headers.as_ref(),
            body.as_ref(),
        );

        // Build request
        let mut request = Request::new(method, url);
        let req_headers = request.headers_mut();
        // TODO append headers - https://github.com/ilaborie/clawspec/issues/20

        // Set body
        if let Some(body) = body {
            req_headers.typed_insert(body.content_type.clone());
            let req_body = request.body_mut();
            *req_body = Some(Body::from(body.data));
        }

        // Call
        let response = client.execute(request).await?;

        // TODO fail if status code is not accepted (default: 200-400) - https://github.com/ilaborie/clawspec/issues/22

        // Parse response
        let call_result = CallResult::new(operation_id, Arc::clone(&collectors), response).await?;
        operation.add_response(call_result.clone());

        // collect operation
        let mut cs = collectors.write().await;
        cs.collect_schemas(schemas);
        cs.collect_operation(operation);
        mem::drop(cs);

        Ok(call_result)
    }
}
