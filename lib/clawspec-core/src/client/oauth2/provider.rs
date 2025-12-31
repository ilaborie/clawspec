//! OAuth2 token provider for acquiring and refreshing tokens.

use std::time::Duration;

use oauth2::{AccessToken, TokenResponse};

use super::config::{OAuth2Config, OAuth2GrantType};
use super::error::OAuth2Error;
use super::token::OAuth2Token;

impl OAuth2Config {
    /// Acquires a new access token using the configured grant type.
    ///
    /// This method handles:
    /// - Client Credentials grant: fetches a new token from the token endpoint
    /// - Pre-Acquired token: returns the cached token if available
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Token endpoint returns an error
    /// - Response cannot be parsed
    pub async fn acquire_token(&self) -> Result<OAuth2Token, OAuth2Error> {
        match self.grant_type {
            OAuth2GrantType::ClientCredentials => self.acquire_client_credentials_token().await,
            OAuth2GrantType::PreAcquired => self.get_pre_acquired_token().await,
        }
    }

    /// Acquires a token using the Client Credentials grant.
    async fn acquire_client_credentials_token(&self) -> Result<OAuth2Token, OAuth2Error> {
        // Create HTTP client with redirect disabled for SSRF prevention
        // Use oauth2::reqwest to ensure version compatibility
        let http_client = oauth2::reqwest::ClientBuilder::new()
            .redirect(oauth2::reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| OAuth2Error::TokenAcquisitionFailed {
                reason: format!("Failed to create HTTP client: {e}"),
            })?;

        self.acquire_client_credentials_token_with_client(&http_client)
            .await
    }

    /// Internal method for acquiring tokens with a custom HTTP client.
    ///
    /// This enables testing without making real network requests by injecting
    /// mock HTTP clients that return predefined responses.
    pub(crate) async fn acquire_client_credentials_token_with_client(
        &self,
        http_client: &oauth2::reqwest::Client,
    ) -> Result<OAuth2Token, OAuth2Error> {
        use oauth2::basic::BasicClient;
        use oauth2::{AuthUrl, ClientId, ClientSecret, Scope, TokenUrl};

        let client_id = ClientId::new(self.client_id.clone());

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

        // Build client using the new builder pattern (oauth2 5.x)
        // The type-state pattern ensures exchange_client_credentials() is available
        // only after set_token_uri() is called
        let mut client = BasicClient::new(client_id)
            .set_auth_uri(auth_url)
            .set_token_uri(token_url);

        // Set client secret if provided
        if let Some(ref secret) = self.client_secret {
            client = client.set_client_secret(ClientSecret::new(secret.as_str().to_string()));
        }

        let mut request = client.exchange_client_credentials();

        // Add scopes
        for scope in self.scopes.iter().map(|s| Scope::new(s.clone())) {
            request = request.add_scope(scope);
        }

        // Execute the request
        let token_result = request.request_async(http_client).await.map_err(|e| {
            OAuth2Error::TokenAcquisitionFailed {
                reason: format!("{e}"),
            }
        })?;

        // Convert to our token type
        let token =
            Self::convert_token_response(token_result.access_token(), token_result.expires_in());

        // Cache the token
        self.set_token(token.clone()).await;

        Ok(token)
    }

    /// Returns the pre-acquired token if available.
    async fn get_pre_acquired_token(&self) -> Result<OAuth2Token, OAuth2Error> {
        self.get_token().await.ok_or(OAuth2Error::TokenExpired)
    }

    /// Converts an oauth2 token response to our token type.
    fn convert_token_response(
        access_token: &AccessToken,
        expires_in: Option<Duration>,
    ) -> OAuth2Token {
        if let Some(duration) = expires_in {
            OAuth2Token::with_expiry(access_token.secret().clone(), duration)
        } else {
            OAuth2Token::new(access_token.secret().clone())
        }
    }

    /// Gets a valid token, acquiring a new one if necessary.
    ///
    /// This is the main entry point for getting an access token.
    /// It checks the cache first and only acquires a new token if needed.
    pub async fn get_valid_token(&self) -> Result<OAuth2Token, OAuth2Error> {
        // Check if we have a valid cached token
        if !self.needs_token().await
            && let Some(token) = self.get_token().await
        {
            return Ok(token);
        }

        // Need to acquire a new token
        self.acquire_token().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================
    // Pre-acquired token tests
    // =========================================

    #[tokio::test]
    async fn should_return_pre_acquired_token() {
        let config = OAuth2Config::pre_acquired(
            "client-id",
            "https://auth.example.com/token",
            "pre-acquired-access-token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let token = config.get_valid_token().await.expect("Should get token");
        assert_eq!(token.access_token(), "pre-acquired-access-token");
    }

    #[tokio::test]
    async fn should_fail_when_no_pre_acquired_token() {
        let config =
            OAuth2Config::pre_acquired("client-id", "https://auth.example.com/token", "token")
                .expect("Should create builder")
                .build()
                .expect("Should build config");

        config.token_cache.clear().await;

        let result = config.get_pre_acquired_token().await;
        assert!(result.is_err());
        match result.expect_err("Should fail") {
            OAuth2Error::TokenExpired => {}
            _ => panic!("Expected TokenExpired error"),
        }
    }

    // =========================================
    // Cache behavior tests
    // =========================================

    #[tokio::test]
    async fn should_return_cached_token_without_network_call() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        // Pre-populate cache with a valid token (long expiry)
        let token = OAuth2Token::with_expiry("cached-valid-token", Duration::from_secs(3600));
        config.set_token(token).await;

        // get_valid_token should return cached token
        let result = config.get_valid_token().await;
        let token = result.expect("Should return cached token");
        assert_eq!(token.access_token(), "cached-valid-token");
    }

    // =========================================
    // convert_token_response tests
    // =========================================

    #[test]
    fn should_convert_token_with_expiry() {
        let access_token = oauth2::AccessToken::new("test-token".to_string());
        let expires_in = Some(Duration::from_secs(3600));

        let token = OAuth2Config::convert_token_response(&access_token, expires_in);

        assert_eq!(token.access_token(), "test-token");
        assert!(token.time_until_expiry().is_some());
    }

    #[test]
    fn should_convert_token_without_expiry() {
        let access_token = oauth2::AccessToken::new("no-expiry".to_string());
        let expires_in = None;

        let token = OAuth2Config::convert_token_response(&access_token, expires_in);

        assert_eq!(token.access_token(), "no-expiry");
        assert!(token.time_until_expiry().is_none());
    }

    // =========================================
    // Scope configuration tests
    // =========================================

    #[tokio::test]
    async fn should_configure_scopes() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .add_scope("read:users")
        .add_scope("write:users")
        .build()
        .expect("Should build config");

        assert_eq!(config.scopes, vec!["read:users", "write:users"]);
    }
}
