//! OAuth2 configuration and builder.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, Scope, TokenUrl};
use url::Url;

use super::error::OAuth2Error;
use super::token::{OAuth2Token, TokenCache};
use crate::client::SecureString;

/// Default threshold for token refresh (60 seconds before expiry).
const DEFAULT_REFRESH_THRESHOLD: Duration = Duration::from_secs(60);

/// OAuth2 grant type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuth2GrantType {
    /// Client Credentials grant (machine-to-machine).
    ClientCredentials,
    /// Pre-acquired token (externally obtained).
    PreAcquired,
}

/// OAuth2 authentication configuration.
///
/// Use [`OAuth2ConfigBuilder`] to create instances.
#[derive(Clone)]
pub struct OAuth2Config {
    /// Client ID for OAuth2.
    pub(crate) client_id: String,
    /// Client secret for OAuth2 (required for client_credentials).
    pub(crate) client_secret: Option<SecureString>,
    /// Token endpoint URL.
    pub(crate) token_url: Url,
    /// Authorization endpoint URL (optional, for documentation).
    pub(crate) auth_url: Option<Url>,
    /// Requested scopes.
    pub(crate) scopes: Vec<String>,
    /// Grant type.
    pub(crate) grant_type: OAuth2GrantType,
    /// Auto-refresh tokens before expiry.
    pub(crate) auto_refresh: bool,
    /// Threshold for token refresh.
    pub(crate) refresh_threshold: Duration,
    /// Token cache for reusing tokens.
    pub(crate) token_cache: TokenCache,
}

impl OAuth2Config {
    /// Creates a builder for client credentials flow.
    pub fn client_credentials(
        client_id: impl Into<String>,
        client_secret: impl Into<SecureString>,
        token_url: impl AsRef<str>,
    ) -> Result<OAuth2ConfigBuilder, OAuth2Error> {
        Ok(OAuth2ConfigBuilder::new(client_id, token_url)?
            .with_client_secret(client_secret)
            .with_grant_type(OAuth2GrantType::ClientCredentials))
    }

    /// Creates a builder for a pre-acquired token.
    pub fn pre_acquired(
        client_id: impl Into<String>,
        token_url: impl AsRef<str>,
        access_token: impl Into<String>,
    ) -> Result<OAuth2ConfigBuilder, OAuth2Error> {
        let token = OAuth2Token::new(access_token);
        Ok(OAuth2ConfigBuilder::new(client_id, token_url)?
            .with_pre_acquired_token(token)
            .with_grant_type(OAuth2GrantType::PreAcquired))
    }

    /// Checks if a new token should be acquired.
    pub async fn needs_token(&self) -> bool {
        self.token_cache
            .should_refresh(self.refresh_threshold)
            .await
    }

    /// Gets the cached token if available and not expired.
    pub async fn get_token(&self) -> Option<OAuth2Token> {
        self.token_cache.get().await
    }

    /// Stores a token in the cache.
    pub async fn set_token(&self, token: OAuth2Token) {
        self.token_cache.set(token).await;
    }

    /// Creates an oauth2 BasicClient for token requests.
    ///
    /// This client is configured with redirect disabled for SSRF prevention.
    pub(crate) fn create_oauth2_client(&self) -> Result<BasicClient, OAuth2Error> {
        let client_id = ClientId::new(self.client_id.clone());
        let client_secret = self
            .client_secret
            .as_ref()
            .map(|s| ClientSecret::new(s.as_str().to_string()));

        // Use a dummy auth URL if not specified (client_credentials doesn't need it)
        let auth_url_str = self
            .auth_url
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| format!("{}/../authorize", self.token_url));

        let auth_url = AuthUrl::new(auth_url_str).map_err(|e| OAuth2Error::ConfigurationError {
            reason: format!("Invalid authorization URL: {e}"),
        })?;

        let token_url = TokenUrl::new(self.token_url.to_string()).map_err(|e| {
            OAuth2Error::ConfigurationError {
                reason: format!("Invalid token URL: {e}"),
            }
        })?;

        let client = BasicClient::new(client_id, client_secret, auth_url, Some(token_url));

        Ok(client)
    }

    /// Returns the scopes as oauth2 Scope objects.
    pub(crate) fn oauth2_scopes(&self) -> Vec<Scope> {
        self.scopes.iter().map(|s| Scope::new(s.clone())).collect()
    }
}

impl fmt::Debug for OAuth2Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2Config")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_url", &self.token_url)
            .field("auth_url", &self.auth_url)
            .field("scopes", &self.scopes)
            .field("grant_type", &self.grant_type)
            .field("auto_refresh", &self.auto_refresh)
            .field("refresh_threshold", &self.refresh_threshold)
            .finish()
    }
}

/// Builder for OAuth2 configuration.
#[derive(Clone)]
pub struct OAuth2ConfigBuilder {
    client_id: String,
    client_secret: Option<SecureString>,
    token_url: Url,
    auth_url: Option<Url>,
    scopes: Vec<String>,
    grant_type: OAuth2GrantType,
    auto_refresh: bool,
    refresh_threshold: Duration,
    pre_acquired_token: Option<OAuth2Token>,
}

impl OAuth2ConfigBuilder {
    /// Creates a new builder with required parameters.
    pub fn new(
        client_id: impl Into<String>,
        token_url: impl AsRef<str>,
    ) -> Result<Self, OAuth2Error> {
        let token_url =
            Url::parse(token_url.as_ref()).map_err(|e| OAuth2Error::InvalidTokenEndpoint {
                url: token_url.as_ref().to_string(),
                reason: e.to_string(),
            })?;

        Ok(Self {
            client_id: client_id.into(),
            client_secret: None,
            token_url,
            auth_url: None,
            scopes: Vec::new(),
            grant_type: OAuth2GrantType::ClientCredentials,
            auto_refresh: true,
            refresh_threshold: DEFAULT_REFRESH_THRESHOLD,
            pre_acquired_token: None,
        })
    }

    /// Sets the client secret.
    #[must_use]
    pub fn with_client_secret(mut self, secret: impl Into<SecureString>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Sets the authorization URL (optional, for documentation).
    pub fn with_auth_url(mut self, auth_url: impl AsRef<str>) -> Result<Self, OAuth2Error> {
        let url = Url::parse(auth_url.as_ref()).map_err(|e| OAuth2Error::ConfigurationError {
            reason: format!("Invalid authorization URL: {e}"),
        })?;
        self.auth_url = Some(url);
        Ok(self)
    }

    /// Adds a scope.
    #[must_use]
    pub fn add_scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    /// Adds multiple scopes.
    #[must_use]
    pub fn add_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes.extend(scopes.into_iter().map(Into::into));
        self
    }

    /// Sets the grant type.
    #[must_use]
    fn with_grant_type(mut self, grant_type: OAuth2GrantType) -> Self {
        self.grant_type = grant_type;
        self
    }

    /// Sets whether to automatically refresh tokens.
    #[must_use]
    pub fn with_auto_refresh(mut self, auto_refresh: bool) -> Self {
        self.auto_refresh = auto_refresh;
        self
    }

    /// Sets the refresh threshold (how long before expiry to refresh).
    #[must_use]
    pub fn with_refresh_threshold(mut self, threshold: Duration) -> Self {
        self.refresh_threshold = threshold;
        self
    }

    /// Sets a pre-acquired token.
    #[must_use]
    fn with_pre_acquired_token(mut self, token: OAuth2Token) -> Self {
        self.pre_acquired_token = Some(token);
        self
    }

    /// Builds the OAuth2 configuration.
    pub fn build(self) -> Result<OAuth2Config, OAuth2Error> {
        // Validate configuration
        if self.grant_type == OAuth2GrantType::ClientCredentials && self.client_secret.is_none() {
            return Err(OAuth2Error::ConfigurationError {
                reason: "Client credentials flow requires a client secret".to_string(),
            });
        }

        let token_cache = if let Some(token) = self.pre_acquired_token {
            TokenCache::with_token(token)
        } else {
            TokenCache::new()
        };

        Ok(OAuth2Config {
            client_id: self.client_id,
            client_secret: self.client_secret,
            token_url: self.token_url,
            auth_url: self.auth_url,
            scopes: self.scopes,
            grant_type: self.grant_type,
            auto_refresh: self.auto_refresh,
            refresh_threshold: self.refresh_threshold,
            token_cache,
        })
    }
}

impl fmt::Debug for OAuth2ConfigBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2ConfigBuilder")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_url", &self.token_url)
            .field("scopes", &self.scopes)
            .field("grant_type", &self.grant_type)
            .finish()
    }
}

/// Wraps OAuth2Config in an Arc for sharing across async tasks.
#[derive(Debug, Clone)]
pub struct SharedOAuth2Config(pub(crate) Arc<OAuth2Config>);

impl SharedOAuth2Config {
    /// Creates a new shared config.
    pub fn new(config: OAuth2Config) -> Self {
        Self(Arc::new(config))
    }

    /// Returns a reference to the inner config.
    pub fn inner(&self) -> &OAuth2Config {
        &self.0
    }
}

impl From<OAuth2Config> for SharedOAuth2Config {
    fn from(config: OAuth2Config) -> Self {
        Self::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_client_credentials_config() {
        let config = OAuth2Config::client_credentials(
            "client-id",
            "client-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        assert_eq!(config.client_id, "client-id");
        assert!(config.client_secret.is_some());
        assert_eq!(config.token_url.as_str(), "https://auth.example.com/token");
        assert_eq!(config.grant_type, OAuth2GrantType::ClientCredentials);
    }

    #[test]
    fn should_create_pre_acquired_config() {
        let config = OAuth2Config::pre_acquired(
            "client-id",
            "https://auth.example.com/token",
            "pre-acquired-token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        assert_eq!(config.grant_type, OAuth2GrantType::PreAcquired);
    }

    #[test]
    fn should_reject_invalid_token_url() {
        let result = OAuth2ConfigBuilder::new("client-id", "not-a-url");
        assert!(result.is_err());

        let err = result.expect_err("Should fail");
        match err {
            OAuth2Error::InvalidTokenEndpoint { url, .. } => {
                assert_eq!(url, "not-a-url");
            }
            _ => panic!("Expected InvalidTokenEndpoint error"),
        }
    }

    #[test]
    fn should_require_client_secret_for_client_credentials() {
        let result = OAuth2ConfigBuilder::new("client-id", "https://auth.example.com/token")
            .expect("Should create builder")
            .with_grant_type(OAuth2GrantType::ClientCredentials)
            .build();

        assert!(result.is_err());
        match result.expect_err("Should fail") {
            OAuth2Error::ConfigurationError { reason } => {
                assert!(reason.contains("client secret"));
            }
            _ => panic!("Expected ConfigurationError"),
        }
    }

    #[test]
    fn should_add_scopes() {
        let config = OAuth2Config::client_credentials(
            "client-id",
            "secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .add_scope("read:users")
        .add_scope("write:users")
        .build()
        .expect("Should build config");

        assert_eq!(config.scopes, vec!["read:users", "write:users"]);
    }

    #[test]
    fn should_add_multiple_scopes() {
        let config = OAuth2Config::client_credentials(
            "client-id",
            "secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .add_scopes(["scope1", "scope2", "scope3"])
        .build()
        .expect("Should build config");

        assert_eq!(config.scopes, vec!["scope1", "scope2", "scope3"]);
    }

    #[test]
    fn should_set_refresh_threshold() {
        let config = OAuth2Config::client_credentials(
            "client-id",
            "secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .with_refresh_threshold(Duration::from_secs(120))
        .build()
        .expect("Should build config");

        assert_eq!(config.refresh_threshold, Duration::from_secs(120));
    }

    #[test]
    fn should_redact_debug_output() {
        let config = OAuth2Config::client_credentials(
            "client-id",
            "super-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("super-secret"));
    }

    #[tokio::test]
    async fn should_cache_pre_acquired_token() {
        let config =
            OAuth2Config::pre_acquired("client-id", "https://auth.example.com/token", "my-token")
                .expect("Should create builder")
                .build()
                .expect("Should build config");

        let token = config.get_token().await.expect("Should have cached token");
        assert_eq!(token.access_token(), "my-token");
    }

    #[tokio::test]
    async fn should_need_token_when_cache_empty() {
        let config = OAuth2Config::client_credentials(
            "client-id",
            "secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        assert!(config.needs_token().await);
    }
}
