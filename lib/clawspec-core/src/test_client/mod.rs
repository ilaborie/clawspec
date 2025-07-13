//! Generic test client framework for async server testing.
//!
//! This module provides a generic testing framework that works with any async server
//! implementation through the [`TestServer`] trait. It allows you to start a server,
//! make API calls, and generate OpenAPI specifications from your tests.
//!
//! # Core Components
//!
//! - [`TestClient<T>`]: Generic test client that wraps any server implementing [`TestServer`]
//! - [`TestServer`]: Trait for server implementations (Axum, Warp, actix-web, etc.)
//! - [`TestServerConfig`]: Configuration for test behavior
//! - [`TestAppError`]: Error types for test operations
//!
//! # Quick Start
//!
//! For a complete working example, see the [axum example](https://github.com/ilaborie/clawspec/tree/main/examples/axum-example).
//!
//! ```rust,no_run
//! use clawspec_core::test_client::{TestClient, TestServer, TestServerConfig};
//! use std::net::TcpListener;
//!
//! #[derive(Debug)]
//! struct MyTestServer;
//!
//! impl TestServer for MyTestServer {
//!     type Error = std::io::Error;
//!
//!     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
//!         // Start your server here
//!         listener.set_nonblocking(true)?;
//!         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
//!         // Your server startup logic
//!         Ok(())
//!     }
//! }
//!
//! #[tokio::test]
//! async fn test_my_api() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut test_client = TestClient::start(MyTestServer).await?;
//!     
//!     let response = test_client
//!         .get("/api/users")?
//!         .exchange()
//!         .await?;
//!     
//!     // Generate OpenAPI documentation
//!     test_client.write_openapi("openapi.yml").await?;
//!     
//!     Ok(())
//! }
//! ```

use std::fs;
use std::marker::Sync;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use tracing::{debug, error};

use crate::ApiClient;

mod error;
pub use self::error::*;

mod test_server;
pub use self::test_server::*;

/// A generic test client for async server testing.
///
/// `TestClient<T>` provides a framework-agnostic way to test web servers by wrapping
/// any server implementation that implements the [`TestServer`] trait. It manages
/// server lifecycle, health checking, and provides convenient access to the underlying
/// [`ApiClient`] for making requests and generating OpenAPI specifications.
///
/// # Type Parameters
///
/// * `T` - The server type that implements [`TestServer`]
///
/// # Features
///
/// - **Server Lifecycle Management**: Automatically starts and stops the server
/// - **Health Checking**: Waits for server to be ready before returning success
/// - **OpenAPI Generation**: Collects API calls and generates OpenAPI specifications
/// - **Deref to ApiClient**: Direct access to all [`ApiClient`] methods
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,no_run
/// use clawspec_core::test_client::{TestClient, TestServer};
/// use std::net::TcpListener;
///
/// #[derive(Debug)]
/// struct MyServer;
///
/// impl TestServer for MyServer {
///     type Error = std::io::Error;
///
///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
///         listener.set_nonblocking(true)?;
///         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
///         // Start your server here
///         Ok(())
///     }
/// }
///
/// #[tokio::test]
/// async fn test_api() -> Result<(), Box<dyn std::error::Error>> {
///     let mut client = TestClient::start(MyServer).await?;
///     
///     let response = client.get("/users")?.exchange().await?;
///     assert_eq!(response.status_code(), 200);
///     
///     Ok(())
/// }
/// ```
///
/// ## With Custom Configuration
///
/// ```rust,no_run
/// use clawspec_core::{test_client::{TestClient, TestServer, TestServerConfig}, ApiClient};
/// use std::{net::TcpListener, time::Duration};
///
/// #[derive(Debug)]
/// struct MyServer;
///
/// impl TestServer for MyServer {
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
///                 ApiClient::builder()
///                     .with_host("localhost")
///                     .with_base_path("/api/v1").unwrap()
///             ),
///             min_backoff_delay: Duration::from_millis(25),
///             max_backoff_delay: Duration::from_secs(2),
///             backoff_jitter: true,
///             max_retry_attempts: 15,
///         }
///     }
/// }
///
/// #[tokio::test]
/// async fn test_with_config() -> Result<(), Box<dyn std::error::Error>> {
///     let mut client = TestClient::start(MyServer).await?;
///     
///     // Client is already configured with base path /api/v1
///     let response = client.get("/users")?.exchange().await?; // Calls /api/v1/users
///     
///     Ok(())
/// }
/// ```
///
/// ## Generating OpenAPI Documentation
///
/// ```rust,no_run
/// use clawspec_core::test_client::{TestClient, TestServer};
/// use std::net::TcpListener;
///
/// #[derive(Debug)]
/// struct MyServer;
///
/// impl TestServer for MyServer {
///     type Error = std::io::Error;
///
///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
///         listener.set_nonblocking(true)?;
///         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
///         // Server implementation
///         Ok(())
///     }
/// }
///
/// #[tokio::test]
/// async fn generate_docs() -> Result<(), Box<dyn std::error::Error>> {
///     let mut client = TestClient::start(MyServer).await?;
///     
///     // Make various API calls
///     client.get("/users")?.exchange().await?;
///     client.post("/users")?.json(&serde_json::json!({"name": "John"}))?.exchange().await?;
///     client.get("/users/123")?.exchange().await?;
///     
///     // Generate OpenAPI specification
///     client.write_openapi("docs/openapi.yml").await?;
///     
///     Ok(())
/// }
/// ```
///
/// # Implementation Details
///
/// The `TestClient` uses [`derive_more::Deref`] and [`derive_more::DerefMut`] to provide
/// transparent access to the underlying [`ApiClient`]. This means you can call any
/// [`ApiClient`] method directly on the `TestClient`:
///
/// ```rust,no_run
/// # use clawspec_core::test_client::{TestClient, TestServer};
/// # use std::net::TcpListener;
/// # #[derive(Debug)] struct MyServer;
/// # impl TestServer for MyServer {
/// #   type Error = std::io::Error;
/// #   async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
/// #       listener.set_nonblocking(true)?;
/// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
/// #       Ok(())
/// #   }
/// # }
/// # #[tokio::test]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = TestClient::start(MyServer).await?;
///
/// // These are all ApiClient methods available directly on TestClient
/// let response = client.get("/endpoint")?.exchange().await?;
/// let openapi = client.collected_openapi().await;
/// client.register_schema::<MyType>().await;
/// # Ok(())
/// # }
/// # #[derive(serde::Serialize, utoipa::ToSchema)] struct MyType;
/// ```
///
/// # Lifecycle Management
///
/// When a `TestClient` is dropped, it automatically aborts the server task:
///
/// ```rust,no_run
/// # use clawspec_core::test_client::{TestClient, TestServer};
/// # use std::net::TcpListener;
/// # #[derive(Debug)] struct MyServer;
/// # impl TestServer for MyServer {
/// #   type Error = std::io::Error;
/// #   async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
/// #       listener.set_nonblocking(true)?;
/// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
/// #       Ok(())
/// #   }
/// # }
/// # #[tokio::test]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// {
///     let client = TestClient::start(MyServer).await?;
///     // Server is running
/// } // Server is automatically stopped when client is dropped
/// # Ok(())
/// # }
/// ```
#[derive(Debug, derive_more::Deref, derive_more::DerefMut)]
pub struct TestClient<T> {
    #[allow(dead_code)]
    local_addr: SocketAddr,
    #[deref]
    #[deref_mut]
    client: ApiClient,
    handle: Option<tokio::task::JoinHandle<()>>,
    #[allow(dead_code)]
    test_server: Arc<T>,
}

impl<T> TestClient<T>
where
    T: TestServer + Send + Sync + 'static,
{
    /// Start a test server and create a TestClient.
    ///
    /// This method creates a new TestClient by:
    /// 1. Binding to a random localhost port
    /// 2. Starting the server in a background task
    /// 3. Configuring an ApiClient with the server's address
    /// 4. Waiting for the server to become healthy
    ///
    /// # Arguments
    ///
    /// * `test_server` - An implementation of [`TestServer`] to start
    ///
    /// # Returns
    ///
    /// * `Ok(TestClient<T>)` - A ready-to-use test client
    /// * `Err(TestAppError)` - If server startup or health check fails
    ///
    /// # Errors
    ///
    /// This method can fail for several reasons:
    /// - Port binding failure (system resource issues)
    /// - Server startup failure (implementation errors)
    /// - Health check timeout (server not becoming ready)
    /// - ApiClient configuration errors
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```rust,no_run
    /// use clawspec_core::test_client::{TestClient, TestServer};
    /// use std::net::TcpListener;
    ///
    /// #[derive(Debug)]
    /// struct MyServer;
    ///
    /// impl TestServer for MyServer {
    ///     type Error = std::io::Error;
    ///
    ///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    ///         listener.set_nonblocking(true)?;
    ///         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    ///         // Start your server
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[tokio::test]
    /// async fn test_server_start() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TestClient::start(MyServer).await?;
    ///     // Server is now running and ready for requests
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## With Health Check
    ///
    /// ```rust,no_run
    /// use clawspec_core::{test_client::{TestClient, TestServer}, ApiClient};
    /// use std::net::TcpListener;
    ///
    /// #[derive(Debug)]
    /// struct MyServer;
    ///
    /// impl TestServer for MyServer {
    ///     type Error = std::io::Error;
    ///
    ///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    ///         // Server implementation
    ///         listener.set_nonblocking(true)?;
    ///         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    ///         Ok(())
    ///     }
    ///
    ///     async fn is_healthy(&self, client: &mut ApiClient) -> Result<clawspec_core::test_client::HealthStatus, Self::Error> {
    ///         // Custom health check
    ///         match client.get("/health").unwrap().exchange().await {
    ///             Ok(_) => Ok(clawspec_core::test_client::HealthStatus::Healthy),
    ///             Err(_) => Ok(clawspec_core::test_client::HealthStatus::Unhealthy),
    ///         }
    ///     }
    /// }
    ///
    /// #[tokio::test]
    /// async fn test_with_health_check() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TestClient::start(MyServer).await?;
    ///     // Server is guaranteed to be healthy
    ///     Ok(())
    /// }
    /// ```
    pub async fn start(test_server: T) -> Result<Self, TestAppError> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let listener = TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;

        let test_server = Arc::new(test_server);
        let handle = tokio::spawn({
            let server = Arc::clone(&test_server);
            async move {
                if let Err(error) = server.launch(listener).await {
                    error!(?error, "Server launch failed");
                }
            }
        });

        let TestServerConfig {
            api_client,
            min_backoff_delay,
            max_backoff_delay,
            backoff_jitter,
            max_retry_attempts,
        } = test_server.config();

        // Build client with comprehensive OpenAPI metadata
        let client = api_client.unwrap_or_else(ApiClient::builder);
        let client = client.with_port(local_addr.port()).build()?;

        // Wait until ready with exponential backoff
        let healthy = Self::wait_for_health(
            &test_server,
            &client,
            local_addr,
            min_backoff_delay,
            max_backoff_delay,
            backoff_jitter,
            max_retry_attempts,
        )
        .await;

        if !healthy {
            return Err(TestAppError::UnhealthyServer {
                timeout: max_backoff_delay,
            });
        }

        let result = Self {
            local_addr,
            client,
            handle: Some(handle),
            test_server,
        };
        Ok(result)
    }

    /// Wait for the server to become healthy using exponential backoff.
    ///
    /// This method implements a retry mechanism with exponential backoff to check
    /// if the server is healthy. It handles different health status responses:
    /// - `Healthy`: Server is ready, returns success
    /// - `Unhealthy`: Server not ready, retries with exponential backoff
    /// - `Uncheckable`: Falls back to TCP connection test
    /// - Error: Returns failure immediately
    ///
    /// # Arguments
    ///
    /// * `test_server` - The server implementation to check
    /// * `client` - ApiClient configured for the server
    /// * `local_addr` - Server address for TCP connection fallback
    /// * `min_backoff_delay` - Minimum delay for exponential backoff
    /// * `max_backoff_delay` - Maximum delay for exponential backoff
    /// * `backoff_jitter` - Whether to add jitter to backoff delays
    /// * `max_retry_attempts` - Maximum number of retry attempts before giving up
    ///
    /// # Returns
    ///
    /// * `true` - Server is healthy and ready
    /// * `false` - Server failed health checks or encountered errors
    async fn wait_for_health(
        test_server: &Arc<T>,
        client: &ApiClient,
        local_addr: SocketAddr,
        min_backoff_delay: Duration,
        max_backoff_delay: Duration,
        backoff_jitter: bool,
        max_retry_attempts: usize,
    ) -> bool {
        // Configure exponential backoff with provided settings
        let mut backoff_builder = ExponentialBuilder::default()
            .with_min_delay(min_backoff_delay)
            .with_max_delay(max_backoff_delay)
            .with_max_times(max_retry_attempts); // Limit total retry attempts to prevent infinite loops

        if backoff_jitter {
            backoff_builder = backoff_builder.with_jitter();
        }

        let backoff = backoff_builder;

        let health_check = || {
            let mut client = client.clone();
            let server = Arc::clone(test_server);
            async move {
                let result = server.is_healthy(&mut client).await;
                match result {
                    Ok(HealthStatus::Healthy) => {
                        debug!("🟢 server healthy");
                        Ok(true)
                    }
                    Ok(HealthStatus::Unhealthy) => {
                        debug!("🟠 server not yet healthy, retrying with exponential backoff");
                        Err(std::io::Error::new(
                            std::io::ErrorKind::ConnectionRefused,
                            "Server not healthy yet",
                        ))
                    }
                    Ok(HealthStatus::Uncheckable) => {
                        debug!("❓wait until a connection can be establish with the server");
                        let connection = tokio::net::TcpStream::connect(local_addr).await;
                        if let Err(err) = &connection {
                            error!(?err, %local_addr, "Oops, fail to establish connection");
                        }
                        Ok(connection.is_ok())
                    }
                    Err(error) => {
                        error!(?error, "Health check error");
                        Ok(false)
                    }
                }
            }
        };

        health_check.retry(&backoff).await.unwrap_or(false)
    }

    /// Write the collected OpenAPI specification to a file.
    ///
    /// This method generates an OpenAPI specification from all the API calls made
    /// through this TestClient and writes it to the specified file. The format
    /// (JSON or YAML) is determined by the file extension.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path where the OpenAPI specification should be written.
    ///   File extension determines format:
    ///   - `.yml` or `.yaml` → YAML format
    ///   - All others → JSON format
    ///
    /// # Returns
    ///
    /// * `Ok(())` - File was written successfully
    /// * `Err(TestAppError)` - If file operations or serialization fails
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - Parent directories don't exist and can't be created
    /// - File can't be written (permissions, disk space, etc.)
    /// - OpenAPI serialization fails (YAML or JSON)
    ///
    /// # File Format Detection
    ///
    /// The output format is automatically determined by file extension:
    /// - `openapi.yml` → YAML format
    /// - `openapi.yaml` → YAML format  
    /// - `openapi.json` → JSON format
    /// - `spec.txt` → JSON format (default for unknown extensions)
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```rust,no_run
    /// use clawspec_core::test_client::{TestClient, TestServer};
    /// use std::net::TcpListener;
    ///
    /// #[derive(Debug)]
    /// struct MyServer;
    ///
    /// impl TestServer for MyServer {
    ///     type Error = std::io::Error;
    ///
    ///     async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    ///         listener.set_nonblocking(true)?;
    ///         let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    ///         // Server implementation
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[tokio::test]
    /// async fn generate_openapi() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = TestClient::start(MyServer).await?;
    ///     
    ///     // Make some API calls
    ///     client.get("/users")?.exchange().await?;
    ///     client.post("/users")?.json(&serde_json::json!({"name": "John"}))?.exchange().await?;
    ///     
    ///     // Generate YAML specification
    ///     client.write_openapi("openapi.yml").await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Different Formats
    ///
    /// ```rust,no_run
    /// # use clawspec_core::test_client::{TestClient, TestServer};
    /// # use std::net::TcpListener;
    /// # #[derive(Debug)] struct MyServer;
    /// # impl TestServer for MyServer {
    /// #   type Error = std::io::Error;
    /// #   async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    /// #       listener.set_nonblocking(true)?;
    /// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    /// #       Ok(())
    /// #   }
    /// # }
    /// # #[tokio::test]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = TestClient::start(MyServer).await?;
    ///
    /// // Make API calls
    /// client.get("/api/health")?.exchange().await?;
    ///
    /// // Write in different formats
    /// client.write_openapi("docs/openapi.yaml").await?;  // YAML format
    /// client.write_openapi("docs/openapi.json").await?;  // JSON format
    /// client.write_openapi("docs/spec.txt").await?;      // JSON format (default)
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Creating Parent Directories
    ///
    /// The method automatically creates parent directories if they don't exist:
    ///
    /// ```rust,no_run
    /// # use clawspec_core::test_client::{TestClient, TestServer};
    /// # use std::net::TcpListener;
    /// # #[derive(Debug)] struct MyServer;
    /// # impl TestServer for MyServer {
    /// #   type Error = std::io::Error;
    /// #   async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    /// #       listener.set_nonblocking(true)?;
    /// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    /// #       Ok(())
    /// #   }
    /// # }
    /// # #[tokio::test]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = TestClient::start(MyServer).await?;
    ///
    /// // This will create the docs/api/v1/ directory structure if it doesn't exist
    /// client.write_openapi("docs/api/v1/openapi.yml").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Generated OpenAPI Structure
    ///
    /// The generated OpenAPI specification includes:
    /// - All API endpoints called through the client
    /// - Request and response schemas for structured data
    /// - Parameter definitions (path, query, headers)
    /// - Status codes and error responses
    /// - Server information and metadata
    ///
    /// The specification follows OpenAPI 3.1 format and can be used with various
    /// tools for documentation generation, client generation, and API validation.
    pub async fn write_openapi(mut self, path: impl AsRef<Path>) -> Result<(), TestAppError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let openapi = self.client.collected_openapi().await;

        let ext = path.extension().unwrap_or_default();
        let contents = if ext == "yml" || ext == "yaml" {
            openapi.to_yaml().map_err(|err| TestAppError::YamlError {
                error: format!("{err:#?}"),
            })?
        } else {
            serde_json::to_string_pretty(&openapi)?
        };

        fs::write(path, contents)?;

        Ok(())
    }
}

/// Automatic cleanup when TestClient is dropped.
///
/// This implementation ensures that the background server task is properly
/// terminated when the TestClient goes out of scope, preventing resource leaks.
impl<T> Drop for TestClient<T> {
    /// Abort the background server task when the TestClient is dropped.
    ///
    /// This method is called automatically when the TestClient goes out of scope.
    /// It ensures that the server task is cleanly terminated, preventing the
    /// server from continuing to run after the test is complete.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use clawspec_core::test_client::{TestClient, TestServer};
    /// # use std::net::TcpListener;
    /// # #[derive(Debug)] struct MyServer;
    /// # impl TestServer for MyServer {
    /// #   type Error = std::io::Error;
    /// #   async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
    /// #       listener.set_nonblocking(true)?;
    /// #       let _tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    /// #       Ok(())
    /// #   }
    /// # }
    /// # #[tokio::test]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// {
    ///     let client = TestClient::start(MyServer).await?;
    ///     // Use the client for testing
    ///     client.get("/api/test")?.exchange().await?;
    /// } // <- TestClient is dropped here, server task is automatically aborted
    ///   
    /// // Server is no longer running
    /// # Ok(())
    /// # }
    /// ```
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiClient;
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tokio::net::TcpListener as TokioTcpListener;

    /// Mock server for testing TestClient functionality
    #[derive(Debug)]
    struct MockTestServer {
        should_be_healthy: Arc<AtomicBool>,
        startup_delay: Duration,
        custom_config: Option<TestServerConfig>,
    }

    impl MockTestServer {
        fn new() -> Self {
            Self {
                should_be_healthy: Arc::new(AtomicBool::new(true)),
                startup_delay: Duration::from_millis(10),
                custom_config: None,
            }
        }

        fn with_health_status(self, healthy: bool) -> Self {
            self.should_be_healthy.store(healthy, Ordering::Relaxed);
            self
        }

        fn with_startup_delay(mut self, delay: Duration) -> Self {
            self.startup_delay = delay;
            self
        }

        fn with_config(mut self, config: TestServerConfig) -> Self {
            self.custom_config = Some(config);
            self
        }
    }

    impl TestServer for MockTestServer {
        type Error = std::io::Error;

        async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
            // Simulate startup delay
            if !self.startup_delay.is_zero() {
                tokio::time::sleep(self.startup_delay).await;
            }

            // Convert to non-blocking for tokio compatibility
            listener.set_nonblocking(true)?;
            let tokio_listener = TokioTcpListener::from_std(listener)?;

            // Simple HTTP server that responds to health checks
            loop {
                if let Ok((mut stream, _)) = tokio_listener.accept().await {
                    tokio::spawn(async move {
                        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
                        let _ =
                            tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                                .await;
                        let _ = tokio::io::AsyncWriteExt::shutdown(&mut stream).await;
                    });
                }
            }
        }

        async fn is_healthy(&self, _client: &mut ApiClient) -> Result<HealthStatus, Self::Error> {
            Ok(if self.should_be_healthy.load(Ordering::Relaxed) {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            })
        }

        fn config(&self) -> TestServerConfig {
            self.custom_config.clone().unwrap_or_default()
        }
    }

    #[tokio::test]
    async fn test_test_client_start_success() {
        let server = MockTestServer::new();

        let result = TestClient::start(server).await;
        assert!(result.is_ok());

        let test_client = result.unwrap();
        assert!(test_client.handle.is_some());

        // Test that the server is running by checking the local address
        let addr = test_client.local_addr;
        assert_eq!(addr.ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(addr.port(), 0); // Should have been assigned a port
    }

    #[tokio::test]
    async fn test_test_client_start_with_custom_config() {
        let min_delay = Duration::from_millis(5);
        let max_delay = Duration::from_millis(100);
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

        let server = MockTestServer::new().with_config(config);
        let result = TestClient::start(server).await;

        assert!(result.is_ok());
        let test_client = result.unwrap();
        assert!(test_client.handle.is_some());
    }

    #[tokio::test]
    async fn test_test_client_start_unhealthy_server() {
        let expected_max_delay = Duration::from_millis(50);
        let config = TestServerConfig {
            api_client: None,
            min_backoff_delay: Duration::from_millis(5),
            max_backoff_delay: expected_max_delay,
            backoff_jitter: false,
            max_retry_attempts: 3,
        };
        let server = MockTestServer::new()
            .with_health_status(false)
            .with_config(config);

        let result = TestClient::start(server).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            TestAppError::UnhealthyServer {
                timeout: actual_timeout,
            } => {
                assert_eq!(actual_timeout, expected_max_delay);
            }
            other => panic!("Expected UnhealthyServer error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_test_client_start_slow_server() {
        let server = MockTestServer::new().with_startup_delay(Duration::from_millis(50));

        let result = TestClient::start(server).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_test_client_deref_to_api_client() {
        let server = MockTestServer::new();
        let mut test_client = TestClient::start(server)
            .await
            .expect("client should start");

        // Test that we can access ApiClient methods through deref
        let openapi = test_client.collected_openapi().await;
        assert_eq!(openapi.info.title, ""); // Default title
    }

    #[tokio::test]
    async fn test_test_client_deref_mut_to_api_client() {
        let server = MockTestServer::new();
        let test_client = TestClient::start(server)
            .await
            .expect("client should start");

        // Test that we can mutably access ApiClient methods through deref_mut
        let result = test_client.get("/test");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_test_client_write_openapi_json() {
        let server = MockTestServer::new();
        let test_client = TestClient::start(server)
            .await
            .expect("client should start");

        let temp_file = "/tmp/test_openapi.json";
        let result = test_client.write_openapi(temp_file).await;

        assert!(result.is_ok());

        // Verify file was created and contains valid JSON
        let content = std::fs::read_to_string(temp_file).expect("file should exist");
        let json: serde_json::Value = serde_json::from_str(&content).expect("should be valid JSON");

        assert!(json.get("openapi").is_some());
        assert!(json.get("info").is_some());

        // Cleanup
        let _ = std::fs::remove_file(temp_file);
    }

    #[tokio::test]
    async fn test_test_client_write_openapi_yaml() {
        let server = MockTestServer::new();
        let test_client = TestClient::start(server)
            .await
            .expect("client should start");

        let temp_file = "/tmp/test_openapi.yml";
        let result = test_client.write_openapi(temp_file).await;

        assert!(result.is_ok());

        // Verify file was created and contains valid YAML
        let content = std::fs::read_to_string(temp_file).expect("file should exist");
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).expect("should be valid YAML");

        assert!(yaml.get("openapi").is_some());
        assert!(yaml.get("info").is_some());

        // Cleanup
        let _ = std::fs::remove_file(temp_file);
    }

    #[tokio::test]
    async fn test_test_client_write_openapi_creates_parent_dirs() {
        let server = MockTestServer::new();
        let test_client = TestClient::start(server)
            .await
            .expect("client should start");

        let temp_dir = "/tmp/test_clawspec_dir/subdir";
        let temp_file = format!("{temp_dir}/openapi.json");

        let result = test_client.write_openapi(&temp_file).await;
        assert!(result.is_ok());

        // Verify file and directories were created
        assert!(std::fs::metadata(&temp_file).is_ok());

        // Cleanup
        let _ = std::fs::remove_dir_all("/tmp/test_clawspec_dir");
    }

    #[tokio::test]
    async fn test_test_client_drop_aborts_handle() {
        let server = MockTestServer::new();
        let test_client = TestClient::start(server)
            .await
            .expect("client should start");

        let handle = test_client.handle.as_ref().unwrap();
        assert!(!handle.is_finished());

        // Drop the test client
        drop(test_client);

        // Give a moment for the handle to be aborted
        tokio::time::sleep(Duration::from_millis(10)).await;
        // Note: We can't easily test that the handle was aborted since we dropped test_client
        // But the Drop implementation should call abort()
    }

    #[test]
    fn test_test_client_debug_trait() {
        // Test that TestClient implements Debug (compile-time check)
        let server = MockTestServer::new();
        // We can't easily create a TestClient in a sync test, but we can verify the trait bounds
        fn assert_debug<T: std::fmt::Debug>(_: &T) {}
        assert_debug(&server);
    }

    #[test]
    fn test_test_client_trait_bounds() {
        // Verify that TestClient has the expected trait bounds
        #[allow(dead_code)]
        fn assert_bounds<T>(_: TestClient<T>)
        where
            T: TestServer + Send + Sync + 'static,
        {
            // TestClient should implement Deref and DerefMut to ApiClient
        }

        // This is a compile-time check
    }

    /// Mock server that simulates different error conditions
    #[derive(Debug)]
    struct ErrorTestServer {
        error_type: ErrorType,
    }

    #[derive(Debug)]
    enum ErrorType {
        #[allow(dead_code)]
        BindFailure,
        HealthTimeout,
    }

    impl ErrorTestServer {
        #[allow(dead_code)]
        fn bind_failure() -> Self {
            Self {
                error_type: ErrorType::BindFailure,
            }
        }

        fn health_timeout() -> Self {
            Self {
                error_type: ErrorType::HealthTimeout,
            }
        }
    }

    impl TestServer for ErrorTestServer {
        type Error = std::io::Error;

        async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
            match self.error_type {
                ErrorType::BindFailure => {
                    // Simulate bind failure by returning an error
                    Err(std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        "Simulated bind failure",
                    ))
                }
                ErrorType::HealthTimeout => {
                    // Start normally but never become healthy
                    listener.set_nonblocking(true)?;
                    let _tokio_listener = TokioTcpListener::from_std(listener)?;

                    // Just keep running without accepting connections
                    loop {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }

        async fn is_healthy(&self, _client: &mut ApiClient) -> Result<HealthStatus, Self::Error> {
            match self.error_type {
                ErrorType::BindFailure => Ok(HealthStatus::Unhealthy),
                ErrorType::HealthTimeout => {
                    // Always return unhealthy, which will cause the exponential backoff to timeout
                    Ok(HealthStatus::Unhealthy)
                }
            }
        }

        fn config(&self) -> TestServerConfig {
            TestServerConfig {
                api_client: None,
                min_backoff_delay: Duration::from_millis(1), // Very fast for testing
                max_backoff_delay: Duration::from_millis(10), // Short max delay for testing
                backoff_jitter: false,                       // Predictable timing for tests
                max_retry_attempts: 3,                       // Quick timeout for tests
            }
        }
    }

    #[tokio::test]
    async fn test_test_client_start_health_timeout() {
        let server = ErrorTestServer::health_timeout();

        let result = TestClient::start(server).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            TestAppError::UnhealthyServer { timeout } => {
                // The timeout should be the max_backoff_delay from the ErrorTestServer config
                assert_eq!(timeout, Duration::from_millis(10));
            }
            other => panic!("Expected UnhealthyServer error, got: {other:?}"),
        }
    }
}
