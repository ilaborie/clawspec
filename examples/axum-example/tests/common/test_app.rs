#![allow(
    clippy::missing_errors_doc,
    dead_code,
    missing_docs,
    clippy::expect_used
)]
use std::fs;
use std::net::TcpListener;
use std::path::Path;

use anyhow::Context;
use axum::http::StatusCode;
use tracing::info;
use utoipa::openapi::{ContactBuilder, InfoBuilder, ServerBuilder};

use clawspec_core::ApiClient;
use clawspec_core::test_client::{TestClient, TestServer, TestServerConfig};

use axum_example::launch;

#[derive(Debug)]
pub struct AppTestServer;

impl TestServer for AppTestServer {
    async fn launch(&self, listener: TcpListener) {
        listener.set_nonblocking(true).expect("set non-blocking");
        let listener = tokio::net::TcpListener::from_std(listener).expect("valid listener");
        info!(?listener, "launching server");
        launch(listener).await.expect("server launched");
    }

    async fn is_healthy(&self, client: &mut ApiClient) -> Option<bool> {
        let Ok(mut result) = client
            .get("/health")
            .expect("valid path")
            .with_expected_status_code(StatusCode::OK)
            .exchange()
            .await
        else {
            return Some(false);
        };
        let _ = result.as_empty().await;
        Some(true)
    }

    fn config(&self) -> TestServerConfig {
        let client = ApiClient::builder()
            .with_base_path("/api")
            .expect("valid base path")
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
            );
        TestServerConfig {
            api_client: Some(client),
            ..Default::default()
        }
    }
}

#[derive(Debug, derive_more::Deref, derive_more::DerefMut)]
pub struct TestApp {
    #[deref]
    #[deref_mut]
    client: TestClient<AppTestServer>,
}

impl TestApp {
    pub async fn start() -> anyhow::Result<Self> {
        let client = TestClient::start(AppTestServer).await?;
        Ok(Self { client })
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
