use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::domain::{ObservationId, PartialObservation, PatchObservation};
use super::repository::ObservationRepository;
use crate::AppState;

pub(crate) fn observation_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_observations).post(create_observation))
        .route(
            "/{observation_id}",
            put(update_observation)
                .patch(patch_observation)
                .delete(delete_observation),
        )
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct ListOption {
    offset: usize,
    limit: usize,
}
impl Default for ListOption {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 10,
        }
    }
}

async fn list_observations(
    State(repo): State<ObservationRepository>,
    Query(ListOption { offset, limit }): Query<ListOption>,
) -> impl IntoResponse {
    let limit = limit.min(100);
    repo.list(offset, limit).await.map(|observations| {
        Json(json!({
            "observations": observations
        }))
    })
}

async fn create_observation(
    State(repo): State<ObservationRepository>,
    Json(new_observation): Json<PartialObservation>,
) -> impl IntoResponse {
    repo.create(new_observation)
        .await
        .map(|result| (StatusCode::CREATED, Json(result)))
}

async fn update_observation(
    State(repo): State<ObservationRepository>,
    Path(id): Path<ObservationId>,
    Json(new_observation): Json<PartialObservation>,
) -> impl IntoResponse {
    repo.update(id, new_observation).await
}

async fn patch_observation(
    State(repo): State<ObservationRepository>,
    Path(id): Path<ObservationId>,
    Json(patch): Json<PatchObservation>,
) -> impl IntoResponse {
    repo.patch(id, patch).await.map(Json)
}

async fn delete_observation(
    State(repo): State<ObservationRepository>,
    Path(id): Path<ObservationId>,
) -> impl IntoResponse {
    repo.delete(id).await.map(Json)
}
