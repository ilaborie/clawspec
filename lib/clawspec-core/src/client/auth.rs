use std::fmt;

use http::HeaderValue;
use reqwest::header::{AUTHORIZATION, HeaderName};
use serde::{Deserialize, Serialize};

/// Authentication configuration for API requests.
///
/// This enum supports various authentication methods commonly used in APIs.
/// Authentication can be configured at the client level and optionally overridden
/// for individual requests.
///
/// # Examples
///
/// ```rust
/// use clawspec_core::Authentication;
///
/// // Bearer token authentication
/// let auth = Authentication::Bearer("my-api-token".to_string());
///
/// // Basic authentication
/// let auth = Authentication::Basic {
///     username: "user".to_string(),
///     password: "pass".to_string(),
/// };
///
/// // API key in header
/// let auth = Authentication::ApiKey {
///     header_name: "X-API-Key".to_string(),
///     key: "secret-key".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authentication {
    /// Bearer token authentication (RFC 6750)
    /// Adds `Authorization: Bearer <token>` header
    Bearer(String),

    /// HTTP Basic authentication (RFC 7617)
    /// Adds `Authorization: Basic <base64(username:password)>` header
    Basic { username: String, password: String },

    /// API key authentication with custom header
    /// Adds `<header_name>: <key>` header
    ApiKey { header_name: String, key: String },
}

impl Authentication {
    /// Converts the authentication into HTTP headers.
    ///
    /// Returns a tuple of (HeaderName, HeaderValue) that can be added to the request.
    pub fn to_header(&self) -> Result<(HeaderName, HeaderValue), crate::ApiClientError> {
        match self {
            Authentication::Bearer(token) => {
                let value = HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(crate::ApiClientError::InvalidHeaderValue)?;
                Ok((AUTHORIZATION, value))
            }

            Authentication::Basic { username, password } => {
                use base64::Engine;
                let credentials = base64::engine::general_purpose::STANDARD
                    .encode(format!("{username}:{password}"));
                let value = HeaderValue::from_str(&format!("Basic {credentials}"))
                    .map_err(crate::ApiClientError::InvalidHeaderValue)?;
                Ok((AUTHORIZATION, value))
            }

            Authentication::ApiKey { header_name, key } => {
                let header = HeaderName::from_bytes(header_name.as_bytes())
                    .map_err(crate::ApiClientError::InvalidHeaderName)?;
                let value = HeaderValue::from_str(key)
                    .map_err(crate::ApiClientError::InvalidHeaderValue)?;
                Ok((header, value))
            }
        }
    }

    /// Masks sensitive authentication data for display/logging.
    fn mask_token(token: &str) -> String {
        if token.len() <= 8 {
            "***".to_string()
        } else {
            format!("{}...{}", &token[..4], &token[token.len() - 4..])
        }
    }
}

impl fmt::Display for Authentication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Authentication::Bearer(token) => {
                let masked = Self::mask_token(token);
                write!(f, "Bearer {masked}")
            }
            Authentication::Basic { username, .. } => write!(f, "Basic (username: {username})"),
            Authentication::ApiKey { header_name, key } => {
                let masked = Self::mask_token(key);
                write!(f, "ApiKey ({header_name}: {masked})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_authentication() {
        let auth = Authentication::Bearer("my-secret-token".to_string());
        let (header_name, header_value) = auth.to_header().unwrap();

        assert_eq!(header_name, AUTHORIZATION);
        assert_eq!(header_value, "Bearer my-secret-token");
    }

    #[test]
    fn test_basic_authentication() {
        let auth = Authentication::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let (header_name, header_value) = auth.to_header().unwrap();

        assert_eq!(header_name, AUTHORIZATION);
        // "user:pass" base64 encoded is "dXNlcjpwYXNz"
        assert_eq!(header_value, "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn test_api_key_authentication() {
        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key-123".to_string(),
        };
        let (header_name, header_value) = auth.to_header().unwrap();

        assert_eq!(header_name, "X-API-Key");
        assert_eq!(header_value, "secret-key-123");
    }

    #[test]
    fn test_display_masks_secrets() {
        let auth = Authentication::Bearer("very-secret-token-12345".to_string());
        assert_eq!(auth.to_string(), "Bearer very...2345");

        let auth = Authentication::Basic {
            username: "user".to_string(),
            password: "password".to_string(),
        };
        assert_eq!(auth.to_string(), "Basic (username: user)");

        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key-12345".to_string(),
        };
        assert_eq!(auth.to_string(), "ApiKey (X-API-Key: secr...2345)");
    }

    #[test]
    fn test_mask_short_tokens() {
        assert_eq!(Authentication::mask_token("short"), "***");
        assert_eq!(Authentication::mask_token("12345678"), "***");
        assert_eq!(Authentication::mask_token("123456789"), "1234...6789");
    }

    #[test]
    fn test_serialization() {
        let auth = Authentication::Bearer("token".to_string());
        let json = serde_json::to_string(&auth).unwrap();
        assert_eq!(json, r#"{"bearer":"token"}"#);

        let auth = Authentication::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert_eq!(json, r#"{"basic":{"username":"user","password":"pass"}}"#);

        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert_eq!(
            json,
            r#"{"api_key":{"header_name":"X-API-Key","key":"secret-key"}}"#
        );
    }
}
