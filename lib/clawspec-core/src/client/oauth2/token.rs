//! OAuth2 token types and caching.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// An OAuth2 access token with expiration tracking.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct OAuth2Token {
    /// The access token value.
    access_token: String,
    /// When the token expires (if known).
    #[zeroize(skip)]
    expires_at: Option<Instant>,
    /// Optional refresh token for obtaining new access tokens.
    refresh_token: Option<String>,
}

impl OAuth2Token {
    /// Creates a new OAuth2 token.
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            expires_at: None,
            refresh_token: None,
        }
    }

    /// Creates a new OAuth2 token with an expiration time.
    pub fn with_expiry(access_token: impl Into<String>, expires_in: Duration) -> Self {
        Self {
            access_token: access_token.into(),
            expires_at: Some(Instant::now() + expires_in),
            refresh_token: None,
        }
    }

    /// Sets the refresh token.
    #[must_use]
    pub fn with_refresh_token(mut self, refresh_token: impl Into<String>) -> Self {
        self.refresh_token = Some(refresh_token.into());
        self
    }

    /// Returns the access token value.
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Returns the refresh token if available.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Checks if the token is expired.
    ///
    /// Returns `false` if the token has no expiration time.
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Instant::now() >= exp)
    }

    /// Checks if the token should be refreshed.
    ///
    /// Returns `true` if the token will expire within the given threshold.
    pub fn should_refresh(&self, threshold: Duration) -> bool {
        self.expires_at
            .is_some_and(|exp| Instant::now() + threshold >= exp)
    }

    /// Returns the time until expiration, if known.
    pub fn time_until_expiry(&self) -> Option<Duration> {
        self.expires_at.and_then(|exp| {
            let now = Instant::now();
            if now >= exp { None } else { Some(exp - now) }
        })
    }
}

impl fmt::Debug for OAuth2Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2Token")
            .field("access_token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// Thread-safe cache for OAuth2 tokens.
///
/// This cache ensures that only one token refresh happens at a time,
/// even when multiple requests are made concurrently.
#[derive(Debug, Clone, Default)]
pub struct TokenCache {
    inner: Arc<RwLock<Option<OAuth2Token>>>,
}

impl TokenCache {
    /// Creates a new empty token cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a token cache with an initial token.
    pub fn with_token(token: OAuth2Token) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Some(token))),
        }
    }

    /// Returns the cached token if it exists and is not expired.
    pub async fn get(&self) -> Option<OAuth2Token> {
        let guard = self.inner.read().await;
        guard.as_ref().filter(|t| !t.is_expired()).cloned()
    }

    /// Returns `true` if the token should be refreshed.
    ///
    /// A token should be refreshed if:
    /// - No token is cached
    /// - The token is expired
    /// - The token will expire within the threshold
    pub async fn should_refresh(&self, threshold: Duration) -> bool {
        let guard = self.inner.read().await;
        match guard.as_ref() {
            None => true,
            Some(token) => token.should_refresh(threshold),
        }
    }

    /// Stores a new token in the cache.
    pub async fn set(&self, token: OAuth2Token) {
        let mut guard = self.inner.write().await;
        *guard = Some(token);
    }

    /// Clears the cached token.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn clear(&self) {
        let mut guard = self.inner.write().await;
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_token() {
        let token = OAuth2Token::new("access-token-123");
        assert_eq!(token.access_token(), "access-token-123");
        assert!(token.refresh_token().is_none());
        assert!(!token.is_expired());
    }

    #[test]
    fn should_create_token_with_expiry() {
        let token = OAuth2Token::with_expiry("token", Duration::from_secs(3600));
        assert!(!token.is_expired());
        assert!(token.time_until_expiry().is_some());
    }

    #[test]
    fn should_detect_expired_token() {
        let token = OAuth2Token::with_expiry("token", Duration::ZERO);
        // Token created with zero duration is immediately expired
        assert!(token.is_expired());
    }

    #[test]
    fn should_detect_refresh_needed() {
        // Token expires in 30 seconds
        let token = OAuth2Token::with_expiry("token", Duration::from_secs(30));

        // Should refresh if threshold is 60 seconds
        assert!(token.should_refresh(Duration::from_secs(60)));

        // Should not refresh if threshold is 10 seconds
        assert!(!token.should_refresh(Duration::from_secs(10)));
    }

    #[test]
    fn should_add_refresh_token() {
        let token = OAuth2Token::new("access").with_refresh_token("refresh");
        assert_eq!(token.refresh_token(), Some("refresh"));
    }

    #[test]
    fn should_redact_debug_output() {
        let token = OAuth2Token::new("secret-token").with_refresh_token("secret-refresh");
        let debug_str = format!("{token:?}");
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("secret-token"));
        assert!(!debug_str.contains("secret-refresh"));
    }

    #[tokio::test]
    async fn should_cache_token() {
        let cache = TokenCache::new();
        assert!(cache.get().await.is_none());

        let token = OAuth2Token::new("cached-token");
        cache.set(token).await;

        let cached = cache.get().await.expect("Token should be cached");
        assert_eq!(cached.access_token(), "cached-token");
    }

    #[tokio::test]
    async fn should_not_return_expired_token() {
        let cache = TokenCache::new();
        let token = OAuth2Token::with_expiry("expired", Duration::ZERO);
        cache.set(token).await;

        // Should return None for expired token
        assert!(cache.get().await.is_none());
    }

    #[tokio::test]
    async fn should_clear_cache() {
        let cache = TokenCache::new();
        cache.set(OAuth2Token::new("token")).await;
        assert!(cache.get().await.is_some());

        cache.clear().await;
        assert!(cache.get().await.is_none());
    }

    #[tokio::test]
    async fn should_detect_refresh_needed_in_cache() {
        let cache = TokenCache::new();

        // Empty cache needs refresh
        assert!(cache.should_refresh(Duration::from_secs(60)).await);

        // Token expiring soon needs refresh
        let token = OAuth2Token::with_expiry("token", Duration::from_secs(30));
        cache.set(token).await;
        assert!(cache.should_refresh(Duration::from_secs(60)).await);

        // Token with plenty of time doesn't need refresh
        let token = OAuth2Token::with_expiry("token", Duration::from_secs(3600));
        cache.set(token).await;
        assert!(!cache.should_refresh(Duration::from_secs(60)).await);
    }
}
