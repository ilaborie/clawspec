#![allow(clippy::missing_errors_doc, dead_code, missing_docs)]
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use anyhow::Context;
use clawspec_utoipa::ApiClient;
use tracing::error;

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

        // Build client
        let client = ApiClient::builder()
            .port(local_addr.port())
            .base_path("/api")?
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
