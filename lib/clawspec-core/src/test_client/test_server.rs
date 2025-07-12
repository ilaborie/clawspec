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
/// use clawspec_core::test_client::{TestServer, TestServerConfig};
/// use std::net::TcpListener;
/// use std::time::Duration;
///
/// #[derive(Debug)]
/// struct MyTestServer;
///
/// impl TestServer for MyTestServer {
///     async fn launch(&self, listener: TcpListener) {
///         // Convert to non-blocking for tokio compatibility
///         listener.set_nonblocking(true).expect("set non-blocking");
///         let tokio_listener = tokio::net::TcpListener::from_std(listener)
///             .expect("valid listener");
///         
///         // Start your web server here
///         // For example, with axum:
///         // axum::serve(tokio_listener, app).await.expect("server started");
///     }
///
///     async fn is_healthy(&self, client: &mut clawspec_core::ApiClient) -> Option<bool> {
///         // Check if server is ready by making a health check request
///         match client.get("/health").unwrap().exchange().await {
///             Ok(_) => Some(true),
///             Err(_) => Some(false),
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
///             health_check_timeout: Duration::from_secs(5),
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
///     async fn launch(&self, listener: TcpListener) {
///         listener.set_nonblocking(true).expect("set non-blocking");
///         let tokio_listener = tokio::net::TcpListener::from_std(listener)
///             .expect("valid listener");
///         
///         // Start your web framework here:
///         // For Axum: axum::serve(tokio_listener, self.app.clone()).await.expect("server started");
///         // For Warp: warp::serve(self.app.clone()).run_async(tokio_listener).await;
///         // For actix-web: HttpServer::new(|| self.app.clone()).listen(tokio_listener)?.run().await?;
///     }
/// }
/// ```
///
/// # Health Checking
///
/// The `is_healthy` method allows implementing custom health check logic:
///
/// - Return `Some(true)` if the server is ready to accept requests
/// - Return `Some(false)` if the server is not ready
/// - Return `None` to use the default TCP connection test
///
/// The TestClient will wait for the server to become healthy before returning success.
pub trait TestServer {
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
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_core::test_client::TestServer;
    /// # use std::net::TcpListener;
    /// # struct MyServer;
    /// impl TestServer for MyServer {
    ///     async fn launch(&self, listener: TcpListener) {
    ///         listener.set_nonblocking(true).expect("set non-blocking");
    ///         let tokio_listener = tokio::net::TcpListener::from_std(listener)
    ///             .expect("valid listener");
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
    fn launch(&self, listener: TcpListener) -> impl Future<Output = ()> + Send;

    /// Check if the server is healthy and ready to accept requests.
    ///
    /// This method is called periodically during server startup to determine
    /// when the server is ready. The default implementation returns None,
    /// which triggers a TCP connection test.
    ///
    /// # Arguments
    ///
    /// * `client` - An ApiClient configured for this test server
    ///
    /// # Returns
    ///
    /// * `Some(true)` - Server is healthy and ready
    /// * `Some(false)` - Server is not healthy
    /// * `None` - Use default TCP connection test
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clawspec_core::{test_client::TestServer, ApiClient};
    /// # use std::net::TcpListener;
    /// # struct MyServer;
    /// impl TestServer for MyServer {
    ///     # async fn launch(&self, _listener: TcpListener) {}
    ///     async fn is_healthy(&self, client: &mut ApiClient) -> Option<bool> {
    ///         // Try to make a health check request
    ///         match client.get("/health").unwrap().exchange().await {
    ///             Ok(_) => Some(true),
    ///             Err(_) => Some(false),
    ///         }
    ///     }
    /// }
    /// ```
    fn is_healthy(&self, _client: &mut ApiClient) -> impl Future<Output = Option<bool>> + Send {
        std::future::ready(None)
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
    ///     # async fn launch(&self, _listener: TcpListener) {}
    ///     fn config(&self) -> TestServerConfig {
    ///         TestServerConfig {
    ///             api_client: Some(
    ///                 ApiClient::builder()
    ///                     .with_host("localhost")
    ///                     .with_base_path("/api").unwrap()
    ///             ),
    ///             health_check_timeout: Duration::from_secs(30),
    ///         }
    ///     }
    /// }
    /// ```
    fn config(&self) -> TestServerConfig {
        TestServerConfig::default()
    }
}

/// Default timeout for health check operations.
///
/// This is used when no custom timeout is specified in TestServerConfig.
pub const DEFAULT_HEALTHCHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Configuration for test server behavior and client setup.
///
/// This struct allows customizing how the TestClient interacts with the test server,
/// including the ApiClient configuration and health check timing.
///
/// # Fields
///
/// * `api_client` - Optional pre-configured ApiClient builder for custom client setup
/// * `health_check_timeout` - Maximum time to wait for server to become healthy
///
/// # Examples
///
/// ## Default Configuration
///
/// ```rust
/// use clawspec_core::test_client::TestServerConfig;
///
/// let config = TestServerConfig::default();
/// assert!(config.api_client.is_none());
/// assert_eq!(config.health_check_timeout.as_secs(), 10);
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
///     health_check_timeout: Duration::from_secs(30),
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
///     async fn launch(&self, listener: TcpListener) {
///         // Server implementation
/// #       listener.set_nonblocking(true).expect("set non-blocking");
/// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener).expect("valid listener");
///     }
///
///     fn config(&self) -> TestServerConfig {
///         TestServerConfig {
///             api_client: Some(
///                 clawspec_core::ApiClient::builder()
///                     .with_host("localhost")
///                     .with_base_path("/api").unwrap()
///             ),
///             health_check_timeout: Duration::from_secs(15),
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
    ///     health_check_timeout: std::time::Duration::from_secs(10),
    /// };
    /// ```
    pub api_client: Option<ApiClientBuilder>,

    /// Maximum time to wait for the server to become healthy.
    ///
    /// The TestClient will repeatedly check if the server is healthy (using either
    /// the `is_healthy` method or TCP connection tests) until this timeout expires.
    /// If the server doesn't become healthy within this time, startup will fail.
    ///
    /// # Default
    ///
    /// [`DEFAULT_HEALTHCHECK_TIMEOUT`] (10 seconds)
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::test_client::TestServerConfig;
    /// use std::time::Duration;
    ///
    /// let config = TestServerConfig {
    ///     api_client: None,
    ///     health_check_timeout: Duration::from_secs(30), // 30 second timeout
    /// };
    /// ```
    pub health_check_timeout: Duration,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            api_client: None,
            health_check_timeout: DEFAULT_HEALTHCHECK_TIMEOUT,
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

        fn with_config(config: TestServerConfig) -> Self {
            Self {
                config,
                should_be_healthy: true,
            }
        }

        fn with_health_status(mut self, healthy: bool) -> Self {
            self.should_be_healthy = healthy;
            self
        }
    }

    impl TestServer for MockServer {
        async fn launch(&self, listener: TcpListener) {
            // Convert to non-blocking for tokio compatibility
            listener.set_nonblocking(true).expect("set non-blocking");
            let tokio_listener = TokioTcpListener::from_std(listener).expect("valid listener");

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

        async fn is_healthy(&self, _client: &mut ApiClient) -> Option<bool> {
            Some(self.should_be_healthy)
        }

        fn config(&self) -> TestServerConfig {
            self.config.clone()
        }
    }

    #[test]
    fn test_test_server_config_default() {
        let config = TestServerConfig::default();

        assert!(config.api_client.is_none());
        assert_eq!(config.health_check_timeout, DEFAULT_HEALTHCHECK_TIMEOUT);
    }

    #[test]
    fn test_test_server_config_custom() {
        let timeout = Duration::from_secs(5);
        let client_builder = ApiClient::builder()
            .with_host("test.example.com")
            .with_port(8080);

        let config = TestServerConfig {
            api_client: Some(client_builder),
            health_check_timeout: timeout,
        };

        assert!(config.api_client.is_some());
        assert_eq!(config.health_check_timeout, timeout);
    }

    #[tokio::test]
    async fn test_mock_server_health_check_healthy() {
        let server = MockServer::new().with_health_status(true);
        let mut client = ApiClient::builder().build().expect("valid client");

        let result = server.is_healthy(&mut client).await;
        assert_eq!(result, Some(true));
    }

    #[tokio::test]
    async fn test_mock_server_health_check_unhealthy() {
        let server = MockServer::new().with_health_status(false);
        let mut client = ApiClient::builder().build().expect("valid client");

        let result = server.is_healthy(&mut client).await;
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_default_healthcheck_timeout_constant() {
        assert_eq!(DEFAULT_HEALTHCHECK_TIMEOUT, Duration::from_secs(10));
    }

    #[test]
    fn test_test_server_trait_bounds() {
        // Test that MockServer implements the required traits
        fn assert_test_server<T: TestServer + Send + Sync + 'static>(_: T) {}

        let server = MockServer::new();
        assert_test_server(server);
    }
}
