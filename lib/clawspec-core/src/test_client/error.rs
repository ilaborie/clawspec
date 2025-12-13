//! Error types for the test client framework.
//!
//! This module provides comprehensive error handling for test operations,
//! server lifecycle management, and OpenAPI generation.

use std::time::Duration;

use crate::ApiClientError;

/// Error types for test client operations.
///
/// `TestAppError` represents all possible errors that can occur during
/// test client operations, from server startup to OpenAPI generation.
/// It provides detailed error information to help diagnose test failures.
///
/// # Error Categories
///
/// - **I/O Errors**: File operations, network binding issues
/// - **Client Errors**: ApiClient configuration and request errors  
/// - **Serialization Errors**: JSON/YAML parsing and generation errors
/// - **Server Health Errors**: Server startup and health check failures
///
/// All variants implement standard error traits and provide detailed context for debugging.
#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum TestAppError {
    /// I/O operation failed.
    ///
    /// This includes file operations (creating directories, writing files)
    /// and network operations (binding to ports, socket operations).
    #[display("I/O error: {_0}")]
    IoError(std::io::Error),

    /// ApiClient configuration or operation failed.
    ///
    /// This wraps errors from the underlying ApiClient, including
    /// configuration errors, request failures, and response parsing issues.
    #[display("API client error: {_0}")]
    ClientError(ApiClientError),

    /// JSON serialization or deserialization failed.
    ///
    /// This occurs when generating OpenAPI specifications in JSON format
    /// or when processing JSON request/response data.
    #[display("JSON error: {_0}")]
    JsonError(serde_json::Error),

    /// YAML serialization failed.
    ///
    /// This occurs specifically when generating OpenAPI specifications
    /// in YAML format. Contains the detailed error message.
    #[display("YAML serialization error: {error}")]
    YamlError {
        /// Detailed error message from the YAML serializer.
        error: String,
    },

    /// Server failed to become healthy within the timeout period.
    ///
    /// This indicates that the test server either failed to start properly
    /// or did not respond to health checks within the configured timeout.
    ///
    /// # Troubleshooting
    ///
    /// - Check server logs for startup errors
    /// - Verify health check implementation
    /// - Increase timeout if server startup is slow
    /// - Ensure no port conflicts with other services
    #[from(ignore)]
    #[display("Server failed to become healthy within {timeout:?}")]
    UnhealthyServer {
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// Server operation failed.
    ///
    /// This wraps errors from the TestServer implementation, including
    /// launch failures and health check errors.
    #[display("Server error: {_0}")]
    ServerError(Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_test_app_error_display() {
        let yaml_error = TestAppError::YamlError {
            error: "Invalid YAML format".to_string(),
        };
        assert_eq!(
            format!("{yaml_error}"),
            "YAML serialization error: Invalid YAML format"
        );

        let unhealthy_error = TestAppError::UnhealthyServer {
            timeout: Duration::from_secs(5),
        };
        assert_eq!(
            format!("{unhealthy_error}"),
            "Server failed to become healthy within 5s"
        );
    }

    #[test]
    fn test_test_app_error_debug() {
        let yaml_error = TestAppError::YamlError {
            error: "Invalid YAML format".to_string(),
        };
        let debug_str = format!("{yaml_error:?}");
        assert!(debug_str.contains("YamlError"));
        assert!(debug_str.contains("Invalid YAML format"));

        let unhealthy_error = TestAppError::UnhealthyServer {
            timeout: Duration::from_secs(10),
        };
        let debug_str = format!("{unhealthy_error:?}");
        assert!(debug_str.contains("UnhealthyServer"));
        assert!(debug_str.contains("10s"));
    }

    #[test]
    fn test_test_app_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let tokio_error = io_error;
        let test_error: TestAppError = tokio_error.into();

        match test_error {
            TestAppError::IoError(_) => {} // Expected
            other => panic!("Expected IoError, got: {other:?}"),
        }
    }

    #[test]
    fn test_test_app_error_from_json_error() {
        let json_str = r#"{"invalid": json"#;
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let test_error: TestAppError = json_error.into();

        match test_error {
            TestAppError::JsonError(_) => {} // Expected
            other => panic!("Expected JsonError, got: {other:?}"),
        }
    }

    #[test]
    fn test_test_app_error_from_api_client_error() {
        // Create an ApiClient error by building with invalid URL parts
        use crate::ApiClient;

        // Create a malformed URL that will cause ApiClientError
        let build_result = ApiClient::builder()
            .with_scheme(http::uri::Scheme::HTTPS)
            .with_host("invalid\0host") // Invalid host with null byte
            .build();

        match build_result {
            Err(api_error) => {
                let test_error: TestAppError = api_error.into();
                match test_error {
                    TestAppError::ClientError(_) => {} // Expected
                    other => panic!("Expected ClientError, got: {other:?}"),
                }
            }
            Ok(_) => {
                // If the above doesn't work, try a different approach
                // Some invalid base paths might not cause errors at build time
                // Let's just test the conversion directly
                use crate::ApiClientError;
                let api_error = ApiClientError::InvalidBasePath {
                    error: "invalid".to_string(),
                };
                let test_error: TestAppError = api_error.into();
                match test_error {
                    TestAppError::ClientError(_) => {} // Expected
                    other => panic!("Expected ClientError, got: {other:?}"),
                }
            }
        }
    }

    #[test]
    fn test_yaml_error_creation() {
        let error_msg = "YAML serialization failed";
        let yaml_error = TestAppError::YamlError {
            error: error_msg.to_string(),
        };

        match yaml_error {
            TestAppError::YamlError { error } => {
                assert_eq!(error, error_msg);
            }
            other => panic!("Expected YamlError, got: {other:?}"),
        }
    }

    #[test]
    fn test_unhealthy_server_error_creation() {
        let timeout = Duration::from_millis(2500);
        let unhealthy_error = TestAppError::UnhealthyServer { timeout };

        match unhealthy_error {
            TestAppError::UnhealthyServer {
                timeout: actual_timeout,
            } => {
                assert_eq!(actual_timeout, timeout);
            }
            other => panic!("Expected UnhealthyServer, got: {other:?}"),
        }
    }

    #[test]
    fn test_server_error_creation() {
        let server_error = std::io::Error::new(std::io::ErrorKind::AddrInUse, "Port in use");
        let test_error = TestAppError::ServerError(Box::new(server_error));

        match test_error {
            TestAppError::ServerError(_) => {} // Expected
            other => panic!("Expected ServerError, got: {other:?}"),
        }
    }

    #[test]
    fn test_error_trait_bounds() {
        // Verify that TestAppError implements the required traits
        fn assert_error_traits<T>(_: T)
        where
            T: std::error::Error + std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
        {
        }

        let yaml_error = TestAppError::YamlError {
            error: "test".to_string(),
        };
        assert_error_traits(yaml_error);
    }

    #[test]
    fn test_error_source_chain() {
        // Test that errors maintain their source chain
        let original_io_error =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied");
        let tokio_error = original_io_error;
        let test_error: TestAppError = tokio_error.into();

        // Check that we can access the source error
        match test_error {
            TestAppError::IoError(ref io_err) => {
                assert_eq!(io_err.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("Expected IoError, got: {other:?}"),
        }
    }
}
