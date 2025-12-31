//! OAuth2 token provider for acquiring and refreshing tokens.

use std::future::Future;
use std::time::Duration;

use oauth2::reqwest::async_http_client;
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
        self.acquire_client_credentials_token_with_client(async_http_client)
            .await
    }

    /// Internal method for acquiring tokens with a custom HTTP client.
    ///
    /// This enables testing without making real network requests by injecting
    /// mock HTTP clients that return predefined responses.
    async fn acquire_client_credentials_token_with_client<F, RE, Fut>(
        &self,
        http_client: F,
    ) -> Result<OAuth2Token, OAuth2Error>
    where
        F: FnOnce(oauth2::HttpRequest) -> Fut + Send,
        RE: std::error::Error + 'static + Send,
        Fut: Future<Output = Result<oauth2::HttpResponse, RE>> + Send,
    {
        let client = self.create_oauth2_client()?;

        let mut request = client.exchange_client_credentials();

        // Add scopes
        for scope in self.oauth2_scopes() {
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
mod test_helpers {
    //! Test utilities for mocking OAuth2 HTTP responses.

    // Use http types re-exported by oauth2 to avoid version conflicts
    use oauth2::http::{HeaderMap, StatusCode};

    /// Creates a successful OAuth2 token response body.
    pub fn token_response_body(access_token: &str, expires_in: Option<u64>) -> Vec<u8> {
        let json = match expires_in {
            Some(exp) => serde_json::json!({
                "access_token": access_token,
                "token_type": "Bearer",
                "expires_in": exp
            }),
            None => serde_json::json!({
                "access_token": access_token,
                "token_type": "Bearer"
            }),
        };
        serde_json::to_vec(&json).expect("JSON serialization should succeed")
    }

    /// Creates an OAuth2 error response body.
    pub fn error_response_body(error: &str, description: &str) -> Vec<u8> {
        let json = serde_json::json!({
            "error": error,
            "error_description": description
        });
        serde_json::to_vec(&json).expect("JSON serialization should succeed")
    }

    /// Creates a mock HTTP client that returns a successful token response.
    pub fn mock_success_client(
        access_token: &str,
        expires_in: Option<u64>,
    ) -> impl FnOnce(
        oauth2::HttpRequest,
    ) -> std::future::Ready<Result<oauth2::HttpResponse, std::io::Error>> {
        let body = token_response_body(access_token, expires_in);
        move |_request| {
            std::future::ready(Ok(oauth2::HttpResponse {
                status_code: StatusCode::OK,
                headers: HeaderMap::new(),
                body,
            }))
        }
    }

    /// Creates a mock HTTP client that returns a network error.
    pub fn mock_network_error_client() -> impl FnOnce(
        oauth2::HttpRequest,
    ) -> std::future::Ready<
        Result<oauth2::HttpResponse, std::io::Error>,
    > {
        |_request| {
            std::future::ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "Connection refused",
            )))
        }
    }

    /// Creates a mock HTTP client that returns an OAuth2 error response.
    pub fn mock_oauth2_error_client(
        error: &str,
        description: &str,
    ) -> impl FnOnce(
        oauth2::HttpRequest,
    ) -> std::future::Ready<Result<oauth2::HttpResponse, std::io::Error>> {
        let body = error_response_body(error, description);
        move |_request| {
            std::future::ready(Ok(oauth2::HttpResponse {
                status_code: StatusCode::BAD_REQUEST,
                headers: HeaderMap::new(),
                body,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;

    // =========================================
    // Pre-acquired token tests (existing)
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
    // Client credentials token acquisition tests
    // =========================================

    #[tokio::test]
    async fn should_acquire_token_with_expiry() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let result = config
            .acquire_client_credentials_token_with_client(mock_success_client(
                "test-access-token",
                Some(3600),
            ))
            .await;

        let token = result.expect("Should acquire token");
        assert_eq!(token.access_token(), "test-access-token");
        assert!(token.time_until_expiry().is_some());
    }

    #[tokio::test]
    async fn should_acquire_token_without_expiry() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let result = config
            .acquire_client_credentials_token_with_client(mock_success_client(
                "no-expiry-token",
                None,
            ))
            .await;

        let token = result.expect("Should acquire token");
        assert_eq!(token.access_token(), "no-expiry-token");
        assert!(token.time_until_expiry().is_none());
    }

    #[tokio::test]
    async fn should_cache_acquired_token() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let _ = config
            .acquire_client_credentials_token_with_client(mock_success_client(
                "cached-token",
                Some(3600),
            ))
            .await
            .expect("Should acquire token");

        let cached = config.get_token().await.expect("Token should be cached");
        assert_eq!(cached.access_token(), "cached-token");
    }

    #[tokio::test]
    async fn should_acquire_token_with_scopes() {
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

        let result = config
            .acquire_client_credentials_token_with_client(mock_success_client(
                "scoped-token",
                Some(3600),
            ))
            .await;

        assert!(result.is_ok());
        assert_eq!(config.scopes, vec!["read:users", "write:users"]);
    }

    // =========================================
    // Error handling tests
    // =========================================

    #[tokio::test]
    async fn should_handle_network_error() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let result = config
            .acquire_client_credentials_token_with_client(mock_network_error_client())
            .await;

        let err = result.expect_err("Should fail with network error");
        match err {
            OAuth2Error::TokenAcquisitionFailed { reason } => {
                // Network errors are wrapped by oauth2 crate
                assert!(
                    !reason.is_empty(),
                    "Error reason should not be empty: {reason}"
                );
            }
            _ => panic!("Expected TokenAcquisitionFailed error"),
        }
    }

    #[tokio::test]
    async fn should_handle_oauth2_error_response() {
        let config = OAuth2Config::client_credentials(
            "test-client",
            "test-secret",
            "https://auth.example.com/token",
        )
        .expect("Should create builder")
        .build()
        .expect("Should build config");

        let result = config
            .acquire_client_credentials_token_with_client(mock_oauth2_error_client(
                "invalid_client",
                "Client authentication failed",
            ))
            .await;

        let err = result.expect_err("Should fail with OAuth2 error");
        match err {
            OAuth2Error::TokenAcquisitionFailed { reason } => {
                // OAuth2 error response is parsed by the oauth2 crate
                assert!(
                    !reason.is_empty(),
                    "Error reason should not be empty: {reason}"
                );
            }
            _ => panic!("Expected TokenAcquisitionFailed error"),
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
}
