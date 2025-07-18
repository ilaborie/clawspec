/// A collection of all HTTP parameters (query, headers, cookies) for an API call.
///
/// This struct groups together query parameters, headers, and cookies to reduce
/// the number of arguments passed between functions and improve code organization.
#[derive(Debug, Clone, Default)]
pub struct CallParameters {
    pub(super) query: super::CallQuery,
    pub(super) headers: Option<super::CallHeaders>,
    pub(super) cookies: Option<super::CallCookies>,
}

/// Metadata for an OpenAPI operation.
///
/// This struct contains optional metadata that can be associated with an API operation
/// to provide additional context in the generated OpenAPI specification.
#[derive(Debug, Clone, Default)]
pub(super) struct OperationMetadata {
    pub(super) operation_id: String,
    pub(super) tags: Option<Vec<String>>,
    pub(super) description: Option<String>,
    pub(super) response_description: Option<String>,
}

impl CallParameters {
    /// Creates CallParameters with the specified values.
    pub(super) fn with_all(
        query: super::CallQuery,
        headers: Option<super::CallHeaders>,
        cookies: Option<super::CallCookies>,
    ) -> Self {
        Self {
            query,
            headers,
            cookies,
        }
    }

    /// Collects all schemas from query, headers, and cookies.
    pub(super) fn collect_schemas(&self) -> super::schema::Schemas {
        let mut schemas = self.query.schemas.clone();

        if let Some(ref headers) = self.headers {
            schemas.merge(headers.schemas().clone());
        }

        if let Some(ref cookies) = self.cookies {
            schemas.merge(cookies.schemas().clone());
        }

        schemas
    }

    /// Generates OpenAPI parameters from all parameter types.
    pub(super) fn to_parameters(&self) -> Vec<utoipa::openapi::path::Parameter> {
        let mut parameters = Vec::new();

        // Add query parameters
        if !self.query.is_empty() {
            parameters.extend(self.query.to_parameters());
        }

        // Add header parameters
        if let Some(ref headers) = self.headers {
            parameters.extend(headers.to_parameters());
        }

        // Add cookie parameters
        if let Some(ref cookies) = self.cookies {
            parameters.extend(cookies.to_parameters());
        }

        parameters
    }

    /// Converts headers to HTTP headers for requests.
    pub(super) fn to_http_headers(&self) -> Result<Vec<(String, String)>, super::ApiClientError> {
        match &self.headers {
            Some(headers) => headers.to_http_headers(),
            None => Ok(Vec::new()),
        }
    }

    /// Converts cookies to HTTP Cookie header format.
    pub(super) fn to_cookie_header(&self) -> Result<String, super::ApiClientError> {
        match &self.cookies {
            Some(cookies) => cookies.to_cookie_header(),
            None => Ok(String::new()),
        }
    }
}
