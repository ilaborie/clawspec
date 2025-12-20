use std::future::Future;
use std::net::TcpListener;
use std::time::Duration;

use crate::{ApiClient, ApiClientBuilder};

/// A trait for implementing test server abstractions for different web frameworks.
///
/// This trait provides a generic interface for launching and managing test servers
/// for various web frameworks (e.g., Axum, Warp, actix-web). It allows the TestClient
/// to work with any server implementation in a framework-agnostic way.
///
/// # Associated Types
///
/// - [`Error`](TestServer::Error): Error type that can occur during server operations
///
/// # Required Methods
///
/// - [`launch`](TestServer::launch): Starts the server with the provided TcpListener
///
/// # Optional Methods
///
/// - [`is_healthy`](TestServer::is_healthy): Checks if the server is ready to accept requests
/// - [`config`](TestServer::config): Provides configuration for the test framework
///
/// # Example
///
/// ```rust
/// use clawspec_core::test_client::{TestServer, TestServerConfig, HealthStatus};
/// use std::net::TcpListener;
/// use std::time::Duration;
///
/// #[derive(Debug)]
/// struct ServerError;
///
/// impl std::fmt::Display for ServerError {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         write!(f, "Server error")
///     }
/// }
///
/// impl std::error::Error for ServerError {}
///
/// #[derive(Debug)]
/// struct MyTestServer;
///
/// impl TestServer for MyTestServer {
///     type Error = ServerError;
///
///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
///         // Convert to non-blocking for tokio compatibility
///         listener.set_nonblocking(true).map_err(|_| ServerError)?;
///         let tokio_listener = tokio::net::TcpListener::from_std(listener)
///             .map_err(|_| ServerError)?;
///         
///         // Start your web server here
///         // For example, with axum:
///         // axum::serve(tokio_listener, app).await.map_err(|_| ServerError)?;
///         Ok(())
///     }
///
///     async fn is_healthy(&self, client: &mut clawspec_core::ApiClient) -> Result<HealthStatus, Self::Error> {
///         // Check if server is ready by making a health check request
///         match client.get("/health").unwrap().await {
///             Ok(_) => Ok(HealthStatus::Healthy),
///             Err(_) => Ok(HealthStatus::Unhealthy),
///         }
///     }
///
///     fn config(&self) -> TestServerConfig {
///         TestServerConfig {
///             api_client: Some(
///                 clawspec_core::ApiClient::builder()
///                     .with_host("localhost")
///                     .with_base_path("/api").unwrap()
///             ),
///             min_backoff_delay: Duration::from_millis(10),
///             max_backoff_delay: Duration::from_secs(1),
///             backoff_jitter: true,
///             max_retry_attempts: 10,
///         }
///     }
/// }
/// ```
///
/// # Framework Integration
///
/// ## Framework Integration Example
///
/// ```rust,no_run
/// use clawspec_core::test_client::{TestServer, TestServerConfig};
/// use std::net::TcpListener;
///
/// #[derive(Debug)]
/// struct WebFrameworkTestServer {
///     // Your web framework's app/router would go here
///     // For example: app: axum::Router, or app: warp::Filter, etc.
/// }
///
/// impl WebFrameworkTestServer {
///     fn new(/* app: YourApp */) -> Self {
///         Self { /* app */ }
///     }
/// }
///
/// impl TestServer for WebFrameworkTestServer {
///     type Error = std::io::Error; // or your custom error type
///
///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
///         listener.set_nonblocking(true)?;
///         let tokio_listener = tokio::net::TcpListener::from_std(listener)?;
///         
///         // Start your web framework here:
///         // For Axum: axum::serve(tokio_listener, self.app.clone()).await?;
///         // For Warp: warp::serve(self.app.clone()).run_async(tokio_listener).await;
///         // For actix-web: HttpServer::new(|| self.app.clone()).listen(tokio_listener)?.run().await?;
///         Ok(())
///     }
/// }
/// ```
///
/// # Health Checking
///
/// The `is_healthy` method allows implementing custom health check logic:
///
/// - Return `Ok(HealthStatus::Healthy)` if the server is ready to accept requests
/// - Return `Ok(HealthStatus::Unhealthy)` if the server is not ready
/// - Return `Ok(HealthStatus::Uncheckable)` to use the default TCP connection test
/// - Return `Err(Self::Error)` if an error occurs during health checking
///
/// The TestClient will wait for the server to become healthy before returning success.
/// Health check status returned by the `is_healthy` method.
///
/// This enum provides more explicit control over health checking behavior
/// compared to the previous `Option<bool>` approach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Server is healthy and ready to accept requests.
    Healthy,
    /// Server is not healthy or not ready yet.
    Unhealthy,
    /// Use the default TCP connection test to determine health.
    /// This is equivalent to the previous `None` return value.
    Uncheckable,
}

pub trait TestServer {
    /// The error type that can be returned by server operations.
    ///
    /// This should implement [`std::error::Error`] to provide proper error handling
    /// and error chain support.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Launch the server using the provided TcpListener.
    ///
    /// This method should start the web server and bind it to the given listener.
    /// The implementation should convert the std::net::TcpListener to a tokio::net::TcpListener
    /// for async compatibility.
    ///
    /// # Arguments
    ///
    /// * `listener` - A TcpListener bound to a random port for testing
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Server launched successfully
    /// * `Err(Self::Error)` - Server failed to launch
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_core::test_client::{TestServer, HealthStatus};
    /// # use std::net::TcpListener;
    /// # #[derive(Debug)] struct ServerError;
    /// # impl std::fmt::Display for ServerError {
    /// #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    /// #         write!(f, "Server error")
    /// #     }
    /// # }
    /// # impl std::error::Error for ServerError {}
    /// # struct MyServer;
    /// impl TestServer for MyServer {
    ///     type Error = ServerError;
    ///
    ///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    ///         listener.set_nonblocking(true).map_err(|_| ServerError)?;
    ///         let tokio_listener = tokio::net::TcpListener::from_std(listener)
    ///             .map_err(|_| ServerError)?;
    ///         
    ///         // Start your server here
    ///         loop {
    ///             if let Ok((stream, _)) = tokio_listener.accept().await {
    ///                 // Handle connection
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    fn launch(&self, listener: TcpListener)
    -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Check if the server is healthy and ready to accept requests.
    ///
    /// This method is called periodically during server startup to determine
    /// when the server is ready. The default implementation returns `HealthStatus::Uncheckable`,
    /// which triggers a TCP connection test.
    ///
    /// # Arguments
    ///
    /// * `client` - An ApiClient configured for this test server
    ///
    /// # Returns
    ///
    /// * `Ok(HealthStatus::Healthy)` - Server is healthy and ready
    /// * `Ok(HealthStatus::Unhealthy)` - Server is not healthy
    /// * `Ok(HealthStatus::Uncheckable)` - Use default TCP connection test
    /// * `Err(Self::Error)` - Error occurred during health check
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_core::{test_client::{TestServer, HealthStatus}, ApiClient};
    /// # use std::net::TcpListener;
    /// # #[derive(Debug)] struct ServerError;
    /// # impl std::fmt::Display for ServerError {
    /// #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    /// #         write!(f, "Server error")
    /// #     }
    /// # }
    /// # impl std::error::Error for ServerError {}
    /// # struct MyServer;
    /// impl TestServer for MyServer {
    ///     # type Error = ServerError;
    ///     # async fn launch(&self, _listener: TcpListener) -> Result<(), Self::Error> { Ok(()) }
    ///     async fn is_healthy(&self, client: &mut ApiClient) -> Result<HealthStatus, Self::Error> {
    ///         // Try to make a health check request
    ///         match client.get("/health").unwrap().await {
    ///             Ok(_) => Ok(HealthStatus::Healthy),
    ///             Err(_) => Ok(HealthStatus::Unhealthy),
    ///         }
    ///     }
    /// }
    /// ```
    fn is_healthy(
        &self,
        _client: &mut ApiClient,
    ) -> impl Future<Output = Result<HealthStatus, Self::Error>> + Send {
        std::future::ready(Ok(HealthStatus::Uncheckable))
    }

    /// Provide configuration for the test framework.
    ///
    /// This method allows customizing the ApiClient and health check behavior
    /// for the specific server implementation.
    ///
    /// # Returns
    ///
    /// A TestServerConfig with custom settings, or default if not overridden.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_core::{test_client::{TestServer, TestServerConfig}, ApiClient};
    /// # use std::{net::TcpListener, time::Duration};
    /// # struct MyServer;
    /// impl TestServer for MyServer {
    ///     # type Error = std::io::Error;
    ///     # async fn launch(&self, _listener: TcpListener) -> Result<(), Self::Error> { Ok(()) }
    ///     fn config(&self) -> TestServerConfig {
    ///         TestServerConfig {
    ///             api_client: Some(
    ///                 ApiClient::builder()
    ///                     .with_host("localhost")
    ///                     .with_base_path("/api").unwrap()
    ///             ),
    ///             min_backoff_delay: Duration::from_millis(10),
    ///             max_backoff_delay: Duration::from_secs(1),
    ///             backoff_jitter: true,
    ///             max_retry_attempts: 10,
    ///         }
    ///     }
    /// }
    /// ```
    fn config(&self) -> TestServerConfig {
        TestServerConfig::default()
    }
}

/// Configuration for test server behavior and client setup.
///
/// This struct allows customizing how the TestClient interacts with the test server,
/// including the ApiClient configuration and exponential backoff timing for health checks.
///
/// # Fields
///
/// * `api_client` - Optional pre-configured ApiClient builder for custom client setup
/// * `min_backoff_delay` - Minimum delay for exponential backoff between health check retries
/// * `max_backoff_delay` - Maximum delay for exponential backoff between health check retries
/// * `backoff_jitter` - Whether to add jitter to exponential backoff delays
/// * `max_retry_attempts` - Maximum number of health check retry attempts
///
/// # Examples
///
/// ## Default Configuration
///
/// ```rust
/// use clawspec_core::test_client::TestServerConfig;
/// use std::time::Duration;
///
/// let config = TestServerConfig::default();
/// assert!(config.api_client.is_none());
/// assert_eq!(config.min_backoff_delay, Duration::from_millis(10));
/// assert_eq!(config.max_backoff_delay, Duration::from_secs(1));
/// assert_eq!(config.backoff_jitter, true);
/// assert_eq!(config.max_retry_attempts, 10);
/// ```
///
/// ## Custom Configuration
///
/// ```rust
/// use clawspec_core::{test_client::TestServerConfig, ApiClient};
/// use std::time::Duration;
///
/// let config = TestServerConfig {
///     api_client: Some(
///         ApiClient::builder()
///             .with_host("test-server.local")
///             .with_port(3000)
///             .with_base_path("/api/v1").unwrap()
///     ),
///     min_backoff_delay: Duration::from_millis(50),
///     max_backoff_delay: Duration::from_secs(5),
///     backoff_jitter: false,
///     max_retry_attempts: 3,
/// };
/// ```
///
/// ## Using with TestServer
///
/// ```rust
/// use clawspec_core::test_client::{TestServer, TestServerConfig};
/// use std::{net::TcpListener, time::Duration};
///
/// #[derive(Debug)]
/// struct MyTestServer;
///
/// impl TestServer for MyTestServer {
///     type Error = std::io::Error;
///
///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
///         // Server implementation
///         listener.set_nonblocking(true)?;
///         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
///         Ok(())
///     }
///
///     fn config(&self) -> TestServerConfig {
///         TestServerConfig {
///             api_client: Some(
///                 clawspec_core::ApiClient::builder()
///                     .with_host("localhost")
///                     .with_base_path("/api").unwrap()
///             ),
///             min_backoff_delay: Duration::from_millis(25),
///             max_backoff_delay: Duration::from_secs(2),
///             backoff_jitter: true,
///             max_retry_attempts: 15,
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TestServerConfig {
    /// Optional pre-configured ApiClient builder.
    ///
    /// If provided, this builder will be used as the base for creating the ApiClient.
    /// If None, a default builder will be used. The TestClient will automatically
    /// configure the port based on the bound server address.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::{test_client::TestServerConfig, ApiClient};
    ///
    /// let config = TestServerConfig {
    ///     api_client: Some(
    ///         ApiClient::builder()
    ///             .with_host("api.example.com")
    ///             .with_base_path("/v1").unwrap()
    ///     ),
    ///     min_backoff_delay: std::time::Duration::from_millis(10),
    ///     max_backoff_delay: std::time::Duration::from_secs(1),
    ///     backoff_jitter: true,
    ///     max_retry_attempts: 10,
    /// };
    /// ```
    pub api_client: Option<ApiClientBuilder>,

    /// Minimum delay for exponential backoff between health check retries.
    ///
    /// This is the initial delay used when the server is unhealthy and needs
    /// to be retried. The delay will increase exponentially up to `max_backoff_delay`.
    ///
    /// # Default
    ///
    /// 10 milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::test_client::TestServerConfig;
    /// use std::time::Duration;
    ///
    /// let config = TestServerConfig {
    ///     min_backoff_delay: Duration::from_millis(50), // Start with 50ms
    ///     ..Default::default()
    /// };
    /// ```
    pub min_backoff_delay: Duration,

    /// Maximum delay for exponential backoff between health check retries.
    ///
    /// This is the upper bound for the exponential backoff delay. Once the
    /// delay reaches this value, it will not increase further.
    ///
    /// # Default
    ///
    /// 1 second
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::test_client::TestServerConfig;
    /// use std::time::Duration;
    ///
    /// let config = TestServerConfig {
    ///     max_backoff_delay: Duration::from_secs(5), // Max 5 seconds
    ///     ..Default::default()
    /// };
    /// ```
    pub max_backoff_delay: Duration,

    /// Whether to add jitter to the exponential backoff delays.
    ///
    /// Jitter adds randomization to retry delays to prevent the "thundering herd"
    /// problem when multiple clients retry simultaneously. This is generally
    /// recommended for production use.
    ///
    /// # Default
    ///
    /// `true` (jitter enabled)
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::test_client::TestServerConfig;
    ///
    /// let config = TestServerConfig {
    ///     backoff_jitter: false, // Disable jitter for predictable timing
    ///     ..Default::default()
    /// };
    /// ```
    pub backoff_jitter: bool,

    /// Maximum number of health check retry attempts.
    ///
    /// This limits the total number of health check attempts before giving up.
    /// The health check will stop retrying once this number of attempts is reached,
    /// preventing infinite loops when a server never becomes healthy.
    ///
    /// # Default
    ///
    /// 10 attempts
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::test_client::TestServerConfig;
    ///
    /// let config = TestServerConfig {
    ///     max_retry_attempts: 5, // Only try 5 times before giving up
    ///     ..Default::default()
    /// };
    /// ```
    pub max_retry_attempts: usize,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            api_client: None,
            min_backoff_delay: Duration::from_millis(10),
            max_backoff_delay: Duration::from_secs(1),
            backoff_jitter: true,
            max_retry_attempts: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiClient;
    use std::net::TcpListener;
    use std::time::Duration;
    use tokio::net::TcpListener as TokioTcpListener;

    /// Mock server implementation for testing
    #[derive(Debug)]
    struct MockServer {
        config: TestServerConfig,
        should_be_healthy: bool,
    }

    impl MockServer {
        fn new() -> Self {
            Self {
                config: TestServerConfig::default(),
                should_be_healthy: true,
            }
        }

        fn with_health_status(mut self, healthy: bool) -> Self {
            self.should_be_healthy = healthy;
            self
        }
    }

    impl TestServer for MockServer {
        type Error = std::io::Error;

        async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
            // Convert to non-blocking for tokio compatibility
            listener.set_nonblocking(true)?;
            let tokio_listener = TokioTcpListener::from_std(listener)?;

            // Simple echo server for testing
            loop {
                if let Ok((mut stream, _)) = tokio_listener.accept().await {
                    tokio::spawn(async move {
                        // Simple HTTP response
                        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
                        if let Err(e) =
                            tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                                .await
                        {
                            eprintln!("Failed to write response: {e}");
                        }
                    });
                }
            }
        }

        async fn is_healthy(&self, _client: &mut ApiClient) -> Result<HealthStatus, Self::Error> {
            Ok(if self.should_be_healthy {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            })
        }

        fn config(&self) -> TestServerConfig {
            self.config.clone()
        }
    }

    #[test]
    fn test_test_server_config_default() {
        let config = TestServerConfig::default();

        assert!(config.api_client.is_none());
        assert_eq!(config.min_backoff_delay, Duration::from_millis(10));
        assert_eq!(config.max_backoff_delay, Duration::from_secs(1));
        assert!(config.backoff_jitter);
        assert_eq!(config.max_retry_attempts, 10);
    }

    #[test]
    fn test_test_server_config_custom() {
        let min_delay = Duration::from_millis(50);
        let max_delay = Duration::from_secs(5);
        let client_builder = ApiClient::builder()
            .with_host("test.example.com")
            .with_port(8080);

        let config = TestServerConfig {
            api_client: Some(client_builder),
            min_backoff_delay: min_delay,
            max_backoff_delay: max_delay,
            backoff_jitter: false,
            max_retry_attempts: 5,
        };

        assert!(config.api_client.is_some());
        assert_eq!(config.min_backoff_delay, min_delay);
        assert_eq!(config.max_backoff_delay, max_delay);
        assert!(!config.backoff_jitter);
        assert_eq!(config.max_retry_attempts, 5);
    }

    #[tokio::test]
    async fn test_mock_server_health_check_healthy() {
        let server = MockServer::new().with_health_status(true);
        let mut client = ApiClient::builder().build().expect("valid client");

        let result = server.is_healthy(&mut client).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_mock_server_health_check_unhealthy() {
        let server = MockServer::new().with_health_status(false);
        let mut client = ApiClient::builder().build().expect("valid client");

        let result = server.is_healthy(&mut client).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_default_backoff_configuration() {
        let config = TestServerConfig::default();

        // Verify the default backoff configuration values
        assert_eq!(config.min_backoff_delay, Duration::from_millis(10));
        assert_eq!(config.max_backoff_delay, Duration::from_secs(1));
        assert!(config.backoff_jitter);
        assert_eq!(config.max_retry_attempts, 10);
    }

    #[test]
    fn test_test_server_trait_bounds() {
        // Test that MockServer implements the required traits
        fn assert_test_server<T: TestServer + Send + Sync + 'static>(_: T) {}

        let server = MockServer::new();
        assert_test_server(server);
    }

    #[test]
    fn test_health_status_healthy_variant() {
        let status = HealthStatus::Healthy;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_status_unhealthy_variant() {
        let status = HealthStatus::Unhealthy;
        assert_eq!(status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_status_uncheckable_variant() {
        let status = HealthStatus::Uncheckable;
        assert_eq!(status, HealthStatus::Uncheckable);
    }

    #[test]
    fn test_health_status_debug() {
        let healthy = HealthStatus::Healthy;
        let unhealthy = HealthStatus::Unhealthy;
        let uncheckable = HealthStatus::Uncheckable;

        assert!(format!("{healthy:?}").contains("Healthy"));
        assert!(format!("{unhealthy:?}").contains("Unhealthy"));
        assert!(format!("{uncheckable:?}").contains("Uncheckable"));
    }

    #[test]
    fn test_health_status_clone() {
        let original = HealthStatus::Healthy;
        let cloned = original;

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_health_status_copy() {
        let status = HealthStatus::Unhealthy;
        let copied = status;

        assert_eq!(status, copied);
    }

    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_eq!(HealthStatus::Unhealthy, HealthStatus::Unhealthy);
        assert_eq!(HealthStatus::Uncheckable, HealthStatus::Uncheckable);

        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Uncheckable);
        assert_ne!(HealthStatus::Unhealthy, HealthStatus::Uncheckable);
    }

    /// A minimal test server that uses the default is_healthy implementation
    #[derive(Debug)]
    struct MinimalServer;

    impl TestServer for MinimalServer {
        type Error = std::io::Error;

        async fn launch(&self, _listener: TcpListener) -> Result<(), Self::Error> {
            Ok(())
        }
        // Uses default is_healthy() which returns Uncheckable
        // Uses default config()
    }

    #[tokio::test]
    async fn test_default_is_healthy_returns_uncheckable() {
        let server = MinimalServer;
        let mut client = ApiClient::builder().build().expect("valid client");

        let result = server.is_healthy(&mut client).await;
        assert!(result.is_ok());
        assert_eq!(result.expect("should be Ok"), HealthStatus::Uncheckable);
    }

    #[test]
    fn test_default_config_returns_defaults() {
        let server = MinimalServer;
        let config = server.config();

        assert!(config.api_client.is_none());
        assert_eq!(config.min_backoff_delay, Duration::from_millis(10));
        assert_eq!(config.max_backoff_delay, Duration::from_secs(1));
        assert!(config.backoff_jitter);
        assert_eq!(config.max_retry_attempts, 10);
    }

    #[test]
    fn test_test_server_config_debug() {
        let config = TestServerConfig::default();
        let debug_str = format!("{config:?}");

        assert!(debug_str.contains("TestServerConfig"));
        assert!(debug_str.contains("min_backoff_delay"));
        assert!(debug_str.contains("max_backoff_delay"));
    }

    #[test]
    fn test_test_server_config_clone() {
        let original = TestServerConfig {
            api_client: None,
            min_backoff_delay: Duration::from_millis(100),
            max_backoff_delay: Duration::from_secs(10),
            backoff_jitter: false,
            max_retry_attempts: 3,
        };
        let cloned = original.clone();

        assert!(cloned.api_client.is_none());
        assert_eq!(cloned.min_backoff_delay, Duration::from_millis(100));
        assert_eq!(cloned.max_backoff_delay, Duration::from_secs(10));
        assert!(!cloned.backoff_jitter);
        assert_eq!(cloned.max_retry_attempts, 3);
    }

    #[test]
    fn test_test_server_config_with_api_client_clone() {
        let original = TestServerConfig {
            api_client: Some(ApiClient::builder().with_host("test.local").with_port(3000)),
            min_backoff_delay: Duration::from_millis(50),
            max_backoff_delay: Duration::from_secs(2),
            backoff_jitter: true,
            max_retry_attempts: 5,
        };
        let cloned = original.clone();

        assert!(cloned.api_client.is_some());
        assert_eq!(cloned.min_backoff_delay, Duration::from_millis(50));
        assert_eq!(cloned.max_backoff_delay, Duration::from_secs(2));
        assert!(cloned.backoff_jitter);
        assert_eq!(cloned.max_retry_attempts, 5);
    }
}
