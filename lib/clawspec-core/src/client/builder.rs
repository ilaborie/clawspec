use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr};

use http::Uri;
use http::uri::{PathAndQuery, Scheme};
use utoipa::openapi::{Info, Server};

use super::{ApiClient, ApiClientError};

/// Builder for creating `ApiClient` instances with comprehensive configuration options.
///
/// `ApiClientBuilder` provides a fluent interface for configuring all aspects of an API client,
/// including network settings, base paths, OpenAPI metadata, and server definitions.
///
/// # Example
///
/// ```rust
/// use clawspec_core::ApiClient;
/// use http::uri::Scheme;
/// use utoipa::openapi::{InfoBuilder, ServerBuilder};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = ApiClient::builder()
///     .with_scheme(Scheme::HTTPS)
///     .with_host("api.example.com")
///     .with_port(443)
///     .with_base_path("/v1")?
///     .with_info(
///         InfoBuilder::new()
///             .title("Example API")
///             .version("1.0.0")
///             .description(Some("API documentation generated from tests"))
///             .build()
///     )
///     .add_server(
///         ServerBuilder::new()
///             .url("https://api.example.com/v1")
///             .description(Some("Production server"))
///             .build()
///     )
///     .add_server(
///         ServerBuilder::new()
///             .url("https://staging.example.com/v1")
///             .description(Some("Staging server"))
///             .build()
///     )
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ApiClientBuilder {
    client: reqwest::Client,
    scheme: Scheme,
    host: String,
    port: u16,
    base_path: Option<PathAndQuery>,
    info: Option<Info>,
    servers: Vec<Server>,
}

impl ApiClientBuilder {
    /// Builds the final `ApiClient` instance with all configured settings.
    ///
    /// This method consumes the builder and creates an `ApiClient` ready for making API calls.
    /// All configuration options set through the builder methods are applied to the client.
    ///
    /// # Returns
    ///
    /// Returns a `Result<ApiClient, ApiClientError>` which will be:
    /// - `Ok(ApiClient)` if the client was created successfully
    /// - `Err(ApiClientError)` if there was an error building the URI or other configuration issues
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - The base URI cannot be constructed from the provided scheme, host, and port
    /// - The base path is invalid and cannot be parsed
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::builder()
    ///     .with_host("api.example.com")
    ///     .with_base_path("/v1")?
    ///     .build()?;  // This consumes the builder
    ///
    /// // Now you can use the client for API calls
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> Result<ApiClient, ApiClientError> {
        let Self {
            client,
            scheme,
            host,
            port,
            base_path,
            info,
            servers,
        } = self;

        let builder = Uri::builder()
            .scheme(scheme)
            .authority(format!("{host}:{port}"));
        let builder = if let Some(path) = &base_path {
            builder.path_and_query(path.path())
        } else {
            builder.path_and_query("/")
        };

        let base_uri = builder.build()?;
        let base_path = base_path
            .as_ref()
            .map(|it| it.path().to_string())
            .unwrap_or_default();

        let collectors = Default::default();

        Ok(ApiClient {
            client,
            base_uri,
            base_path,
            info,
            servers,
            collectors,
        })
    }

    pub fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = scheme;
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the base path for all API requests.
    ///
    /// This path will be prepended to all request paths. The path must be valid
    /// according to URI standards (no spaces, properly encoded, etc.).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // API versioning
    /// let client = ApiClient::builder()
    ///     .with_host("api.example.com")
    ///     .with_base_path("/v1")?              // All requests will start with /v1
    ///     .build()?;
    ///
    /// // More complex base paths
    /// let client = ApiClient::builder()
    ///     .with_base_path("/api/v2")?          // Multiple path segments
    ///     .build()?;
    ///
    /// // Nested API paths
    /// let client = ApiClient::builder()
    ///     .with_base_path("/services/user-api/v1")?  // Deep nesting
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ApiClientError::InvalidBasePath` if the path contains invalid characters
    /// (such as spaces) or cannot be parsed as a valid URI path.
    ///
    /// ```rust,should_panic
    /// use clawspec_core::ApiClient;
    ///
    /// // This will fail - paths cannot contain unencoded spaces
    /// let result = ApiClient::builder()
    ///     .with_base_path("/invalid path with spaces")
    ///     .expect("Should fail with invalid path");
    /// ```
    pub fn with_base_path<P>(mut self, base_path: P) -> Result<Self, ApiClientError>
    where
        P: TryInto<PathAndQuery>,
        P::Error: Debug + 'static,
    {
        let base_path = base_path
            .try_into()
            .map_err(|err| ApiClientError::InvalidBasePath {
                error: format!("{err:?}"),
            })?;
        self.base_path = Some(base_path);
        Ok(self)
    }

    /// Sets the OpenAPI info metadata for the generated specification.
    ///
    /// The info object provides metadata about the API including title, version,
    /// description, contact information, license, and other details that will
    /// appear in the generated OpenAPI specification.
    ///
    /// # Parameters
    ///
    /// * `info` - The OpenAPI Info object containing API metadata
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// use utoipa::openapi::InfoBuilder;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::builder()
    ///     .with_info(
    ///         InfoBuilder::new()
    ///             .title("My API")
    ///             .version("1.0.0")
    ///             .description(Some("A comprehensive API for managing resources"))
    ///             .build()
    ///     )
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// - If no info is set, the generated OpenAPI specification will not include an info section
    /// - The info can be updated by calling this method multiple times (last call wins)
    /// - Common practice is to set at least title and version for OpenAPI compliance
    pub fn with_info(mut self, info: Info) -> Self {
        self.info = Some(info);
        self
    }

    /// Sets the complete list of servers for the OpenAPI specification.
    ///
    /// This method replaces any previously configured servers. Use `add_server()`
    /// if you want to add servers incrementally.
    ///
    /// # Parameters
    ///
    /// * `servers` - A vector of Server objects defining the available API servers
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// use utoipa::openapi::ServerBuilder;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let servers = vec![
    ///     ServerBuilder::new()
    ///         .url("https://api.example.com/v1")
    ///         .description(Some("Production server"))
    ///         .build(),
    ///     ServerBuilder::new()
    ///         .url("https://staging.example.com/v1")
    ///         .description(Some("Staging server"))
    ///         .build(),
    /// ];
    ///
    /// let client = ApiClient::builder()
    ///     .with_servers(servers)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_servers(mut self, servers: Vec<Server>) -> Self {
        self.servers = servers;
        self
    }

    /// Adds a single server to the OpenAPI specification.
    ///
    /// This method allows you to incrementally add servers to the configuration.
    /// Each call adds to the existing list of servers.
    ///
    /// # Parameters
    ///
    /// * `server` - A Server object defining an available API server
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ApiClient;
    /// use utoipa::openapi::ServerBuilder;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::builder()
    ///     .add_server(
    ///         ServerBuilder::new()
    ///             .url("https://api.example.com/v1")
    ///             .description(Some("Production server"))
    ///             .build()
    ///     )
    ///     .add_server(
    ///         ServerBuilder::new()
    ///             .url("https://staging.example.com/v1")
    ///             .description(Some("Staging server"))
    ///             .build()
    ///     )
    ///     .add_server(
    ///         ServerBuilder::new()
    ///             .url("http://localhost:8080")
    ///             .description(Some("Development server"))
    ///             .build()
    ///     )
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Server Definition Best Practices
    ///
    /// - Include meaningful descriptions for each server
    /// - Order servers by preference (production first, then staging, then development)
    /// - Use HTTPS for production servers when available
    /// - Include the full base URL including API version paths
    pub fn add_server(mut self, server: Server) -> Self {
        self.servers.push(server);
        self
    }
}

impl Default for ApiClientBuilder {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            scheme: Scheme::HTTP,
            host: IpAddr::V4(Ipv4Addr::LOCALHOST).to_string(),
            port: 80,
            base_path: None,
            info: None,
            servers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::uri::Scheme;
    use utoipa::openapi::{InfoBuilder, ServerBuilder};

    #[test]
    fn test_default_builder_creates_localhost_http_client() {
        let client = ApiClientBuilder::default()
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://127.0.0.1:80/");
    }

    #[test]
    fn test_builder_with_custom_scheme() {
        let client = ApiClientBuilder::default()
            .with_scheme(Scheme::HTTPS)
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"https://127.0.0.1:80/");
    }

    #[test]
    fn test_builder_with_custom_host() {
        let client = ApiClientBuilder::default()
            .with_host("api.example.com")
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://api.example.com:80/");
    }

    #[test]
    fn test_builder_with_custom_port() {
        let client = ApiClientBuilder::default()
            .with_port(8080)
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://127.0.0.1:8080/");
    }

    #[test]
    fn test_builder_with_valid_base_path() {
        let client = ApiClientBuilder::default()
            .with_base_path("/api/v1")
            .expect("valid base path")
            .build()
            .expect("should build client");

        insta::assert_debug_snapshot!(client.base_path, @r#""/api/v1""#);
    }

    #[test]
    fn test_builder_with_invalid_base_path_warns_and_continues() {
        let result = ApiClientBuilder::default().with_base_path("invalid path with spaces");
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_info() {
        let info = InfoBuilder::new()
            .title("Test API")
            .version("1.0.0")
            .description(Some("Test API description"))
            .build();

        let client = ApiClientBuilder::default()
            .with_info(info.clone())
            .build()
            .expect("should build client");

        assert_eq!(client.info, Some(info));
    }

    #[test]
    fn test_builder_with_servers() {
        let servers = vec![
            ServerBuilder::new()
                .url("https://api.example.com")
                .description(Some("Production server"))
                .build(),
            ServerBuilder::new()
                .url("https://staging.example.com")
                .description(Some("Staging server"))
                .build(),
        ];

        let client = ApiClientBuilder::default()
            .with_servers(servers.clone())
            .build()
            .expect("should build client");

        assert_eq!(client.servers, servers);
    }

    #[test]
    fn test_builder_add_server() {
        let server1 = ServerBuilder::new()
            .url("https://api.example.com")
            .description(Some("Production server"))
            .build();

        let server2 = ServerBuilder::new()
            .url("https://staging.example.com")
            .description(Some("Staging server"))
            .build();

        let client = ApiClientBuilder::default()
            .add_server(server1.clone())
            .add_server(server2.clone())
            .build()
            .expect("should build client");

        assert_eq!(client.servers, vec![server1, server2]);
    }

    #[test]
    fn test_builder_with_complete_openapi_config() {
        let info = InfoBuilder::new()
            .title("Complete API")
            .version("2.0.0")
            .description(Some("A fully configured API"))
            .build();

        let server = ServerBuilder::new()
            .url("https://api.example.com/v2")
            .description(Some("Production server"))
            .build();

        let client = ApiClientBuilder::default()
            .with_scheme(Scheme::HTTPS)
            .with_host("api.example.com")
            .with_port(443)
            .with_base_path("/v2")
            .expect("valid base path")
            .with_info(info.clone())
            .add_server(server.clone())
            .build()
            .expect("should build client");

        assert_eq!(client.info, Some(info));
        assert_eq!(client.servers, vec![server]);
        insta::assert_debug_snapshot!(client.base_path, @r#""/v2""#);
        assert_eq!(
            client.base_uri.to_string(),
            "https://api.example.com:443/v2"
        );
    }
}
