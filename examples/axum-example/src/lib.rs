#![allow(missing_docs)]
use std::net::SocketAddr;

use anyhow::Context;
use state::AppState;
use tower_http::trace::TraceLayer;
use tracing::info;

mod errors;
mod routes;
mod state;

pub mod observations;

use self::routes::app_router;

/// Launch the server
///
/// # Errors
///
/// fail if the socket cannot be created (e.g. port already used)
/// fail if the server cannot be launch
pub async fn run(addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("opening {addr}"))?;
    info!("Using address {addr}");

    launch(listener).await
}

/// Launch the server
///
/// # Errors
///
/// fail if the server cannot be launch
pub async fn launch(listener: tokio::net::TcpListener) -> anyhow::Result<()> {
    let state = AppState::new().context("create state")?;
    let app = app_router()
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    axum::serve(listener, app.into_make_service())
        .await
        .context("starting server")?;

    Ok(())
}
