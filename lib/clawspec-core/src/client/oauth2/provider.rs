//! OAuth2 token provider for acquiring and refreshing tokens.

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
        let client = self.create_oauth2_client()?;

        let mut request = client.exchange_client_credentials();

        // Add scopes
        for scope in self.oauth2_scopes() {
            request = request.add_scope(scope);
        }

        // Execute the request
        let token_result = request
            .request_async(async_http_client)
            .await
            .map_err(|e| OAuth2Error::TokenAcquisitionFailed {
                reason: format!("{e}"),
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
        // Create a config without a pre-acquired token
        let config =
            OAuth2Config::pre_acquired("client-id", "https://auth.example.com/token", "token")
                .expect("Should create builder")
                .build()
                .expect("Should build config");

        // Clear the cache
        config.token_cache.clear().await;

        // Should fail because token is not available
        let result = config.get_pre_acquired_token().await;
        assert!(result.is_err());
        match result.expect_err("Should fail") {
            OAuth2Error::TokenExpired => {}
            _ => panic!("Expected TokenExpired error"),
        }
    }
}
