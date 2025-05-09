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
    ApiClientError, CallBody, CallHeaders, CallPath, CallQuery, CalledOperation, Collectors,
    PathResolved,
};

#[derive(derive_more::Debug)]
pub struct ApiCall {
    client: reqwest::Client,
    base_uri: Uri,
    collectors: Arc<RwLock<Collectors>>,

    operation_id: String,
    method: Method,
    path: (String, PathResolved),
    query: Option<CallQuery>,
    headers: Option<CallHeaders>,

    #[debug(ignore)]
    body: Option<CallBody>,
    // TODO auth
    // TODO cookiess
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
            query: None,
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

    pub fn query(mut self, query: Option<CallQuery>) -> Self {
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
    // TODO more generic bodies
}

// Call
impl ApiCall {
    // XXX code to abstract if we want multiple client
    pub async fn exchange(self) -> Result<CalledOperation, ApiClientError> {
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

        // Create opration
        let mut operation = CalledOperation::build(
            operation_id,
            method.clone(),
            &path_name,
            &params,
            query.as_ref(),
            headers.as_ref(),
            body.as_ref(),
        );
        operation.schemas.merge(schemas);

        // Build URL
        let url = format!("{}/{}", base_uri, path.trim_start_matches('/'));
        let url = url.parse::<Url>()?;

        // TODO append query in url

        // Build request
        let mut request = Request::new(method, url);
        let req_headers = request.headers_mut();
        // TODO append headers

        // Set body
        if let Some(body) = body {
            req_headers.typed_insert(body.content_type.clone());
            let req_body = request.body_mut();
            *req_body = Some(Body::from(body.data));
        }

        // Call
        let response = client.execute(request).await?;

        // TODO fail if status code is not accepted (default: 200-400)

        // Parse response
        operation.add_response(response).await?;

        // collect
        let mut cs = collectors.write().await;
        cs.collect_operation(operation.clone());
        mem::drop(cs);

        Ok(operation)
    }
}

// mod fut {
//     use std::pin::Pin;
//     use std::task::{Context, Poll};

//     use crate::ApiClientError;
//     use crate::client::CalledOperation;

//     use super::ApiCall;

//     impl Future for ApiCall {
//         type Output = Result<CalledOperation, ApiClientError>;

//         fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//             self.exchange();
//             todo!()
//         }
//     }
// }
