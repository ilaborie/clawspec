use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use super::domain::{LngLat, ObservationId, PartialObservation, PatchObservation};
use super::repository::ObservationRepository;
use crate::AppState;
use crate::extractors::{ExtractorError, JsonStream, MultipartUpload};

pub(crate) fn observation_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_observations).post(create_observation))
        .route("/import", post(import_observations))
        .route("/upload", post(upload_observations))
        .route(
            "/{observation_id}",
            put(update_observation)
                .patch(patch_observation)
                .delete(delete_observation),
        )
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ListOption {
    pub offset: usize,
    pub limit: usize,
}

impl Default for ListOption {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 10,
        }
    }
}

/// Flattened observation structure for form data that doesn't support nested objects.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FlatObservation {
    pub name: String,
    #[serde(flatten)]
    pub position: LngLat,
    pub color: Option<String>,
    pub notes: Option<String>,
}

impl From<FlatObservation> for PartialObservation {
    fn from(flat: FlatObservation) -> Self {
        Self {
            name: flat.name,
            position: flat.position,
            color: flat.color,
            notes: flat.notes,
        }
    }
}

/// Custom extractor for creating observations that handles multiple formats.
pub struct CreateObservation(pub PartialObservation);

impl<S> FromRequest<S> for CreateObservation
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        match content_type {
            ct if ct.starts_with("application/json") => {
                Json::<PartialObservation>::from_request(req, state)
                    .await
                    .map(|Json(data)| Self(data))
                    .map_err(|rejection| {
                        ExtractorError::json_error_with_location(
                            rejection.body_text(),
                            "observation_data",
                        )
                    })
            }
            ct if ct.starts_with("application/x-www-form-urlencoded") => {
                axum::extract::Form::<FlatObservation>::from_request(req, state)
                    .await
                    .map(|axum::extract::Form(data)| Self(data.into()))
                    .map_err(|rejection| ExtractorError::form_error(rejection.body_text()))
            }
            ct if ct.starts_with("application/xml") || ct.starts_with("text/xml") => {
                let bytes = Bytes::from_request(req, state)
                    .await
                    .map_err(|rejection| ExtractorError::bad_request(rejection.body_text()))?;

                serde_xml_rs::from_reader(bytes.as_ref())
                    .map(Self)
                    .map_err(|err| {
                        ExtractorError::xml_error_with_element(
                            err.to_string(),
                            "PartialObservation",
                        )
                    })
            }
            _ => Err(ExtractorError::unsupported_media_type(content_type)),
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
    CreateObservation(new_observation): CreateObservation,
) -> impl IntoResponse {
    repo.create(new_observation)
        .await
        .map(|result| (StatusCode::CREATED, Json(result)))
}

async fn import_observations(
    State(repo): State<ObservationRepository>,
    JsonStream {
        data,
        bytes_processed,
    }: JsonStream<PartialObservation>,
) -> impl IntoResponse {
    let mut created_ids = Vec::new();
    let mut errors = Vec::new();

    for (index, observation) in data.into_iter().enumerate() {
        match repo.create(observation).await {
            Ok(id) => created_ids.push(id),
            Err(err) => errors.push(format!("Item {index}: {err}")),
        }
    }

    let response = json!({
        "imported": created_ids.len(),
        "error_count": errors.len(),
        "bytes_processed": bytes_processed,
        "created_ids": created_ids,
        "errors": if errors.is_empty() { serde_json::Value::Null } else { json!(errors) }
    });

    (StatusCode::CREATED, Json(response))
}

async fn upload_observations(
    State(repo): State<ObservationRepository>,
    MultipartUpload { fields, files }: MultipartUpload,
) -> impl IntoResponse {
    let mut created_ids = Vec::new();
    let mut errors = Vec::new();

    // Process text fields as individual observations
    for (field_name, field_value) in fields {
        if field_name.starts_with("observation") {
            match serde_json::from_str::<PartialObservation>(&field_value) {
                Ok(observation) => match repo.create(observation).await {
                    Ok(id) => created_ids.push(id),
                    Err(err) => errors.push(format!("Field {field_name}: {err}")),
                },
                Err(err) => errors.push(format!("Field {field_name}: Invalid JSON - {err}")),
            }
        }
    }

    // Process file uploads
    for (field_name, file_data, filename) in files {
        let filename = filename.unwrap_or_else(|| format!("upload_{field_name}"));

        // Try to parse the file as JSON (line-delimited or array)
        match String::from_utf8(file_data) {
            Ok(content) => {
                // First try as NDJSON (newline-delimited JSON)
                let lines: Vec<&str> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .collect();
                let mut file_created = 0;
                let mut file_errors = Vec::new();

                for (line_num, line) in lines.iter().enumerate() {
                    let line_num = line_num + 1;
                    match serde_json::from_str::<PartialObservation>(line) {
                        Ok(observation) => match repo.create(observation).await {
                            Ok(id) => {
                                created_ids.push(id);
                                file_created += 1;
                            }
                            Err(err) => file_errors.push(format!("Line {line_num}: {err}")),
                        },
                        Err(err) => {
                            file_errors.push(format!("Line {line_num}: Invalid JSON - {err}"));
                        }
                    }
                }

                if file_created == 0 && !file_errors.is_empty() {
                    errors.push(format!("File {filename}: No valid observations found"));
                    errors.extend(file_errors);
                }
            }
            Err(_) => {
                errors.push(format!("File {filename}: Not valid UTF-8 text"));
            }
        }
    }

    let response = json!({
        "uploaded": created_ids.len(),
        "error_count": errors.len(),
        "created_ids": created_ids,
        "errors": if errors.is_empty() { serde_json::Value::Null } else { json!(errors) }
    });

    (StatusCode::CREATED, Json(response))
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
