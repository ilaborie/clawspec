# Clawspec

[![Crates.io](https://img.shields.io/crates/v/clawspec-core.svg)](https://crates.io/crates/clawspec-core) [![Documentation](https://docs.rs/clawspec-core/badge.svg)](https://docs.rs/clawspec-core) [![CI](https://github.com/ilaborie/clawspec/workflows/CI/badge.svg)](https://github.com/ilaborie/clawspec/actions) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A Rust library for generating [OpenAPI] specifications from your HTTP client test code. Write tests, get documentation.

[OpenAPI]: https://www.openapis.org/

## Overview

Clawspec automatically generates OpenAPI documentation by observing HTTP client interactions in your tests. Instead of maintaining separate API documentation, your tests become the source of truth.

### Key Features

- 🧪 **Test-Driven Documentation** - Generate specs from integration tests
- 🔒 **Type Safety** - Leverage Rust's type system for accurate schemas
- 🚀 **Zero Runtime Overhead** - Documentation generation only runs during tests
- 🛠️ **Framework Agnostic** - Works with any async HTTP server
- 📝 **OpenAPI 3.1 Compliant** - Generate standard-compliant specifications

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
clawspec-core = "0.1.1"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

### Basic Example with ApiClient

```rust
use clawspec_core::ApiClient;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::test]
async fn test_user_api() -> Result<(), Box<dyn std::error::Error>> {
    // Create API client
    let mut client = ApiClient::builder()
        .with_host("api.example.com")
        .build()?;

    // Make requests - schemas are automatically captured
    let user: User = client
        .get("/users/123")?
        .await?
        .as_json()
        .await?;

    // Generate OpenAPI specification
    let spec = client.collected_openapi().await;
    let yaml = serde_yaml::to_string(&spec)?;
    std::fs::write("openapi.yml", yaml)?;

    Ok(())
}
```

### Test Server Example with TestClient

For testing complete web applications. See the [axum example](https://github.com/ilaborie/clawspec/tree/main/examples/axum-example) for a full working implementation:

```rust
use clawspec_core::test_client::{TestClient, TestServer};
use std::net::TcpListener;

#[derive(Debug)]
struct MyServer;

impl TestServer for MyServer {
    type Error = std::io::Error;

    async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
        // Start your web server with the provided listener
        // Works with Axum, Warp, Actix-web, etc.
        todo!("Launch your server")
    }
}

#[tokio::test]
async fn test_with_server() -> Result<(), Box<dyn std::error::Error>> {
    // Start test server and client
    let mut client = TestClient::start(MyServer).await?;

    // Test your API
    let response = client
        .post("/users")?
        .json(&User {
            id: 1,
            name: "Alice".into(),
            email: "alice@example.com".into()
        })
        .await?;

    assert_eq!(response.status_code(), 201);

    // Write OpenAPI specification
    client.write_openapi("docs/api.yml").await?;
    Ok(())
}
```

## Core Concepts

### ApiClient

The main HTTP client that captures request/response schemas:

- **Builder pattern** for configuration
- **Automatic schema extraction** from Rust types
- **Flexible parameter handling** (path, query, headers)
- **Status code validation** with ranges and specific codes

### TestClient

A test-focused wrapper providing:

- **Automatic server lifecycle management**
- **Health checking with retries**
- **Integrated OpenAPI generation**
- **Framework-agnostic design**

## Advanced Usage

### Parameter Handling

```rust
use clawspec_core::{ApiClient, CallPath, CallQuery, CallHeaders, ParamValue};

let path = CallPath::from("/users/{id}/posts/{post_id}")
    .add_param("id", ParamValue::new(123))
    .add_param("post_id", ParamValue::new(456));

let query = CallQuery::new()
    .add_param("page", ParamValue::new(1))
    .add_param("limit", ParamValue::new(20));

let headers = CallHeaders::new()
    .add_header("Authorization", "Bearer token")
    .add_header("X-Request-ID", "abc123");

let response = client
    .get(path)?
    .with_query(query)
    .with_headers(headers)
    .exchange()
    .await?;
```

### Status Code Validation

By default, requests expect status codes in the range 200-499. You can customize this:

```rust
use clawspec_core::{ApiClient, expected_status_codes};

// Accept specific codes
client.post("/users")?
    .with_expected_status_codes(expected_status_codes!(201, 202))
    .await?;

// Accept ranges
client.get("/health")?
    .with_expected_status_codes(expected_status_codes!(200-299))
    .await?;

// Complex patterns
client.delete("/users/123")?
    .with_expected_status_codes(expected_status_codes!(204, 404, 400-403))
    .await?;
```

### Schema Registration

```rust
use clawspec_core::{ApiClient, register_schemas};

#[derive(serde::Deserialize, utoipa::ToSchema)]
struct CreateUserRequest {
    name: String,
    email: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
struct ErrorResponse {
    code: String,
    message: String,
}

// Register schemas for better documentation
register_schemas!(client, CreateUserRequest, ErrorResponse);
```

## Integration Examples

### With Axum

For a complete working example, see the [axum example implementation](https://github.com/ilaborie/clawspec/tree/main/examples/axum-example).

```rust
use axum::{Router, routing::get};
use clawspec_core::test_client::{TestClient, TestServer, HealthStatus};

struct AxumTestServer {
    router: Router,
}

impl TestServer for AxumTestServer {
    type Error = std::io::Error;

    async fn launch(&self, listener: TcpListener) -> Result<(), Self::Error> {
        listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(listener)?;

        axum::serve(listener, self.router.clone())
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    async fn is_healthy(&self, client: &mut ApiClient) -> Result<HealthStatus, Self::Error> {
        match client.get("/health").unwrap().await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(_) => Ok(HealthStatus::Unhealthy),
        }
    }
}
```

## Configuration

### TestServerConfig

Configure test server behavior:

```rust
use clawspec_core::test_client::TestServerConfig;
use std::time::Duration;

let config = TestServerConfig {
    min_backoff_delay: Duration::from_millis(10),
    max_backoff_delay: Duration::from_secs(1),
    backoff_jitter: true,
    max_retry_attempts: 10,
    ..Default::default()
};
```

## Error Handling

The library provides comprehensive error types:

- `ApiClientError` - HTTP client errors
- `TestAppError` - Test server errors

All errors implement standard error traits and provide detailed context for debugging.

## Best Practices

1. **Write focused tests** - Each test should document specific endpoints
2. **Use descriptive types** - Well-named structs generate better documentation
3. **Register schemas** - Explicitly register types for complete documentation
4. **Validate status codes** - Be explicit about expected responses
5. **Organize tests** - Group related endpoint tests together

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

**Note**: This project has been developed with assistance from [Claude Code](https://claude.ai/code). All AI-generated code has been carefully reviewed, tested, and validated to ensure quality, security, and adherence to Rust best practices.

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

Built with excellent crates from the Rust ecosystem:

- [utoipa](https://github.com/juhaku/utoipa) - OpenAPI schema generation
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime

