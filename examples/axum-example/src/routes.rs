use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::AppState;
use crate::observations::routes::observation_router;

/// The application router
pub fn app_router() -> Router<AppState> {
    Router::new()
        .nest("/api/observations", observation_router())
        .route("/api/health", get(health))
        .route(
            "/openapi",
            get(|| async move { include_str!("../doc/openapi.yml") }),
        )
        .route("/", get(|| async move { "plop" }))
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let uptime = state.uptime();

    Json(json!({
        "status": "OK",
        "uptime": uptime,
    }))
}
