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
    #[cfg(feature = "redaction")]
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
    pub(super) fn collect_schemas(&self) -> super::openapi::schema::Schemas {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{CallCookies, CallHeaders, CallQuery};

    #[test]
    fn test_call_parameters_default() {
        let params = CallParameters::default();

        assert!(params.query.is_empty());
        assert!(params.headers.is_none());
        assert!(params.cookies.is_none());
    }

    #[test]
    fn test_call_parameters_with_all() {
        let query = CallQuery::new().add_param("key", "value");
        let headers = CallHeaders::new().add_header("X-Test", "header-value");
        let cookies = CallCookies::new().add_cookie("session", "abc123");

        let params = CallParameters::with_all(query, Some(headers), Some(cookies));

        assert!(!params.query.is_empty());
        assert!(params.headers.is_some());
        assert!(params.cookies.is_some());
    }

    #[test]
    fn test_call_parameters_collect_schemas_empty() {
        let params = CallParameters::default();
        let schemas = params.collect_schemas();

        // Just verify collect_schemas runs without error for empty params
        let _ = schemas;
    }

    #[test]
    fn test_call_parameters_collect_schemas_with_query() {
        let query = CallQuery::new().add_param("limit", 10i32);
        let params = CallParameters::with_all(query, None, None);
        let schemas = params.collect_schemas();

        // Query with primitive type might still be empty depending on implementation
        // The important thing is that collect_schemas runs without error
        let _ = schemas;
    }

    #[test]
    fn test_call_parameters_collect_schemas_with_headers() {
        let query = CallQuery::new();
        let headers = CallHeaders::new().add_header("X-Custom", "value");
        let params = CallParameters::with_all(query, Some(headers), None);
        let schemas = params.collect_schemas();

        let _ = schemas;
    }

    #[test]
    fn test_call_parameters_collect_schemas_with_cookies() {
        let query = CallQuery::new();
        let cookies = CallCookies::new().add_cookie("session", "value");
        let params = CallParameters::with_all(query, None, Some(cookies));
        let schemas = params.collect_schemas();

        let _ = schemas;
    }

    #[test]
    fn test_call_parameters_collect_schemas_all_types() {
        let query = CallQuery::new().add_param("q", "search");
        let headers = CallHeaders::new().add_header("Authorization", "Bearer token");
        let cookies = CallCookies::new().add_cookie("session", "abc");
        let params = CallParameters::with_all(query, Some(headers), Some(cookies));

        let schemas = params.collect_schemas();
        let _ = schemas;
    }

    #[test]
    fn test_call_parameters_to_parameters_empty() {
        let params = CallParameters::default();
        let openapi_params = params.to_parameters();

        assert!(openapi_params.is_empty());
    }

    #[test]
    fn test_call_parameters_to_parameters_with_query() {
        let query = CallQuery::new().add_param("limit", 10i32);
        let params = CallParameters::with_all(query, None, None);
        let openapi_params = params.to_parameters();

        assert_eq!(openapi_params.len(), 1);
    }

    #[test]
    fn test_call_parameters_to_parameters_with_headers() {
        let query = CallQuery::new();
        let headers = CallHeaders::new().add_header("X-Request-ID", "12345");
        let params = CallParameters::with_all(query, Some(headers), None);
        let openapi_params = params.to_parameters();

        assert_eq!(openapi_params.len(), 1);
    }

    #[test]
    fn test_call_parameters_to_parameters_with_cookies() {
        let query = CallQuery::new();
        let cookies = CallCookies::new().add_cookie("token", "secret");
        let params = CallParameters::with_all(query, None, Some(cookies));
        let openapi_params = params.to_parameters();

        assert_eq!(openapi_params.len(), 1);
    }

    #[test]
    fn test_call_parameters_to_parameters_all_types() {
        let query = CallQuery::new()
            .add_param("page", 1i32)
            .add_param("size", 20i32);
        let headers = CallHeaders::new()
            .add_header("Authorization", "Bearer xxx")
            .add_header("X-Custom", "value");
        let cookies = CallCookies::new()
            .add_cookie("session", "abc")
            .add_cookie("csrf", "xyz");
        let params = CallParameters::with_all(query, Some(headers), Some(cookies));

        let openapi_params = params.to_parameters();

        // 2 query + 2 headers + 2 cookies = 6 parameters
        assert_eq!(openapi_params.len(), 6);
    }

    #[test]
    fn test_call_parameters_to_http_headers_none() {
        let params = CallParameters::default();
        let result = params.to_http_headers();

        assert!(result.is_ok());
        assert!(result.expect("should be Ok").is_empty());
    }

    #[test]
    fn test_call_parameters_to_http_headers_with_headers() {
        let query = CallQuery::new();
        let headers = CallHeaders::new()
            .add_header("X-Test", "value1")
            .add_header("Accept", "application/json");
        let params = CallParameters::with_all(query, Some(headers), None);

        let result = params.to_http_headers();

        assert!(result.is_ok());
        let http_headers = result.expect("should be Ok");
        assert_eq!(http_headers.len(), 2);
    }

    #[test]
    fn test_call_parameters_to_cookie_header_none() {
        let params = CallParameters::default();
        let result = params.to_cookie_header();

        assert!(result.is_ok());
        assert!(result.expect("should be Ok").is_empty());
    }

    #[test]
    fn test_call_parameters_to_cookie_header_with_cookies() {
        let query = CallQuery::new();
        let cookies = CallCookies::new().add_cookie("session", "abc123");
        let params = CallParameters::with_all(query, None, Some(cookies));

        let result = params.to_cookie_header();

        assert!(result.is_ok());
        let cookie_header = result.expect("should be Ok");
        assert!(cookie_header.contains("session=abc123"));
    }

    #[test]
    fn test_operation_metadata_default() {
        let metadata = OperationMetadata::default();

        assert!(metadata.operation_id.is_empty());
        assert!(metadata.tags.is_none());
        assert!(metadata.description.is_none());
        #[cfg(feature = "redaction")]
        assert!(metadata.response_description.is_none());
    }
}
