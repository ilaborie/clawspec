use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::observations::domain::ObservationId;

#[derive(Debug, derive_more::Error, derive_more::From, derive_more::Display)]
pub(super) enum RepositoryError {
    DbError(serde_json::Error),

    #[display("No observation with id {id}")]
    ObservationNotFound {
        id: ObservationId,
    },
}

/// API error response returned for all error cases
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorResponse {
    /// HTTP status code
    pub status: u16,
    /// Human-readable error message
    pub message: String,
    /// Optional structured error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for RepositoryError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::ObservationNotFound { .. } => StatusCode::NOT_FOUND,
            Self::DbError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = self.to_string();

        let error_response = ApiErrorResponse {
            status: status.as_u16(),
            message,
            details: None,
        };

        (status, Json(error_response)).into_response()
    }
}
