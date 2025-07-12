#![allow(clippy::missing_errors_doc, dead_code, missing_docs)]
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use anyhow::Context;
use clawspec_core::ApiClient;
use tracing::error;
use utoipa::openapi::{ContactBuilder, InfoBuilder, ServerBuilder};

use axum_example::launch;

#[derive(Debug, derive_more::Deref, derive_more::DerefMut)]
pub struct TestApp {
    local_addr: SocketAddr,
    #[deref]
    #[deref_mut]
    client: ApiClient,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl TestApp {
    // TODO params - https://github.com/ilaborie/clawspec/issues/14
    // launcher FnOnce(TcpListener) -> Future<Output=()> + Send + 'statuc
    // check healthy: Fn(port) -> bool
    // health check internval: Duration
    pub async fn start() -> anyhow::Result<Self> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("cannot open {addr}"))?;
        let local_addr = listener.local_addr().context("listener address")?;

        let handle = tokio::spawn(async move {
            if let Err(error) = launch(listener).await {
                error!(?error, "server fail to launch");
            }
        });

        let _connection = tokio::net::TcpStream::connect(local_addr)
            .await
            .with_context(|| format!("cannot connect to server {local_addr}"))?;

        // TODO Wait until ready - https://github.com/ilaborie/clawspec/issues/15
        // let health_check_timeout = Duration::from_secs(10);

        // Build client with comprehensive OpenAPI metadata
        let client = ApiClient::builder()
            .with_port(local_addr.port())
            .with_base_path("/api")?
            .with_info(
                InfoBuilder::new()
                    .title("Bird Observation API")
                    .version("1.0.0")
                    .description(Some(
                        "A comprehensive API for managing bird observations with support for \
                        multiple content types, file uploads, and bulk operations. \
                        This API demonstrates RESTful design patterns and provides \
                        comprehensive CRUD operations for bird observation data.",
                    ))
                    .contact(Some(
                        ContactBuilder::new()
                            .name(Some("Bird Observation Team"))
                            .email(Some("api-support@birdwatch.example.com"))
                            .url(Some("https://birdwatch.example.com/support"))
                            .build(),
                    ))
                    .build(),
            )
            .add_server(
                ServerBuilder::new()
                    .url("http://localhost:8080/api")
                    .description(Some("Development server for testing"))
                    .build(),
            )
            .add_server(
                ServerBuilder::new()
                    .url("https://api.birdwatch.example.com/api")
                    .description(Some("Production server"))
                    .build(),
            )
            .add_server(
                ServerBuilder::new()
                    .url("https://staging.birdwatch.example.com/api")
                    .description(Some("Staging server for pre-production testing"))
                    .build(),
            )
            .build()
            .context("failed to build API client")?;

        let result = Self {
            local_addr,
            client,
            handle: Some(handle),
        };
        Ok(result)
    }

    pub async fn write_openapi(mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("create missing parents dir.")?;
        }

        let openapi = self.client.collected_openapi().await;

        let ext = path.extension().unwrap_or_default();
        let contents = if ext == "yml" || ext == "yaml" {
            openapi.to_yaml().context("create YAML")?
        } else {
            openapi.to_pretty_json().context("create JSON")?
        };

        fs::write(path, contents).with_context(|| format!("writing to {}", path.display()))?;

        Ok(())
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
