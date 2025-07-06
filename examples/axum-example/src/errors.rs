use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::observations::domain::ObservationId;

#[derive(Debug, derive_more::Error, derive_more::From, derive_more::Display)]
pub(super) enum RepositoryError {
    DbError(serde_json::Error),

    #[display("No observation with id {id}")]
    ObservationNotFound {
        id: ObservationId,
    },
}

impl IntoResponse for RepositoryError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::ObservationNotFound { .. } => StatusCode::NOT_FOUND,
            Self::DbError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = self.to_string();

        (status, message).into_response()
    }
}
