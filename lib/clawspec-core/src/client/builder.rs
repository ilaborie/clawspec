use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr};

use http::Uri;
use http::uri::{PathAndQuery, Scheme};
use indexmap::IndexMap;
use utoipa::openapi::{Info, Server};

use super::openapi::channel::CollectorHandle;
use super::security::{SecurityRequirement, SecurityScheme};
use super::{ApiClient, ApiClientError};

/// Builder for creating `ApiClient` instances with comprehensive configuration options.
///
/// `ApiClientBuilder` provides a fluent interface for configuring all aspects of an API client,
/// including network settings, base paths, OpenAPI metadata, and server definitions.
///
/// # Default Configuration
///
/// - **Scheme**: HTTP (use `with_scheme()` to change to HTTPS)
/// - **Host**: 127.0.0.1 (localhost)
/// - **Port**: 80 (standard HTTP port)
/// - **Base path**: None (requests go to root path)
/// - **OpenAPI info**: None (no metadata)
/// - **Servers**: Empty list
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
    authentication: Option<super::Authentication>,
    security_schemes: IndexMap<String, SecurityScheme>,
    default_security: Vec<SecurityRequirement>,
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
            authentication,
            security_schemes,
            default_security,
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

        let collector_handle = CollectorHandle::spawn();

        Ok(ApiClient {
            client,
            base_uri,
            base_path,
            info,
            servers,
            collector_handle,
            authentication,
            security_schemes,
            default_security,
        })
    }

    /// Sets the HTTP scheme. Defaults to `HTTP`. Use [`with_https()`](Self::with_https) for convenience.
    pub fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = scheme;
        self
    }

    /// Sets the hostname or IP address. Defaults to `127.0.0.1`.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Sets the port number. Defaults to `80`.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the base path prepended to all requests (e.g., `/v1` or `/api/v2`).
    ///
    /// # Errors
    ///
    /// Returns [`ApiClientError::InvalidBasePath`] if the path is invalid.
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

    /// Sets the OpenAPI info metadata (title, version, description, etc.).
    ///
    /// Use [`with_info_simple()`](Self::with_info_simple) for basic cases.
    pub fn with_info(mut self, info: Info) -> Self {
        self.info = Some(info);
        self
    }

    /// Sets the OpenAPI servers list. Use [`add_server()`](Self::add_server) to add incrementally.
    pub fn with_servers(mut self, servers: Vec<Server>) -> Self {
        self.servers = servers;
        self
    }

    /// Adds a server to the OpenAPI specification. Use [`add_server_simple()`](Self::add_server_simple) for convenience.
    pub fn add_server(mut self, server: Server) -> Self {
        self.servers.push(server);
        self
    }

    /// Sets the default authentication for all requests. Can be overridden per-request.
    ///
    /// Supports `Bearer`, `Basic`, and `ApiKey` authentication types.
    pub fn with_authentication(mut self, authentication: super::Authentication) -> Self {
        self.authentication = Some(authentication);
        self
    }

    // =========================================================================
    // Simplified builder methods (no external types required)
    // =========================================================================

    /// Convenience method to set API title and version without importing utoipa types.
    pub fn with_info_simple(
        mut self,
        title: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        use utoipa::openapi::InfoBuilder;
        self.info = Some(InfoBuilder::new().title(title).version(version).build());
        self
    }

    /// Sets or updates the API description. Creates default info if not set.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        use utoipa::openapi::InfoBuilder;
        let description = description.into();
        self.info = Some(match self.info {
            Some(info) => InfoBuilder::from(info)
                .description(Some(description))
                .build(),
            None => InfoBuilder::new()
                .title("API")
                .version("0.0.0")
                .description(Some(description))
                .build(),
        });
        self
    }

    /// Sets the HTTP scheme to HTTPS. Convenience method for `with_scheme(Scheme::HTTPS)`.
    pub fn with_https(mut self) -> Self {
        self.scheme = Scheme::HTTPS;
        self
    }

    /// Adds a server with URL and description. No utoipa imports needed.
    pub fn add_server_simple(
        mut self,
        url: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        use utoipa::openapi::ServerBuilder;
        let server = ServerBuilder::new()
            .url(url)
            .description(Some(description))
            .build();
        self.servers.push(server);
        self
    }

    /// Registers a named security scheme for OpenAPI `components.securitySchemes`.
    pub fn with_security_scheme(mut self, name: impl Into<String>, scheme: SecurityScheme) -> Self {
        self.security_schemes.insert(name.into(), scheme);
        self
    }

    /// Sets the default security requirement for all operations.
    pub fn with_default_security(mut self, requirement: SecurityRequirement) -> Self {
        self.default_security.push(requirement);
        self
    }

    /// Adds multiple default security requirements (OR relationship).
    pub fn with_default_securities(
        mut self,
        requirements: impl IntoIterator<Item = SecurityRequirement>,
    ) -> Self {
        self.default_security.extend(requirements);
        self
    }

    // =========================================================================
    // OAuth2 convenience methods (requires "oauth2" feature)
    // =========================================================================

    /// Configures OAuth2 Client Credentials authentication. Tokens are auto-refreshed.
    ///
    /// # Errors
    ///
    /// Returns an error if the token URL is invalid.
    ///
    /// ```rust,ignore
    /// let client = ApiClient::builder()
    ///     .with_oauth2_client_credentials("client-id", "secret", "https://auth.example.com/token")?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "oauth2")]
    pub fn with_oauth2_client_credentials(
        self,
        client_id: impl Into<String>,
        client_secret: impl Into<super::SecureString>,
        token_url: impl AsRef<str>,
    ) -> Result<Self, ApiClientError> {
        use super::Authentication;
        use super::oauth2::{OAuth2Config, SharedOAuth2Config};

        let config = OAuth2Config::client_credentials(client_id, client_secret, token_url)
            .map_err(ApiClientError::oauth2_error)?
            .build()
            .map_err(ApiClientError::oauth2_error)?;

        Ok(self.with_authentication(Authentication::OAuth2(SharedOAuth2Config::new(config))))
    }

    /// Configures OAuth2 authentication with Client Credentials flow and scopes.
    ///
    /// This is a convenience method for setting up OAuth2 authentication with specific scopes.
    ///
    /// # Parameters
    ///
    /// * `client_id` - The OAuth2 client ID
    /// * `client_secret` - The OAuth2 client secret
    /// * `token_url` - The token endpoint URL
    /// * `scopes` - The OAuth2 scopes to request
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use clawspec_core::ApiClient;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::builder()
    ///     .with_oauth2_client_credentials_scopes(
    ///         "my-client-id",
    ///         "my-client-secret",
    ///         "https://auth.example.com/oauth/token",
    ///         ["read:users", "write:users"],
    ///     )?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "oauth2")]
    pub fn with_oauth2_client_credentials_scopes(
        self,
        client_id: impl Into<String>,
        client_secret: impl Into<super::SecureString>,
        token_url: impl AsRef<str>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ApiClientError> {
        use super::Authentication;
        use super::oauth2::{OAuth2Config, SharedOAuth2Config};

        let config = OAuth2Config::client_credentials(client_id, client_secret, token_url)
            .map_err(ApiClientError::oauth2_error)?
            .add_scopes(scopes)
            .build()
            .map_err(ApiClientError::oauth2_error)?;

        Ok(self.with_authentication(Authentication::OAuth2(SharedOAuth2Config::new(config))))
    }

    /// Configures OAuth2 authentication with a pre-acquired token.
    ///
    /// Use this method when you already have an access token from another source
    /// (e.g., environment variable, test setup).
    ///
    /// # Parameters
    ///
    /// * `access_token` - The pre-acquired access token
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use clawspec_core::ApiClient;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let token = std::env::var("API_TOKEN").unwrap_or_else(|_| "test-token".to_string());
    ///
    /// let client = ApiClient::builder()
    ///     .with_oauth2_token(token)?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "oauth2")]
    pub fn with_oauth2_token(
        self,
        access_token: impl Into<String>,
    ) -> Result<Self, ApiClientError> {
        use super::Authentication;
        use super::oauth2::{OAuth2Config, SharedOAuth2Config};

        // Use a dummy token URL for pre-acquired tokens
        let config = OAuth2Config::pre_acquired(
            "pre-acquired",
            "https://placeholder.example.com/token",
            access_token,
        )
        .map_err(ApiClientError::oauth2_error)?
        .build()
        .map_err(ApiClientError::oauth2_error)?;

        Ok(self.with_authentication(Authentication::OAuth2(SharedOAuth2Config::new(config))))
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
            authentication: None,
            security_schemes: IndexMap::new(),
            default_security: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::uri::Scheme;
    use utoipa::openapi::{InfoBuilder, ServerBuilder};

    #[tokio::test]
    async fn test_default_builder_creates_localhost_http_client() {
        let client = ApiClientBuilder::default()
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://127.0.0.1:80/");
    }

    #[tokio::test]
    async fn test_builder_with_custom_scheme() {
        let client = ApiClientBuilder::default()
            .with_scheme(Scheme::HTTPS)
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"https://127.0.0.1:80/");
    }

    #[tokio::test]
    async fn test_builder_with_custom_host() {
        let client = ApiClientBuilder::default()
            .with_host("api.example.com")
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://api.example.com:80/");
    }

    #[tokio::test]
    async fn test_builder_with_custom_port() {
        let client = ApiClientBuilder::default()
            .with_port(8080)
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://127.0.0.1:8080/");
    }

    #[tokio::test]
    async fn test_builder_with_valid_base_path() {
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

    #[tokio::test]
    async fn test_builder_with_info() {
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

    #[tokio::test]
    async fn test_builder_with_servers() {
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

    #[tokio::test]
    async fn test_builder_add_server() {
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

    #[tokio::test]
    async fn test_builder_with_complete_openapi_config() {
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

    #[tokio::test]
    async fn test_builder_with_authentication_bearer() {
        let client = ApiClientBuilder::default()
            .with_authentication(super::super::Authentication::Bearer("test-token".into()))
            .build()
            .expect("should build client");

        assert!(matches!(
            client.authentication,
            Some(super::super::Authentication::Bearer(ref token)) if token.equals_str("test-token")
        ));
    }

    #[tokio::test]
    async fn test_builder_with_authentication_basic() {
        let client = ApiClientBuilder::default()
            .with_authentication(super::super::Authentication::Basic {
                username: "user".to_string(),
                password: "pass".into(),
            })
            .build()
            .expect("should build client");

        assert!(matches!(
            client.authentication,
            Some(super::super::Authentication::Basic { ref username, ref password })
                if username == "user" && password.equals_str("pass")
        ));
    }

    #[tokio::test]
    async fn test_builder_with_authentication_api_key() {
        let client = ApiClientBuilder::default()
            .with_authentication(super::super::Authentication::ApiKey {
                header_name: "X-API-Key".to_string(),
                key: "secret-key".into(),
            })
            .build()
            .expect("should build client");

        assert!(matches!(
            client.authentication,
            Some(super::super::Authentication::ApiKey { ref header_name, ref key })
                if header_name == "X-API-Key" && key.equals_str("secret-key")
        ));
    }

    #[tokio::test]
    async fn test_builder_without_authentication() {
        let client = ApiClientBuilder::default()
            .build()
            .expect("should build client");

        assert!(client.authentication.is_none());
    }

    #[tokio::test]
    async fn test_builder_with_security_scheme() {
        use super::super::security::{ApiKeyLocation, SecurityScheme};

        let client = ApiClientBuilder::default()
            .with_security_scheme("bearerAuth", SecurityScheme::bearer())
            .with_security_scheme(
                "apiKey",
                SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header),
            )
            .build()
            .expect("should build client");

        assert_eq!(client.security_schemes.len(), 2);
        assert!(client.security_schemes.contains_key("bearerAuth"));
        assert!(client.security_schemes.contains_key("apiKey"));
    }

    #[tokio::test]
    async fn test_builder_with_default_security() {
        use super::super::security::{SecurityRequirement, SecurityScheme};

        let client = ApiClientBuilder::default()
            .with_security_scheme("bearerAuth", SecurityScheme::bearer())
            .with_default_security(SecurityRequirement::new("bearerAuth"))
            .build()
            .expect("should build client");

        assert_eq!(client.default_security.len(), 1);
        assert_eq!(client.default_security[0].name, "bearerAuth");
    }

    #[tokio::test]
    async fn test_builder_with_multiple_default_securities() {
        use super::super::security::{ApiKeyLocation, SecurityRequirement, SecurityScheme};

        let client = ApiClientBuilder::default()
            .with_security_scheme("bearerAuth", SecurityScheme::bearer())
            .with_security_scheme(
                "apiKey",
                SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header),
            )
            .with_default_securities([
                SecurityRequirement::new("bearerAuth"),
                SecurityRequirement::new("apiKey"),
            ])
            .build()
            .expect("should build client");

        assert_eq!(client.default_security.len(), 2);
    }

    #[tokio::test]
    async fn test_builder_security_scheme_with_description() {
        use super::super::security::SecurityScheme;

        let client = ApiClientBuilder::default()
            .with_security_scheme(
                "bearerAuth",
                SecurityScheme::bearer_with_format("JWT")
                    .with_description("JWT token from /auth/login"),
            )
            .build()
            .expect("should build client");

        let scheme = client.security_schemes.get("bearerAuth").unwrap();
        assert!(matches!(
            scheme,
            SecurityScheme::Bearer {
                format: Some(f),
                description: Some(d)
            } if f == "JWT" && d == "JWT token from /auth/login"
        ));
    }

    #[tokio::test]
    async fn test_security_schemes_appear_in_openapi() {
        use super::super::security::{SecurityRequirement, SecurityScheme};

        let mut client = ApiClientBuilder::default()
            .with_security_scheme("bearerAuth", SecurityScheme::bearer_with_format("JWT"))
            .with_default_security(SecurityRequirement::new("bearerAuth"))
            .build()
            .expect("should build client");

        let openapi = client.collected_openapi().await;

        // Check that security schemes are in components
        let components = openapi.components.expect("should have components");
        let security_schemes = components.security_schemes;
        assert!(security_schemes.contains_key("bearerAuth"));

        // Check that default security is present
        let security = openapi.security.expect("should have security");
        assert!(!security.is_empty());
    }
}
