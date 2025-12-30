use std::future::{Future, IntoFuture};
use std::pin::Pin;

use headers::HeaderMapExt;
use http::header::{HeaderName, HeaderValue};
use http::{Method, Uri};
use reqwest::{Body, Request};
use tracing::debug;
use url::Url;

use super::{ApiCall, BODY_MAX_LENGTH, CollectorSender};
use crate::client::call_parameters::{CallParameters, OperationMetadata};
use crate::client::openapi::CalledOperation;
use crate::client::openapi::channel::CollectorMessage;
use crate::client::parameters::PathResolved;
use crate::client::response::ExpectedStatusCodes;
use crate::client::{ApiClientError, CallBody, CallPath, CallQuery, CallResult};

impl ApiCall {
    pub(in crate::client) fn build(
        client: reqwest::Client,
        base_uri: Uri,
        collector_sender: CollectorSender,
        method: Method,
        path: CallPath,
        authentication: Option<crate::client::Authentication>,
        default_security: Option<Vec<crate::client::security::SecurityRequirement>>,
    ) -> Result<Self, ApiClientError> {
        let operation_id = slug::slugify(format!("{method} {}", path.path));

        let result = Self {
            client,
            base_uri,
            collector_sender,
            method,
            path,
            query: CallQuery::default(),
            headers: None,
            body: None,
            authentication,
            cookies: None,
            expected_status_codes: ExpectedStatusCodes::default(),
            metadata: OperationMetadata {
                operation_id,
                tags: None,
                description: None,
                response_description: None,
            },
            response_description: None,
            skip_collection: false,
            security: default_security,
        };
        Ok(result)
    }
}

impl ApiCall {
    /// Executes the HTTP request and returns a result that must be consumed for OpenAPI generation.
    ///
    /// This method sends the configured HTTP request to the server and returns a [`CallResult`]
    /// that contains the response. **You must call one of the response processing methods**
    /// on the returned `CallResult` to ensure proper OpenAPI documentation generation.
    ///
    /// # ⚠️ Important: Response Consumption Required
    ///
    /// Simply calling `exchange()` is not sufficient! You must consume the [`CallResult`] by
    /// calling one of these methods:
    ///
    /// - [`CallResult::as_empty()`] - For empty responses (204 No Content, DELETE operations, etc.)
    /// - [`CallResult::as_json::<T>()`] - For JSON responses that should be deserialized
    /// - [`CallResult::as_text()`] - For plain text responses
    /// - [`CallResult::as_bytes()`] - For binary responses
    /// - [`CallResult::as_raw()`] - For complete raw response access (status, content-type, body)
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// # use serde::Deserialize;
    /// # use utoipa::ToSchema;
    /// # #[derive(Deserialize, ToSchema)]
    /// # struct User { id: u32, name: String }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = ApiClient::builder().build()?;
    ///
    /// // ✅ CORRECT: Always consume the result
    /// let user: User = client
    ///     .get("/users/123")?
    ///     .await?
    ///     .as_json()  // ← Required for OpenAPI generation!
    ///     .await?;
    ///
    /// // ✅ CORRECT: For operations returning empty responses
    /// client
    ///     .delete("/users/123")?
    ///     .await?
    ///     .as_empty()  // ← Required for OpenAPI generation!
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network issues, timeouts, etc.)
    /// - The response status code is not in the expected range
    /// - Request building fails (invalid URLs, malformed headers, etc.)
    ///
    /// # OpenAPI Documentation
    ///
    /// This method automatically collects operation metadata for OpenAPI generation,
    /// but the response schema and examples are only captured when the [`CallResult`]
    /// is properly consumed with one of the `as_*` methods.
    // TODO: Abstract client implementation to support multiple clients - https://github.com/ilaborie/clawspec/issues/78
    async fn exchange(self) -> Result<CallResult, ApiClientError> {
        let Self {
            client,
            base_uri,
            collector_sender,
            method,
            path,
            query,
            headers,
            body,
            authentication,
            cookies,
            expected_status_codes,
            metadata,
            response_description,
            skip_collection,
            security,
        } = self;

        // Build URL and request
        let url = Self::build_url(&base_uri, &path, &query)?;
        let parameters = CallParameters::with_all(query.clone(), headers.clone(), cookies.clone());
        let request =
            Self::build_request(method.clone(), url, &parameters, &body, &authentication)?;

        // Create operation for OpenAPI documentation
        let operation_id = metadata.operation_id.clone();
        let mut operation = Self::build_operation(
            metadata,
            &method,
            &path,
            parameters.clone(),
            &body,
            response_description,
            security,
        );

        // Execute HTTP request
        debug!(?request, "sending...");
        let response = client.execute(request).await?;
        debug!(?response, "...receiving");

        // Validate status code
        let status_code = response.status().as_u16();
        if !expected_status_codes.contains(status_code) {
            // Get the body only if status code is unexpected
            let body = response
                .text()
                .await
                .map(|text| {
                    if text.len() > BODY_MAX_LENGTH {
                        format!("{}... (truncated)", &text[..1024])
                    } else {
                        text
                    }
                })
                .unwrap_or_else(|e| format!("<unable to read response body: {e}>"));
            return Err(ApiClientError::UnexpectedStatusCode { status_code, body });
        }

        // Process response and collect schemas (only if collection is enabled)
        let call_result = if skip_collection {
            CallResult::new_without_collection(response).await?
        } else {
            let call_result =
                CallResult::new(operation_id, collector_sender.clone(), response).await?;
            operation.add_response(call_result.clone());
            Self::collect_schemas_and_operation(
                &collector_sender,
                &path,
                &parameters,
                &body,
                operation,
            )
            .await;
            call_result
        };

        Ok(call_result)
    }

    pub(super) fn build_url(
        base_uri: &Uri,
        path: &CallPath,
        query: &CallQuery,
    ) -> Result<Url, ApiClientError> {
        let path_resolved = PathResolved::try_from(path.clone())?;
        let base_uri = base_uri.to_string();
        let url = format!(
            "{}/{}",
            base_uri.trim_end_matches('/'),
            path_resolved.path.trim_start_matches('/')
        );
        let mut url = url.parse::<Url>()?;

        if !query.is_empty() {
            let query_string = query.to_query_string()?;
            url.set_query(Some(&query_string));
        }

        Ok(url)
    }

    pub(super) fn build_request(
        method: Method,
        url: Url,
        parameters: &CallParameters,
        body: &Option<CallBody>,
        authentication: &Option<crate::client::Authentication>,
    ) -> Result<Request, ApiClientError> {
        let mut request = Request::new(method, url);
        let req_headers = request.headers_mut();

        // Add authentication header if present
        if let Some(auth) = authentication {
            let (header_name, header_value) = auth.to_header()?;
            req_headers.insert(header_name, header_value);
        }

        // Add custom headers
        for (name, value) in parameters.to_http_headers()? {
            req_headers.insert(
                HeaderName::from_bytes(name.as_bytes())?,
                HeaderValue::from_str(&value)?,
            );
        }

        // Add cookies as Cookie header
        let cookie_header = parameters.to_cookie_header()?;
        if !cookie_header.is_empty() {
            req_headers.insert(
                HeaderName::from_static("cookie"),
                HeaderValue::from_str(&cookie_header)?,
            );
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
        metadata: OperationMetadata,
        method: &Method,
        path: &CallPath,
        parameters: CallParameters,
        body: &Option<CallBody>,
        response_description: Option<String>,
        security: Option<Vec<crate::client::security::SecurityRequirement>>,
    ) -> CalledOperation {
        let OperationMetadata {
            operation_id,
            tags,
            description,
            response_description: _,
        } = metadata;

        CalledOperation::build(
            method.clone(),
            &path.path,
            path,
            parameters,
            body.as_ref(),
            OperationMetadata {
                operation_id: operation_id.to_string(),
                tags,
                description,
                response_description,
            },
            security,
        )
    }

    async fn collect_schemas_and_operation(
        sender: &CollectorSender,
        path: &CallPath,
        parameters: &CallParameters,
        body: &Option<CallBody>,
        operation: CalledOperation,
    ) {
        // Send path schemas
        sender
            .send(CollectorMessage::AddSchemas(path.schemas().clone()))
            .await;

        // Send parameter schemas
        sender
            .send(CollectorMessage::AddSchemas(parameters.collect_schemas()))
            .await;

        // Send body schema entry if present
        if let Some(body) = body {
            sender
                .send(CollectorMessage::AddSchemaEntry(body.entry.clone()))
                .await;
        }

        // Register the operation
        sender
            .send(CollectorMessage::RegisterOperation(operation))
            .await;
    }
}

/// Implement IntoFuture for ApiCall to enable direct .await syntax
///
/// This provides a more ergonomic API by allowing direct `.await` on ApiCall:
/// ```rust,no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut client = clawspec_core::ApiClient::builder().build()?;
/// let response = client.get("/users")?.await?;
/// # Ok(())
/// # }
/// ```
impl IntoFuture for ApiCall {
    type Output = Result<CallResult, ApiClientError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.exchange())
    }
}
