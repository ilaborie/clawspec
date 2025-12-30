//! OAuth2-specific error types.

use std::fmt;

/// Errors that can occur during OAuth2 authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuth2Error {
    /// Token endpoint URL is invalid.
    InvalidTokenEndpoint {
        /// The invalid URL that was provided.
        url: String,
        /// Description of why the URL is invalid.
        reason: String,
    },

    /// Token acquisition failed.
    TokenAcquisitionFailed {
        /// Description of the failure.
        reason: String,
    },

    /// Token has expired and no refresh token is available.
    TokenExpired,

    /// Token refresh failed.
    TokenRefreshFailed {
        /// Description of the failure.
        reason: String,
    },

    /// Invalid OAuth2 response from the token endpoint.
    InvalidTokenResponse {
        /// Description of what was invalid.
        reason: String,
    },

    /// Network error during token request.
    NetworkError {
        /// Description of the network error.
        reason: String,
    },

    /// Configuration error.
    ConfigurationError {
        /// Description of the configuration issue.
        reason: String,
    },
}

impl std::error::Error for OAuth2Error {}

impl fmt::Display for OAuth2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTokenEndpoint { url, reason } => {
                write!(f, "Invalid token endpoint URL '{url}': {reason}")
            }
            Self::TokenAcquisitionFailed { reason } => {
                write!(f, "Token acquisition failed: {reason}")
            }
            Self::TokenExpired => {
                write!(
                    f,
                    "OAuth2 token has expired and no refresh token is available"
                )
            }
            Self::TokenRefreshFailed { reason } => {
                write!(f, "Token refresh failed: {reason}")
            }
            Self::InvalidTokenResponse { reason } => {
                write!(f, "Invalid OAuth2 token response: {reason}")
            }
            Self::NetworkError { reason } => {
                write!(f, "Network error during OAuth2 request: {reason}")
            }
            Self::ConfigurationError { reason } => {
                write!(f, "OAuth2 configuration error: {reason}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_display_invalid_token_endpoint() {
        let error = OAuth2Error::InvalidTokenEndpoint {
            url: "not-a-url".to_string(),
            reason: "missing scheme".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Invalid token endpoint URL 'not-a-url': missing scheme"
        );
    }

    #[test]
    fn should_display_token_acquisition_failed() {
        let error = OAuth2Error::TokenAcquisitionFailed {
            reason: "invalid_client".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Token acquisition failed: invalid_client"
        );
    }

    #[test]
    fn should_display_token_expired() {
        let error = OAuth2Error::TokenExpired;
        assert_eq!(
            error.to_string(),
            "OAuth2 token has expired and no refresh token is available"
        );
    }

    #[test]
    fn should_display_configuration_error() {
        let error = OAuth2Error::ConfigurationError {
            reason: "missing client_secret".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "OAuth2 configuration error: missing client_secret"
        );
    }
}
