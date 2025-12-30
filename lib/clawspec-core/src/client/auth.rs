use std::fmt;

use http::HeaderValue;
use reqwest::header::{AUTHORIZATION, HeaderName};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(feature = "oauth2")]
use super::oauth2::SharedOAuth2Config;

/// Errors that can occur during authentication processing.
///
/// This enum provides granular error information for authentication-related failures,
/// allowing for more specific error handling and better debugging.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Error, derive_more::Display)]
pub enum AuthenticationError {
    /// Bearer token contains invalid characters for HTTP headers.
    #[display("Bearer token contains invalid characters: {message}")]
    InvalidBearerToken {
        /// Description of the invalid characters or format issue.
        message: String,
    },

    /// Basic authentication username contains invalid characters.
    #[display("Basic auth username contains invalid characters: {message}")]
    InvalidUsername {
        /// Description of the invalid characters or format issue.
        message: String,
    },

    /// Basic authentication password contains invalid characters.
    #[display("Basic auth password contains invalid characters: {message}")]
    InvalidPassword {
        /// Description of the invalid characters or format issue.
        message: String,
    },

    /// API key header name is invalid.
    #[display("Invalid API key header name '{header_name}': {message}")]
    InvalidHeaderName {
        /// The invalid header name that was provided.
        header_name: String,
        /// Description of why the header name is invalid.
        message: String,
    },

    /// API key value contains invalid characters for HTTP headers.
    #[display("API key contains invalid characters: {message}")]
    InvalidApiKey {
        /// Description of the invalid characters or format issue.
        message: String,
    },

    /// Base64 encoding failed during Basic authentication processing.
    #[display("Base64 encoding failed: {message}")]
    EncodingError {
        /// Description of the encoding failure.
        message: String,
    },

    /// OAuth2 token is not yet acquired.
    #[cfg(feature = "oauth2")]
    #[display("OAuth2 token has not been acquired yet")]
    OAuth2TokenNotAcquired,

    /// OAuth2 error.
    #[cfg(feature = "oauth2")]
    #[display("OAuth2 error: {message}")]
    OAuth2Error {
        /// Description of the OAuth2 error.
        message: String,
    },
}

/// Secure wrapper for sensitive string data that automatically zeroes memory on drop.
///
/// This wrapper ensures that sensitive authentication data is securely cleared from memory
/// when it's no longer needed, providing protection against memory inspection attacks.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SecureString(String);

impl SecureString {
    /// Creates a new secure string from the provided value.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns a reference to the inner string value.
    ///
    /// # Security Note
    /// The returned reference should not be stored for extended periods
    /// to minimize exposure time of sensitive data.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the SecureString and returns the inner String.
    ///
    /// # Security Note
    /// The caller becomes responsible for the secure handling of the returned String.
    pub fn into_string(mut self) -> String {
        // Clear the original before returning
        std::mem::take(&mut self.0)
    }

    /// Checks if the secure string equals the given string slice.
    ///
    /// This method is provided for convenient testing and comparison without
    /// exposing the internal string value.
    pub fn equals_str(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl fmt::Debug for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecureString")
            .field("value", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Display for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::mask_sensitive(&self.0))
    }
}

impl From<String> for SecureString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecureString {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

impl Serialize for SecureString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SecureString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::new)
    }
}

impl SecureString {
    /// Masks sensitive data for display/logging purposes.
    fn mask_sensitive(value: &str) -> String {
        if value.len() <= 8 {
            "***".to_string()
        } else {
            format!("{}...{}", &value[..4], &value[value.len() - 4..])
        }
    }
}

/// Authentication configuration for API requests.
///
/// This enum supports various authentication methods commonly used in APIs.
/// Authentication can be configured at the client level and optionally overridden
/// for individual requests.
///
/// # Security Features
///
/// - **Memory Protection**: Sensitive data is automatically cleared from memory when dropped
/// - **Display Masking**: Credentials are never displayed in full for logging safety
/// - **Debug Safety**: Authentication data is redacted in debug output
///
/// # Examples
///
/// ```rust
/// use clawspec_core::Authentication;
///
/// // Bearer token authentication
/// let auth = Authentication::Bearer("my-api-token".into());
///
/// // Basic authentication
/// let auth = Authentication::Basic {
///     username: "user".to_string(),
///     password: "pass".into(),
/// };
///
/// // API key in header
/// let auth = Authentication::ApiKey {
///     header_name: "X-API-Key".to_string(),
///     key: "secret-key".into(),
/// };
/// ```
// Serialize/Deserialize can only be derived when oauth2 feature is not enabled
// because SharedOAuth2Config doesn't implement these traits
#[derive(Clone)]
#[cfg_attr(
    not(feature = "oauth2"),
    derive(Serialize, Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum Authentication {
    /// Bearer token authentication (RFC 6750).
    /// Adds `Authorization: Bearer <token>` header.
    Bearer(SecureString),

    /// HTTP Basic authentication (RFC 7617).
    /// Adds `Authorization: Basic <base64(username:password)>` header.
    Basic {
        /// The username for Basic authentication.
        username: String,
        /// The password for Basic authentication.
        password: SecureString,
    },

    /// API key authentication with custom header.
    /// Adds `<header_name>: <key>` header.
    ApiKey {
        /// The header name for the API key.
        header_name: String,
        /// The API key value.
        key: SecureString,
    },

    /// OAuth2 authentication.
    ///
    /// This variant requires the `oauth2` feature to be enabled.
    /// Tokens are acquired automatically and cached for reuse.
    #[cfg(feature = "oauth2")]
    OAuth2(SharedOAuth2Config),
}

impl Authentication {
    /// Converts the authentication into HTTP headers.
    ///
    /// Returns a tuple of (HeaderName, HeaderValue) that can be added to the request.
    ///
    /// # Errors
    ///
    /// Returns `AuthenticationError` if the authentication data contains invalid characters
    /// or cannot be properly formatted for HTTP headers.
    pub fn to_header(&self) -> Result<(HeaderName, HeaderValue), AuthenticationError> {
        match self {
            Authentication::Bearer(token) => {
                let header_value = format!("Bearer {}", token.as_str());
                let value = HeaderValue::from_str(&header_value).map_err(|e| {
                    AuthenticationError::InvalidBearerToken {
                        message: e.to_string(),
                    }
                })?;
                Ok((AUTHORIZATION, value))
            }

            Authentication::Basic { username, password } => {
                // Validate username doesn't contain invalid characters
                if username.contains(':') {
                    return Err(AuthenticationError::InvalidUsername {
                        message: "Username cannot contain colon (:) character".to_string(),
                    });
                }

                use base64::Engine;
                let credentials_str = format!("{}:{}", username, password.as_str());
                let credentials = base64::engine::general_purpose::STANDARD.encode(credentials_str);

                let header_value = format!("Basic {credentials}");
                let value = HeaderValue::from_str(&header_value).map_err(|e| {
                    AuthenticationError::InvalidPassword {
                        message: e.to_string(),
                    }
                })?;
                Ok((AUTHORIZATION, value))
            }

            Authentication::ApiKey { header_name, key } => {
                let header = HeaderName::from_bytes(header_name.as_bytes()).map_err(|e| {
                    AuthenticationError::InvalidHeaderName {
                        header_name: header_name.clone(),
                        message: e.to_string(),
                    }
                })?;
                let value = HeaderValue::from_str(key.as_str()).map_err(|e| {
                    AuthenticationError::InvalidApiKey {
                        message: e.to_string(),
                    }
                })?;
                Ok((header, value))
            }

            #[cfg(feature = "oauth2")]
            Authentication::OAuth2(_) => {
                // OAuth2 authentication requires async token acquisition
                // This synchronous method cannot be used for OAuth2
                Err(AuthenticationError::OAuth2TokenNotAcquired)
            }
        }
    }
}

impl fmt::Debug for Authentication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bearer(_) => f.debug_tuple("Bearer").field(&"[REDACTED]").finish(),
            Self::Basic { username, .. } => f
                .debug_struct("Basic")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::ApiKey { header_name, .. } => f
                .debug_struct("ApiKey")
                .field("header_name", header_name)
                .field("key", &"[REDACTED]")
                .finish(),
            #[cfg(feature = "oauth2")]
            Self::OAuth2(config) => f.debug_tuple("OAuth2").field(config).finish(),
        }
    }
}

impl fmt::Display for Authentication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bearer(token) => {
                write!(f, "Bearer {token}")
            }
            Self::Basic { username, .. } => write!(f, "Basic (username: {username})"),
            Self::ApiKey { header_name, key } => {
                write!(f, "ApiKey ({header_name}: {key})")
            }
            #[cfg(feature = "oauth2")]
            Self::OAuth2(config) => {
                write!(f, "OAuth2 (client_id: {})", config.0.client_id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_authentication() {
        let auth = Authentication::Bearer("my-secret-token".into());
        let (header_name, header_value) = auth.to_header().unwrap();

        assert_eq!(header_name, AUTHORIZATION);
        assert_eq!(header_value, "Bearer my-secret-token");
    }

    #[test]
    fn test_basic_authentication() {
        let auth = Authentication::Basic {
            username: "user".to_string(),
            password: "pass".into(),
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
            key: "secret-key-123".into(),
        };
        let (header_name, header_value) = auth.to_header().unwrap();

        assert_eq!(header_name, "X-API-Key");
        assert_eq!(header_value, "secret-key-123");
    }

    #[test]
    fn test_display_masks_secrets() {
        let auth = Authentication::Bearer("very-secret-token-12345".into());
        assert_eq!(auth.to_string(), "Bearer very...2345");

        let auth = Authentication::Basic {
            username: "user".to_string(),
            password: "password".into(),
        };
        assert_eq!(auth.to_string(), "Basic (username: user)");

        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key-12345".into(),
        };
        assert_eq!(auth.to_string(), "ApiKey (X-API-Key: secr...2345)");
    }

    #[test]
    fn test_secure_string_mask_short_tokens() {
        assert_eq!(SecureString::mask_sensitive("short"), "***");
        assert_eq!(SecureString::mask_sensitive("12345678"), "***");
        assert_eq!(SecureString::mask_sensitive("123456789"), "1234...6789");
    }

    // Note: Serialization tests are disabled when oauth2 feature is enabled
    // because OAuth2 variant doesn't implement Serialize/Deserialize
    #[cfg(not(feature = "oauth2"))]
    #[test]
    fn test_serialization() {
        let auth = Authentication::Bearer("token".into());
        let json = serde_json::to_string(&auth).expect("serialize bearer");
        assert_eq!(json, r#"{"bearer":"token"}"#);

        let auth = Authentication::Basic {
            username: "user".to_string(),
            password: "pass".into(),
        };
        let json = serde_json::to_string(&auth).expect("serialize basic");
        assert_eq!(json, r#"{"basic":{"username":"user","password":"pass"}}"#);

        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "secret-key".into(),
        };
        let json = serde_json::to_string(&auth).expect("serialize apikey");
        assert_eq!(
            json,
            r#"{"api_key":{"header_name":"X-API-Key","key":"secret-key"}}"#
        );
    }

    #[test]
    fn test_authentication_error_display() {
        let error = AuthenticationError::InvalidBearerToken {
            message: "contains null byte".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Bearer token contains invalid characters: contains null byte"
        );

        let error = AuthenticationError::InvalidUsername {
            message: "contains colon".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Basic auth username contains invalid characters: contains colon"
        );

        let error = AuthenticationError::InvalidHeaderName {
            header_name: "Invalid Header".to_string(),
            message: "contains space".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Invalid API key header name 'Invalid Header': contains space"
        );
    }

    #[test]
    fn test_authentication_errors() {
        // Test bearer token with invalid characters
        let auth = Authentication::Bearer("\0invalid".into());
        let result = auth.to_header();
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthenticationError::InvalidBearerToken { .. } => {}
            _ => panic!("Expected InvalidBearerToken error"),
        }

        // Test basic auth with username containing colon
        let auth = Authentication::Basic {
            username: "user:invalid".to_string(),
            password: "password".into(),
        };
        let result = auth.to_header();
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthenticationError::InvalidUsername { .. } => {}
            _ => panic!("Expected InvalidUsername error"),
        }

        // Test API key with invalid header name
        let auth = Authentication::ApiKey {
            header_name: "Invalid Header".to_string(),
            key: "key".into(),
        };
        let result = auth.to_header();
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthenticationError::InvalidHeaderName { .. } => {}
            _ => panic!("Expected InvalidHeaderName error"),
        }

        // Test API key with invalid key value
        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            key: "\0invalid".into(),
        };
        let result = auth.to_header();
        assert!(result.is_err());
        match result.unwrap_err() {
            AuthenticationError::InvalidApiKey { .. } => {}
            _ => panic!("Expected InvalidApiKey error"),
        }
    }

    #[test]
    fn test_secure_string_debug() {
        let secure = SecureString::new("secret-password".to_string());
        let debug_str = format!("{secure:?}");
        assert_eq!(debug_str, "SecureString { value: \"[REDACTED]\" }");
        assert!(!debug_str.contains("secret-password"));
    }

    #[test]
    fn test_secure_string_display() {
        let secure = SecureString::new("secret-password-12345".to_string());
        let display_str = format!("{secure}");
        assert_eq!(display_str, "secr...2345");
        assert!(!display_str.contains("secret-password"));

        let short_secure = SecureString::new("short".to_string());
        let display_str = format!("{short_secure}");
        assert_eq!(display_str, "***");
    }

    #[test]
    fn test_secure_string_conversions() {
        // Test From<String>
        let secure: SecureString = "test".to_string().into();
        assert_eq!(secure.as_str(), "test");

        // Test From<&str>
        let secure: SecureString = "test".into();
        assert_eq!(secure.as_str(), "test");

        // Test into_string
        let secure = SecureString::new("test".to_string());
        let back_to_string = secure.into_string();
        assert_eq!(back_to_string, "test");
    }
}
