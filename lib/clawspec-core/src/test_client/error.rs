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
/// # Examples
///
/// ## Handling Different Error Types
///
/// ```rust,no_run
/// use clawspec_core::test_client::{TestClient, TestServer, TestAppError};
/// use std::net::TcpListener;
///
/// #[derive(Debug)]
/// struct MyServer;
///
/// impl TestServer for MyServer {
///     async fn launch(&self, listener: TcpListener) {
///         listener.set_nonblocking(true).expect("set non-blocking");
///         let tokio_listener = tokio::net::TcpListener::from_std(listener)
///             .expect("valid listener");
///         // Server implementation
///     }
/// }
///
/// #[tokio::test]
/// async fn handle_errors() {
///     match TestClient::start(MyServer).await {
///         Ok(client) => {
///             // Test succeeded
///             println!("Server started successfully");
///         }
///         Err(TestAppError::UnhealthyServer { timeout }) => {
///             println!("Server failed to become healthy within {:?}", timeout);
///         }
///         Err(TestAppError::IoError(io_err)) => {
///             println!("I/O error during server startup: {}", io_err);
///         }
///         Err(TestAppError::ClientError(client_err)) => {
///             println!("ApiClient configuration error: {}", client_err);
///         }
///         Err(err) => {
///             println!("Other error: {}", err);
///         }
///     }
/// }
/// ```
///
/// ## Error Context and Debugging
///
/// ```rust,no_run
/// use clawspec_core::test_client::{TestClient, TestServer, TestAppError};
/// use std::net::TcpListener;
///
/// #[derive(Debug)]
/// struct MyServer;
///
/// impl TestServer for MyServer {
///     async fn launch(&self, listener: TcpListener) {
/// #       listener.set_nonblocking(true).expect("set non-blocking");
/// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener).expect("valid listener");
///         // Server implementation
///     }
/// }
///
/// #[tokio::test]
/// async fn debug_errors() -> Result<(), Box<dyn std::error::Error>> {
///     let client = TestClient::start(MyServer).await
///         .map_err(|err| {
///             eprintln!("Detailed error: {:?}", err);
///             eprintln!("Display error: {}", err);
///             err
///         })?;
///     
///     client.write_openapi("output.yml").await
///         .map_err(|err| {
///             match &err {
///                 TestAppError::YamlError { error } => {
///                     eprintln!("YAML serialization failed: {}", error);
///                 }
///                 TestAppError::IoError(io_err) => {
///                     eprintln!("File write failed: {}", io_err);
///                 }
///                 _ => eprintln!("Unexpected error: {}", err),
///             }
///             err
///         })?;
///     
///     Ok(())
/// }
/// ```
#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum TestAppError {
    /// I/O operation failed.
    ///
    /// This includes file operations (creating directories, writing files)
    /// and network operations (binding to ports, socket operations).
    ///
    /// # Examples
    ///
    /// - Port already in use during server startup
    /// - Permission denied when writing OpenAPI files
    /// - Disk full when creating output directories
    #[display("I/O error: {_0}")]
    IoError(tokio::io::Error),

    /// ApiClient configuration or operation failed.
    ///
    /// This wraps errors from the underlying ApiClient, including
    /// configuration errors, request failures, and response parsing issues.
    ///
    /// # Examples
    ///
    /// - Invalid base path configuration
    /// - HTTP request timeout
    /// - Response parsing errors
    #[display("API client error: {_0}")]
    ClientError(ApiClientError),

    /// JSON serialization or deserialization failed.
    ///
    /// This occurs when generating OpenAPI specifications in JSON format
    /// or when processing JSON request/response data.
    ///
    /// # Examples
    ///
    /// - Invalid Unicode in OpenAPI specification
    /// - Circular references in schema generation
    /// - Malformed JSON data structures
    #[display("JSON error: {_0}")]
    JsonError(serde_json::Error),

    /// YAML serialization failed.
    ///
    /// This occurs specifically when generating OpenAPI specifications
    /// in YAML format. Contains the detailed error message.
    ///
    /// # Examples
    ///
    /// - Invalid characters in YAML output
    /// - Schema too complex for YAML representation
    /// - Memory allocation failure during YAML generation
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
    ///
    /// # Examples
    ///
    /// - Server startup failure due to missing dependencies
    /// - Network configuration issues preventing health checks
    /// - Resource constraints causing slow server initialization
    #[from(ignore)]
    #[display("Server failed to become healthy within {timeout:?}")]
    UnhealthyServer {
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },
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
                assert_eq!(io_err.kind(), tokio::io::ErrorKind::PermissionDenied);
            }
            other => panic!("Expected IoError, got: {other:?}"),
        }
    }
}
