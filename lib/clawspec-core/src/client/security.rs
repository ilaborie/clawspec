//! OpenAPI Security Scheme support for clawspec.
//!
//! This module provides types for defining and configuring OpenAPI security schemes
//! that are included in the generated specification. Security schemes describe
//! the authentication methods available for your API.
//!
//! # Overview
//!
//! Security in OpenAPI consists of two parts:
//! 1. **Security Schemes**: Definitions of authentication methods (Bearer, Basic, API Key, etc.)
//! 2. **Security Requirements**: References to schemes that must be satisfied for an operation
//!
//! # Example
//!
//! ```rust
//! use clawspec_core::{ApiClient, SecurityScheme, SecurityRequirement, ApiKeyLocation};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = ApiClient::builder()
//!     .with_security_scheme("bearerAuth", SecurityScheme::bearer())
//!     .with_security_scheme("apiKey", SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header))
//!     .with_default_security(SecurityRequirement::new("bearerAuth"))
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Generated OpenAPI
//!
//! The security schemes are output in the `components.securitySchemes` section:
//!
//! ```yaml
//! components:
//!   securitySchemes:
//!     bearerAuth:
//!       type: http
//!       scheme: bearer
//!     apiKey:
//!       type: apiKey
//!       name: X-API-Key
//!       in: header
//! security:
//!   - bearerAuth: []
//! ```

use indexmap::IndexMap;
use utoipa::openapi::Deprecated;
use utoipa::openapi::security::{
    ApiKey as UtoipaApiKey, ApiKeyValue, AuthorizationCode, ClientCredentials, DeviceAuthorization,
    Flow, Http, HttpAuthScheme, Implicit, OAuth2 as UtoipaOAuth2,
    OpenIdConnect as UtoipaOpenIdConnect, Password, Scopes, SecurityScheme as UtoipaSecurityScheme,
};

/// OpenAPI security scheme configuration.
///
/// This enum represents the different types of security schemes supported by OpenAPI.
/// Each variant maps directly to an OpenAPI security scheme type.
///
/// # Supported Schemes
///
/// - **Bearer**: HTTP Bearer token authentication (RFC 6750)
/// - **Basic**: HTTP Basic authentication (RFC 7617)
/// - **ApiKey**: API key passed in header, query, or cookie
/// - **OAuth2**: OAuth 2.0 authentication flows
/// - **OpenIdConnect**: OpenID Connect Discovery
///
/// # Example
///
/// ```rust
/// use clawspec_core::{SecurityScheme, ApiKeyLocation};
///
/// // Simple bearer token
/// let bearer = SecurityScheme::bearer();
///
/// // Bearer with JWT format hint
/// let jwt = SecurityScheme::bearer_with_format("JWT");
///
/// // API key in header
/// let api_key = SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header);
///
/// // Basic auth
/// let basic = SecurityScheme::basic();
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityScheme {
    /// HTTP Bearer authentication (RFC 6750).
    ///
    /// Used for token-based authentication where the client sends
    /// an `Authorization: Bearer <token>` header.
    #[non_exhaustive]
    Bearer {
        /// Optional format hint (e.g., "JWT" for JSON Web Tokens)
        format: Option<String>,
        /// Description for documentation
        description: Option<String>,
        /// Whether the scheme is deprecated
        deprecated: bool,
    },

    /// HTTP Basic authentication (RFC 7617).
    ///
    /// Uses `Authorization: Basic <base64(username:password)>` header.
    #[non_exhaustive]
    Basic {
        /// Description for documentation
        description: Option<String>,
        /// Whether the scheme is deprecated
        deprecated: bool,
    },

    /// API Key authentication.
    ///
    /// The API key can be passed in a header, query parameter, or cookie.
    #[non_exhaustive]
    ApiKey {
        /// Name of the header, query parameter, or cookie
        name: String,
        /// Where the API key is passed
        location: ApiKeyLocation,
        /// Description for documentation
        description: Option<String>,
        /// Whether the scheme is deprecated
        deprecated: bool,
    },

    /// OAuth 2.0 authentication.
    ///
    /// Supports multiple OAuth2 flows: authorization code, client credentials,
    /// implicit, password, and device authorization.
    #[non_exhaustive]
    OAuth2 {
        /// OAuth2 flows configuration (boxed to reduce enum size)
        flows: Box<OAuth2Flows>,
        /// URL of the OAuth2 authorization server metadata (RFC 8414)
        metadata_url: Option<String>,
        /// Description for documentation
        description: Option<String>,
        /// Whether the scheme is deprecated
        deprecated: bool,
    },

    /// OpenID Connect Discovery.
    ///
    /// Uses OpenID Connect for authentication with automatic discovery
    /// of the provider's configuration.
    #[non_exhaustive]
    OpenIdConnect {
        /// OpenID Connect discovery URL
        open_id_connect_url: String,
        /// Description for documentation
        description: Option<String>,
        /// Whether the scheme is deprecated
        deprecated: bool,
    },
}

impl SecurityScheme {
    /// Creates a simple HTTP Bearer authentication scheme.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityScheme;
    ///
    /// let scheme = SecurityScheme::bearer();
    /// ```
    pub fn bearer() -> Self {
        Self::Bearer {
            format: None,
            description: None,
            deprecated: false,
        }
    }

    /// Creates an HTTP Bearer authentication scheme with a format hint.
    ///
    /// # Arguments
    ///
    /// * `format` - Format hint (e.g., "JWT" for JSON Web Tokens)
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityScheme;
    ///
    /// let scheme = SecurityScheme::bearer_with_format("JWT");
    /// ```
    pub fn bearer_with_format(format: impl Into<String>) -> Self {
        Self::Bearer {
            format: Some(format.into()),
            description: None,
            deprecated: false,
        }
    }

    /// Creates an HTTP Basic authentication scheme.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityScheme;
    ///
    /// let scheme = SecurityScheme::basic();
    /// ```
    pub fn basic() -> Self {
        Self::Basic {
            description: None,
            deprecated: false,
        }
    }

    /// Creates an API Key authentication scheme.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the header, query parameter, or cookie
    /// * `location` - Where the API key is passed
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{SecurityScheme, ApiKeyLocation};
    ///
    /// let scheme = SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header);
    /// ```
    pub fn api_key(name: impl Into<String>, location: ApiKeyLocation) -> Self {
        Self::ApiKey {
            name: name.into(),
            location,
            description: None,
            deprecated: false,
        }
    }

    /// Creates an OAuth2 authentication scheme.
    ///
    /// # Arguments
    ///
    /// * `flows` - The supported OAuth2 flows
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{OAuth2Flows, SecurityScheme};
    ///
    /// let scheme = SecurityScheme::oauth2(OAuth2Flows::client_credentials(
    ///     "https://auth.example.com/token",
    ///     [("read:users", "Read user data")],
    /// ));
    /// ```
    pub fn oauth2(flows: OAuth2Flows) -> Self {
        Self::OAuth2 {
            flows: Box::new(flows),
            metadata_url: None,
            description: None,
            deprecated: false,
        }
    }

    /// Creates an OAuth2 authentication scheme with the URL of the OAuth2
    /// authorization server metadata (RFC 8414).
    ///
    /// # Arguments
    ///
    /// * `flows` - The supported OAuth2 flows
    /// * `metadata_url` - URL of the authorization server metadata
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{OAuth2Flows, SecurityScheme};
    ///
    /// let scheme = SecurityScheme::oauth2_with_metadata_url(
    ///     OAuth2Flows::client_credentials(
    ///         "https://auth.example.com/token",
    ///         [("read:users", "Read user data")],
    ///     ),
    ///     "https://auth.example.com/.well-known/oauth-authorization-server",
    /// );
    /// ```
    pub fn oauth2_with_metadata_url(flows: OAuth2Flows, metadata_url: impl Into<String>) -> Self {
        Self::OAuth2 {
            flows: Box::new(flows),
            metadata_url: Some(metadata_url.into()),
            description: None,
            deprecated: false,
        }
    }

    /// Creates an OpenID Connect authentication scheme.
    ///
    /// # Arguments
    ///
    /// * `url` - OpenID Connect discovery URL
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityScheme;
    ///
    /// let scheme = SecurityScheme::openid_connect("https://auth.example.com/.well-known/openid-configuration");
    /// ```
    pub fn openid_connect(url: impl Into<String>) -> Self {
        Self::OpenIdConnect {
            open_id_connect_url: url.into(),
            description: None,
            deprecated: false,
        }
    }

    /// Adds a description to the security scheme.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityScheme;
    ///
    /// let scheme = SecurityScheme::bearer()
    ///     .with_description("JWT token obtained from /auth/login");
    /// ```
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        match &mut self {
            Self::Bearer {
                description: desc, ..
            }
            | Self::Basic {
                description: desc, ..
            }
            | Self::ApiKey {
                description: desc, ..
            }
            | Self::OAuth2 {
                description: desc, ..
            }
            | Self::OpenIdConnect {
                description: desc, ..
            } => *desc = Some(description.into()),
        }
        self
    }

    /// Marks the security scheme as deprecated, or not.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{ApiKeyLocation, SecurityScheme};
    ///
    /// let scheme = SecurityScheme::api_key("X-Legacy-Key", ApiKeyLocation::Header)
    ///     .with_deprecated(true);
    /// ```
    pub fn with_deprecated(mut self, deprecated: bool) -> Self {
        match &mut self {
            Self::Bearer {
                deprecated: flag, ..
            }
            | Self::Basic {
                deprecated: flag, ..
            }
            | Self::ApiKey {
                deprecated: flag, ..
            }
            | Self::OAuth2 {
                deprecated: flag, ..
            }
            | Self::OpenIdConnect {
                deprecated: flag, ..
            } => *flag = deprecated,
        }
        self
    }

    /// Converts this security scheme to a utoipa SecurityScheme.
    pub(crate) fn to_utoipa(&self) -> UtoipaSecurityScheme {
        match self {
            Self::Bearer {
                format,
                description,
                deprecated,
                ..
            } => {
                let mut http = Http::new(HttpAuthScheme::Bearer);
                http.bearer_format = format.clone();
                http.description = description.clone();
                http.deprecated = to_deprecated(*deprecated);
                UtoipaSecurityScheme::Http(http)
            }
            Self::Basic {
                description,
                deprecated,
                ..
            } => {
                let mut http = Http::new(HttpAuthScheme::Basic);
                http.description = description.clone();
                http.deprecated = to_deprecated(*deprecated);
                UtoipaSecurityScheme::Http(http)
            }
            Self::ApiKey {
                name,
                location,
                description,
                deprecated,
                ..
            } => {
                let mut api_key_value = ApiKeyValue::new(name);
                api_key_value.description = description.clone();
                api_key_value.deprecated = to_deprecated(*deprecated);
                let api_key = match location {
                    ApiKeyLocation::Header => UtoipaApiKey::Header(api_key_value),
                    ApiKeyLocation::Query => UtoipaApiKey::Query(api_key_value),
                    ApiKeyLocation::Cookie => UtoipaApiKey::Cookie(api_key_value),
                };
                UtoipaSecurityScheme::ApiKey(api_key)
            }
            Self::OAuth2 {
                flows,
                metadata_url,
                description,
                deprecated,
                ..
            } => {
                let mut oauth2 = flows.to_utoipa();
                oauth2.oauth2_metadata_url = metadata_url.clone();
                oauth2.description = description.clone();
                oauth2.deprecated = to_deprecated(*deprecated);
                UtoipaSecurityScheme::OAuth2(oauth2)
            }
            Self::OpenIdConnect {
                open_id_connect_url,
                description,
                deprecated,
                ..
            } => {
                let mut oidc = UtoipaOpenIdConnect::new(open_id_connect_url);
                oidc.description = description.clone();
                oidc.deprecated = to_deprecated(*deprecated);
                UtoipaSecurityScheme::OpenIdConnect(oidc)
            }
        }
    }
}

fn to_deprecated(deprecated: bool) -> Option<Deprecated> {
    deprecated.then_some(Deprecated::True)
}

fn collect_scopes(
    scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
) -> IndexMap<String, String> {
    scopes
        .into_iter()
        .map(|(name, description)| (name.into(), description.into()))
        .collect()
}

/// Location where an API key is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiKeyLocation {
    /// API key in HTTP header
    Header,
    /// API key in query parameter
    Query,
    /// API key in cookie
    Cookie,
}

/// OAuth2 flow configurations.
///
/// Represents the different OAuth2 flows supported by OpenAPI.
///
/// # Example
///
/// ```rust
/// use clawspec_core::{OAuth2DeviceAuthorizationFlow, OAuth2Flows};
///
/// let flows = OAuth2Flows::client_credentials(
///     "https://auth.example.com/token",
///     [("api:access", "API access")],
/// )
/// .with_device_authorization(OAuth2DeviceAuthorizationFlow::new(
///     "https://auth.example.com/device",
///     "https://auth.example.com/token",
///     [("api:access", "API access")],
/// ));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct OAuth2Flows {
    /// Authorization Code flow
    pub authorization_code: Option<OAuth2Flow>,
    /// Client Credentials flow
    pub client_credentials: Option<OAuth2Flow>,
    /// Implicit flow (deprecated in OAuth 2.1)
    pub implicit: Option<OAuth2ImplicitFlow>,
    /// Password flow (deprecated in OAuth 2.1)
    pub password: Option<OAuth2Flow>,
    /// Device Authorization flow (RFC 8628)
    ///
    /// This flow is only written in the specification: the `oauth2` feature
    /// cannot acquire tokens with it.
    pub device_authorization: Option<OAuth2DeviceAuthorizationFlow>,
}

impl OAuth2Flows {
    /// Creates a new OAuth2Flows with authorization code flow.
    pub fn authorization_code(
        authorization_url: impl Into<String>,
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self::default().with_authorization_code(
            OAuth2Flow::new(token_url, scopes).with_authorization_url(authorization_url),
        )
    }

    /// Creates a new OAuth2Flows with client credentials flow.
    pub fn client_credentials(
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self::default().with_client_credentials(OAuth2Flow::new(token_url, scopes))
    }

    /// Creates a new OAuth2Flows with device authorization flow.
    ///
    /// This flow is only written in the specification: the `oauth2` feature
    /// cannot acquire tokens with it.
    pub fn device_authorization(
        device_authorization_url: impl Into<String>,
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self::default().with_device_authorization(OAuth2DeviceAuthorizationFlow::new(
            device_authorization_url,
            token_url,
            scopes,
        ))
    }

    /// Sets the authorization code flow.
    pub fn with_authorization_code(mut self, flow: OAuth2Flow) -> Self {
        self.authorization_code = Some(flow);
        self
    }

    /// Sets the client credentials flow.
    pub fn with_client_credentials(mut self, flow: OAuth2Flow) -> Self {
        self.client_credentials = Some(flow);
        self
    }

    /// Sets the implicit flow.
    pub fn with_implicit(mut self, flow: OAuth2ImplicitFlow) -> Self {
        self.implicit = Some(flow);
        self
    }

    /// Sets the password flow.
    pub fn with_password(mut self, flow: OAuth2Flow) -> Self {
        self.password = Some(flow);
        self
    }

    /// Sets the device authorization flow.
    pub fn with_device_authorization(mut self, flow: OAuth2DeviceAuthorizationFlow) -> Self {
        self.device_authorization = Some(flow);
        self
    }

    fn to_utoipa(&self) -> UtoipaOAuth2 {
        let mut flows: Vec<Flow> = Vec::new();

        if let Some(flow) = &self.authorization_code {
            let scopes = Scopes::from_iter(flow.scopes.clone());
            let auth_code = if let Some(ref refresh) = flow.refresh_url {
                AuthorizationCode::with_refresh_url(
                    flow.authorization_url.as_deref().unwrap_or_default(),
                    &flow.token_url,
                    scopes,
                    refresh,
                )
            } else {
                AuthorizationCode::new(
                    flow.authorization_url.as_deref().unwrap_or_default(),
                    &flow.token_url,
                    scopes,
                )
            };
            flows.push(Flow::AuthorizationCode(auth_code));
        }

        if let Some(flow) = &self.client_credentials {
            let scopes = Scopes::from_iter(flow.scopes.clone());
            let client_creds = if let Some(ref refresh) = flow.refresh_url {
                ClientCredentials::with_refresh_url(&flow.token_url, scopes, refresh)
            } else {
                ClientCredentials::new(&flow.token_url, scopes)
            };
            flows.push(Flow::ClientCredentials(client_creds));
        }

        if let Some(flow) = &self.implicit {
            let scopes = Scopes::from_iter(flow.scopes.clone());
            let implicit = if let Some(ref refresh) = flow.refresh_url {
                Implicit::with_refresh_url(&flow.authorization_url, scopes, refresh)
            } else {
                Implicit::new(&flow.authorization_url, scopes)
            };
            flows.push(Flow::Implicit(implicit));
        }

        if let Some(flow) = &self.password {
            let scopes = Scopes::from_iter(flow.scopes.clone());
            let password = if let Some(ref refresh) = flow.refresh_url {
                Password::with_refresh_url(&flow.token_url, scopes, refresh)
            } else {
                Password::new(&flow.token_url, scopes)
            };
            flows.push(Flow::Password(password));
        }

        if let Some(flow) = &self.device_authorization {
            let scopes = Scopes::from_iter(flow.scopes.clone());
            let device_authorization = if let Some(ref refresh) = flow.refresh_url {
                DeviceAuthorization::with_refresh_url(
                    &flow.device_authorization_url,
                    &flow.token_url,
                    scopes,
                    refresh,
                )
            } else {
                DeviceAuthorization::new(&flow.device_authorization_url, &flow.token_url, scopes)
            };
            flows.push(Flow::DeviceAuthorization(device_authorization));
        }

        UtoipaOAuth2::new(flows)
    }
}

/// OAuth2 flow configuration (for flows with token URL).
///
/// Used for the authorization code, client credentials and password flows.
///
/// # Example
///
/// ```rust
/// use clawspec_core::OAuth2Flow;
///
/// let flow = OAuth2Flow::new("https://auth.example.com/token", [("read:users", "Read user data")])
///     .with_authorization_url("https://auth.example.com/authorize")
///     .with_refresh_url("https://auth.example.com/refresh");
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OAuth2Flow {
    /// Authorization URL (required for authorization_code, not for client_credentials)
    pub authorization_url: Option<String>,
    /// Token URL
    pub token_url: String,
    /// Refresh URL (optional)
    pub refresh_url: Option<String>,
    /// Available scopes
    pub scopes: IndexMap<String, String>,
}

impl OAuth2Flow {
    /// Creates a new flow without authorization URL.
    pub fn new(
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            authorization_url: None,
            token_url: token_url.into(),
            refresh_url: None,
            scopes: collect_scopes(scopes),
        }
    }

    /// Sets the authorization URL (required for the authorization code flow).
    pub fn with_authorization_url(mut self, authorization_url: impl Into<String>) -> Self {
        self.authorization_url = Some(authorization_url.into());
        self
    }

    /// Sets the refresh URL.
    pub fn with_refresh_url(mut self, refresh_url: impl Into<String>) -> Self {
        self.refresh_url = Some(refresh_url.into());
        self
    }
}

/// OAuth2 implicit flow configuration.
///
/// # Example
///
/// ```rust
/// use clawspec_core::OAuth2ImplicitFlow;
///
/// let flow = OAuth2ImplicitFlow::new(
///     "https://auth.example.com/authorize",
///     [("read:users", "Read user data")],
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OAuth2ImplicitFlow {
    /// Authorization URL
    pub authorization_url: String,
    /// Refresh URL (optional)
    pub refresh_url: Option<String>,
    /// Available scopes
    pub scopes: IndexMap<String, String>,
}

impl OAuth2ImplicitFlow {
    /// Creates a new implicit flow.
    pub fn new(
        authorization_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            authorization_url: authorization_url.into(),
            refresh_url: None,
            scopes: collect_scopes(scopes),
        }
    }

    /// Sets the refresh URL.
    pub fn with_refresh_url(mut self, refresh_url: impl Into<String>) -> Self {
        self.refresh_url = Some(refresh_url.into());
        self
    }
}

/// OAuth2 device authorization flow configuration (RFC 8628).
///
/// This flow is only written in the specification: the `oauth2` feature
/// cannot acquire tokens with it.
///
/// # Example
///
/// ```rust
/// use clawspec_core::OAuth2DeviceAuthorizationFlow;
///
/// let flow = OAuth2DeviceAuthorizationFlow::new(
///     "https://auth.example.com/device",
///     "https://auth.example.com/token",
///     [("read:users", "Read user data")],
/// )
/// .with_refresh_url("https://auth.example.com/refresh");
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OAuth2DeviceAuthorizationFlow {
    /// Device authorization URL
    pub device_authorization_url: String,
    /// Token URL
    pub token_url: String,
    /// Refresh URL (optional)
    pub refresh_url: Option<String>,
    /// Available scopes
    pub scopes: IndexMap<String, String>,
}

impl OAuth2DeviceAuthorizationFlow {
    /// Creates a new device authorization flow.
    pub fn new(
        device_authorization_url: impl Into<String>,
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            device_authorization_url: device_authorization_url.into(),
            token_url: token_url.into(),
            refresh_url: None,
            scopes: collect_scopes(scopes),
        }
    }

    /// Sets the refresh URL.
    pub fn with_refresh_url(mut self, refresh_url: impl Into<String>) -> Self {
        self.refresh_url = Some(refresh_url.into());
        self
    }
}

/// Security requirement specifying which scheme and scopes are needed.
///
/// A security requirement references a security scheme by name and optionally
/// specifies required scopes (for OAuth2 schemes).
///
/// # Example
///
/// ```rust
/// use clawspec_core::SecurityRequirement;
///
/// // Simple requirement (no scopes)
/// let bearer_req = SecurityRequirement::new("bearerAuth");
///
/// // OAuth2 with required scopes
/// let oauth_req = SecurityRequirement::with_scopes("oauth2", ["read:users", "write:users"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityRequirement {
    /// Name of the security scheme (must match a registered scheme)
    pub name: String,
    /// Required scopes (empty for non-OAuth schemes)
    pub scopes: Vec<String>,
}

impl SecurityRequirement {
    /// Creates a new security requirement without scopes.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the security scheme
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityRequirement;
    ///
    /// let req = SecurityRequirement::new("bearerAuth");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scopes: Vec::new(),
        }
    }

    /// Creates a new security requirement with scopes.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the security scheme
    /// * `scopes` - Required OAuth2 scopes
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::SecurityRequirement;
    ///
    /// let req = SecurityRequirement::with_scopes("oauth2", ["read:users", "write:users"]);
    /// ```
    pub fn with_scopes(
        name: impl Into<String>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            scopes: scopes.into_iter().map(Into::into).collect(),
        }
    }

    /// Converts to utoipa SecurityRequirement.
    pub(crate) fn to_utoipa(&self) -> utoipa::openapi::security::SecurityRequirement {
        utoipa::openapi::security::SecurityRequirement::new(
            &self.name,
            self.scopes.iter().map(String::as_str),
        )
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;

    fn to_yaml(scheme: &SecurityScheme) -> String {
        serde_saphyr::to_string(&scheme.to_utoipa()).expect("should serialize to YAML")
    }

    #[test]
    fn should_convert_deprecated_api_key() {
        let scheme = SecurityScheme::api_key("X-Legacy-Key", ApiKeyLocation::Header)
            .with_description("Legacy key")
            .with_deprecated(true);

        assert_snapshot!(to_yaml(&scheme), @r"
        type: apiKey
        in: header
        name: X-Legacy-Key
        description: Legacy key
        deprecated: true
        ");
    }

    #[test]
    fn should_not_write_deprecated_when_false() {
        let scheme = SecurityScheme::bearer_with_format("JWT")
            .with_deprecated(true)
            .with_deprecated(false);

        assert_snapshot!(to_yaml(&scheme), @r"
        type: http
        scheme: bearer
        bearerFormat: JWT
        ");
    }

    #[test]
    fn should_convert_oauth2_with_metadata_url_and_device_flow() {
        let flows = OAuth2Flows::client_credentials(
            "https://auth.example.com/token",
            [("api:access", "API access")],
        )
        .with_device_authorization(
            OAuth2DeviceAuthorizationFlow::new(
                "https://auth.example.com/device",
                "https://auth.example.com/token",
                [("read:users", "Read user data")],
            )
            .with_refresh_url("https://auth.example.com/refresh"),
        );
        let scheme = SecurityScheme::oauth2_with_metadata_url(
            flows,
            "https://auth.example.com/.well-known/oauth-authorization-server",
        )
        .with_description("OAuth2")
        .with_deprecated(true);

        assert_snapshot!(to_yaml(&scheme), @r#"
        type: oauth2
        flows:
          clientCredentials:
            tokenUrl: https://auth.example.com/token
            scopes:
              "api:access": API access
          deviceAuthorization:
            deviceAuthorizationUrl: https://auth.example.com/device
            tokenUrl: https://auth.example.com/token
            refreshUrl: https://auth.example.com/refresh
            scopes:
              "read:users": Read user data
        oauth2MetadataUrl: https://auth.example.com/.well-known/oauth-authorization-server
        description: OAuth2
        deprecated: true
        "#);
    }

    #[test]
    fn should_convert_all_oauth2_flows() {
        let flows = OAuth2Flows::default()
            .with_authorization_code(
                OAuth2Flow::new("https://auth.example.com/token", [("read", "Read access")])
                    .with_authorization_url("https://auth.example.com/authorize")
                    .with_refresh_url("https://auth.example.com/refresh"),
            )
            .with_client_credentials(OAuth2Flow::new(
                "https://auth.example.com/token",
                [("api", "API access")],
            ))
            .with_implicit(
                OAuth2ImplicitFlow::new(
                    "https://auth.example.com/authorize",
                    [("implicit", "Implicit access")],
                )
                .with_refresh_url("https://auth.example.com/refresh"),
            )
            .with_password(OAuth2Flow::new(
                "https://auth.example.com/token",
                [("password", "Password access")],
            ))
            .with_device_authorization(OAuth2DeviceAuthorizationFlow::new(
                "https://auth.example.com/device",
                "https://auth.example.com/token",
                [("device", "Device access")],
            ));

        assert_snapshot!(to_yaml(&SecurityScheme::oauth2(flows)), @"
        type: oauth2
        flows:
          authorizationCode:
            authorizationUrl: https://auth.example.com/authorize
            tokenUrl: https://auth.example.com/token
            refreshUrl: https://auth.example.com/refresh
            scopes:
              read: Read access
          clientCredentials:
            tokenUrl: https://auth.example.com/token
            scopes:
              api: API access
          deviceAuthorization:
            deviceAuthorizationUrl: https://auth.example.com/device
            tokenUrl: https://auth.example.com/token
            scopes:
              device: Device access
          implicit:
            authorizationUrl: https://auth.example.com/authorize
            refreshUrl: https://auth.example.com/refresh
            scopes:
              implicit: Implicit access
          password:
            tokenUrl: https://auth.example.com/token
            scopes:
              password: Password access
        ");
    }

    #[test]
    fn should_convert_device_authorization_flows() {
        let flows = OAuth2Flows::device_authorization(
            "https://auth.example.com/device",
            "https://auth.example.com/token",
            [("read:users", "Read user data")],
        );

        assert_snapshot!(to_yaml(&SecurityScheme::oauth2(flows)), @r#"
        type: oauth2
        flows:
          deviceAuthorization:
            deviceAuthorizationUrl: https://auth.example.com/device
            tokenUrl: https://auth.example.com/token
            scopes:
              "read:users": Read user data
        "#);
    }

    #[test]
    fn test_bearer_scheme_creation() {
        let scheme = SecurityScheme::bearer();
        assert!(matches!(
            scheme,
            SecurityScheme::Bearer {
                format: None,
                description: None,
                deprecated: false,
                ..
            }
        ));
    }

    #[test]
    fn test_bearer_with_format() {
        let scheme = SecurityScheme::bearer_with_format("JWT");
        assert!(matches!(
            scheme,
            SecurityScheme::Bearer {
                format: Some(ref f),
                description: None,
                deprecated: false,
                ..
            } if f == "JWT"
        ));
    }

    #[test]
    fn test_basic_scheme_creation() {
        let scheme = SecurityScheme::basic();
        assert!(matches!(
            scheme,
            SecurityScheme::Basic {
                description: None,
                deprecated: false,
                ..
            }
        ));
    }

    #[test]
    fn test_api_key_scheme_creation() {
        let scheme = SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header);
        assert!(matches!(
            scheme,
            SecurityScheme::ApiKey {
                ref name,
                location: ApiKeyLocation::Header,
                description: None,
                deprecated: false,
                ..
            } if name == "X-API-Key"
        ));
    }

    #[test]
    fn test_with_description() {
        let scheme = SecurityScheme::bearer().with_description("JWT Bearer token");
        assert!(matches!(
            scheme,
            SecurityScheme::Bearer {
                format: None,
                description: Some(ref d),
                deprecated: false,
                ..
            } if d == "JWT Bearer token"
        ));
    }

    #[test]
    fn test_security_requirement_new() {
        let req = SecurityRequirement::new("bearerAuth");
        assert_eq!(req.name, "bearerAuth");
        assert!(req.scopes.is_empty());
    }

    #[test]
    fn test_security_requirement_with_scopes() {
        let req = SecurityRequirement::with_scopes("oauth2", ["read:users", "write:users"]);
        assert_eq!(req.name, "oauth2");
        assert_eq!(req.scopes, vec!["read:users", "write:users"]);
    }

    #[test]
    fn test_bearer_to_utoipa() {
        let scheme = SecurityScheme::bearer_with_format("JWT").with_description("JWT token");
        let utoipa_scheme = scheme.to_utoipa();

        assert!(matches!(utoipa_scheme, UtoipaSecurityScheme::Http(_)));
    }

    #[test]
    fn test_basic_to_utoipa() {
        let scheme = SecurityScheme::basic();
        let utoipa_scheme = scheme.to_utoipa();

        assert!(matches!(utoipa_scheme, UtoipaSecurityScheme::Http(_)));
    }

    #[test]
    fn test_api_key_to_utoipa() {
        let scheme = SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header);
        let utoipa_scheme = scheme.to_utoipa();

        assert!(matches!(utoipa_scheme, UtoipaSecurityScheme::ApiKey(_)));
    }

    #[test]
    fn test_openid_connect_to_utoipa() {
        let scheme = SecurityScheme::openid_connect("https://auth.example.com/.well-known/openid");
        let utoipa_scheme = scheme.to_utoipa();

        assert!(matches!(
            utoipa_scheme,
            UtoipaSecurityScheme::OpenIdConnect(_)
        ));
    }

    #[test]
    fn test_oauth2_authorization_code_flows() {
        let flows = OAuth2Flows::authorization_code(
            "https://auth.example.com/authorize",
            "https://auth.example.com/token",
            [("read:users", "Read user data")],
        );

        assert!(flows.authorization_code.is_some());
        assert!(flows.client_credentials.is_none());
    }

    #[test]
    fn test_oauth2_client_credentials_flows() {
        let flows = OAuth2Flows::client_credentials(
            "https://auth.example.com/token",
            [("api:access", "API access")],
        );

        assert!(flows.client_credentials.is_some());
        assert!(flows.authorization_code.is_none());
    }

    #[test]
    fn test_security_requirement_to_utoipa() {
        let req = SecurityRequirement::with_scopes("oauth2", ["read:users"]);
        let utoipa_req = req.to_utoipa();

        // Verify the requirement was created (internal structure)
        assert!(format!("{utoipa_req:?}").contains("oauth2"));
    }
}
