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
use utoipa::openapi::security::{
    ApiKey as UtoipaApiKey, ApiKeyValue, AuthorizationCode, ClientCredentials, Flow, Http,
    HttpAuthScheme, Implicit, OAuth2 as UtoipaOAuth2, OpenIdConnect as UtoipaOpenIdConnect,
    Password, Scopes, SecurityScheme as UtoipaSecurityScheme,
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
    Bearer {
        /// Optional format hint (e.g., "JWT" for JSON Web Tokens)
        format: Option<String>,
        /// Description for documentation
        description: Option<String>,
    },

    /// HTTP Basic authentication (RFC 7617).
    ///
    /// Uses `Authorization: Basic <base64(username:password)>` header.
    Basic {
        /// Description for documentation
        description: Option<String>,
    },

    /// API Key authentication.
    ///
    /// The API key can be passed in a header, query parameter, or cookie.
    ApiKey {
        /// Name of the header, query parameter, or cookie
        name: String,
        /// Where the API key is passed
        location: ApiKeyLocation,
        /// Description for documentation
        description: Option<String>,
    },

    /// OAuth 2.0 authentication.
    ///
    /// Supports multiple OAuth2 flows: authorization code, client credentials,
    /// implicit, and password.
    OAuth2 {
        /// OAuth2 flows configuration (boxed to reduce enum size)
        flows: Box<OAuth2Flows>,
        /// Description for documentation
        description: Option<String>,
    },

    /// OpenID Connect Discovery.
    ///
    /// Uses OpenID Connect for authentication with automatic discovery
    /// of the provider's configuration.
    OpenIdConnect {
        /// OpenID Connect discovery URL
        open_id_connect_url: String,
        /// Description for documentation
        description: Option<String>,
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
        Self::Basic { description: None }
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
            SecurityScheme::Bearer {
                description: desc, ..
            } => *desc = Some(description.into()),
            SecurityScheme::Basic { description: desc } => *desc = Some(description.into()),
            SecurityScheme::ApiKey {
                description: desc, ..
            } => *desc = Some(description.into()),
            SecurityScheme::OAuth2 {
                description: desc, ..
            } => *desc = Some(description.into()),
            SecurityScheme::OpenIdConnect {
                description: desc, ..
            } => *desc = Some(description.into()),
        }
        self
    }

    /// Converts this security scheme to a utoipa SecurityScheme.
    pub(crate) fn to_utoipa(&self) -> UtoipaSecurityScheme {
        match self {
            SecurityScheme::Bearer {
                format,
                description,
            } => {
                let mut http = Http::new(HttpAuthScheme::Bearer);
                if let Some(fmt) = format {
                    http.bearer_format = Some(fmt.clone());
                }
                if let Some(desc) = description {
                    http.description = Some(desc.clone());
                }
                UtoipaSecurityScheme::Http(http)
            }
            SecurityScheme::Basic { description } => {
                let mut http = Http::new(HttpAuthScheme::Basic);
                if let Some(desc) = description {
                    http.description = Some(desc.clone());
                }
                UtoipaSecurityScheme::Http(http)
            }
            SecurityScheme::ApiKey {
                name,
                location,
                description,
            } => {
                let api_key_value = if let Some(desc) = description {
                    ApiKeyValue::with_description(name, desc)
                } else {
                    ApiKeyValue::new(name)
                };
                let api_key = match location {
                    ApiKeyLocation::Header => UtoipaApiKey::Header(api_key_value),
                    ApiKeyLocation::Query => UtoipaApiKey::Query(api_key_value),
                    ApiKeyLocation::Cookie => UtoipaApiKey::Cookie(api_key_value),
                };
                UtoipaSecurityScheme::ApiKey(api_key)
            }
            SecurityScheme::OAuth2 { flows, description } => {
                let mut oauth2 = flows.to_utoipa();
                if let Some(desc) = description {
                    oauth2.description = Some(desc.clone());
                }
                UtoipaSecurityScheme::OAuth2(oauth2)
            }
            SecurityScheme::OpenIdConnect {
                open_id_connect_url,
                description,
            } => {
                let mut oidc = UtoipaOpenIdConnect::new(open_id_connect_url);
                if let Some(desc) = description {
                    oidc.description = Some(desc.clone());
                }
                UtoipaSecurityScheme::OpenIdConnect(oidc)
            }
        }
    }
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OAuth2Flows {
    /// Authorization Code flow
    pub authorization_code: Option<OAuth2Flow>,
    /// Client Credentials flow
    pub client_credentials: Option<OAuth2Flow>,
    /// Implicit flow (deprecated in OAuth 2.1)
    pub implicit: Option<OAuth2ImplicitFlow>,
    /// Password flow (deprecated in OAuth 2.1)
    pub password: Option<OAuth2Flow>,
}

impl OAuth2Flows {
    /// Creates a new OAuth2Flows with authorization code flow.
    pub fn authorization_code(
        authorization_url: impl Into<String>,
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            authorization_code: Some(OAuth2Flow {
                authorization_url: Some(authorization_url.into()),
                token_url: token_url.into(),
                refresh_url: None,
                scopes: scopes
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect(),
            }),
            ..Default::default()
        }
    }

    /// Creates a new OAuth2Flows with client credentials flow.
    pub fn client_credentials(
        token_url: impl Into<String>,
        scopes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            client_credentials: Some(OAuth2Flow {
                authorization_url: None,
                token_url: token_url.into(),
                refresh_url: None,
                scopes: scopes
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect(),
            }),
            ..Default::default()
        }
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

        UtoipaOAuth2::new(flows)
    }
}

/// OAuth2 flow configuration (for flows with token URL).
#[derive(Debug, Clone, PartialEq)]
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

/// OAuth2 implicit flow configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct OAuth2ImplicitFlow {
    /// Authorization URL
    pub authorization_url: String,
    /// Refresh URL (optional)
    pub refresh_url: Option<String>,
    /// Available scopes
    pub scopes: IndexMap<String, String>,
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
    use super::*;

    #[test]
    fn test_bearer_scheme_creation() {
        let scheme = SecurityScheme::bearer();
        assert!(matches!(
            scheme,
            SecurityScheme::Bearer {
                format: None,
                description: None
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
                description: None
            } if f == "JWT"
        ));
    }

    #[test]
    fn test_basic_scheme_creation() {
        let scheme = SecurityScheme::basic();
        assert!(matches!(
            scheme,
            SecurityScheme::Basic { description: None }
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
                description: None
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
                description: Some(ref d)
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
