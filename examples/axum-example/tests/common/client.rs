#![allow(clippy::missing_errors_doc, missing_docs)]
use anyhow::Context;
use axum_example::extractors::ExtractorError;
use serde::Deserialize;
use utoipa::ToSchema;

use clawspec_core::{CallHeaders, CallPath, CallQuery, ParamValue};

use axum_example::observations::ListOption;
use axum_example::observations::domain::{
    Observation, ObservationId, PartialObservation, PatchObservation,
};

use super::TestApp;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ListObservations {
    pub observations: Vec<Observation>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TestClientError {
    pub status: u16,
    pub timestamp: String,
    pub error: ExtractorError,
}

impl TestApp {
    pub async fn list_observations(
        &mut self,
        option: Option<ListOption>,
    ) -> anyhow::Result<ListObservations> {
        let ListOption { offset, limit } = option.unwrap_or_default();
        let query = CallQuery::new()
            .add_param("offset", offset)
            .add_param("limit", limit);

        let result = self
            .get("/observations")?
            .with_query(query)
            .await?
            .as_json()
            .await?;
        Ok(result)
    }

    pub async fn create_observation(
        &mut self,
        new_observation: &PartialObservation,
    ) -> anyhow::Result<Observation> {
        let result = self
            .post("/observations")?
            .json(new_observation)?
            .await
            .context("create observation")?
            .as_json()
            .await?;
        Ok(result)
    }

    pub async fn create_observation_redacted(
        &mut self,
        new_observation: &PartialObservation,
    ) -> anyhow::Result<Observation> {
        let result = self
            .post("/observations")?
            .json(new_observation)?
            .await
            .context("create observation")?
            .as_json_redacted::<Observation>()
            .await?
            .redact_replace("/id", "019aaaaa-0000-7000-8000-000000000000")?
            .redact_replace("/created_at", "2024-01-01T00:00:00Z")?
            .finish()
            .await;
        Ok(result.value)
    }

    pub async fn update_observation(
        &mut self,
        id: ObservationId,
        observation: &PartialObservation,
    ) -> anyhow::Result<()> {
        let path = CallPath::from("/observations/{observation_id}")
            .add_param("observation_id", ParamValue::new(id));

        self.put(path)?
            .json(observation)?
            .await
            .context("update observation")?
            .as_empty()
            .await?;
        Ok(())
    }

    pub async fn patch_observation(
        &mut self,
        id: ObservationId,
        patch: &PatchObservation,
    ) -> anyhow::Result<Observation> {
        let call_path = CallPath::from("/observations/{observation_id}")
            .add_param("observation_id", ParamValue::new(id));

        let result = self
            .patch(call_path)?
            .json(patch)?
            .await
            .context("patch observation")?
            .as_json()
            .await?;
        Ok(result)
    }

    pub async fn delete_observation(&mut self, id: ObservationId) -> anyhow::Result<()> {
        let path = CallPath::from("/observations/{observation_id}")
            .add_param("observation_id", ParamValue::new(id));

        self.delete(path)?
            .await
            .context("delete observation")?
            .as_empty()
            .await?;
        Ok(())
    }

    /// Lists observations with custom headers to demonstrate header parameter collection.
    ///
    /// This method showcases different types of headers including authorization,
    /// request tracking, and custom metadata headers.
    pub async fn list_observations_with_headers(
        &mut self,
        option: Option<ListOption>,
        user_id: u64,
        request_id: &str,
    ) -> anyhow::Result<ListObservations> {
        let ListOption { offset, limit } = option.unwrap_or_default();
        let query = CallQuery::new()
            .add_param("offset", offset)
            .add_param("limit", limit);

        // Demonstrate various header types for OpenAPI documentation
        let headers = CallHeaders::new()
            .add_header("Authorization", format!("Bearer user-token-{user_id}"))
            .add_header("X-Request-ID", request_id.to_string())
            .add_header("X-User-ID", user_id)
            .add_header("X-Client-Version", "1.0.0".to_string())
            .add_header("Accept", "application/json".to_string());

        let result = self
            .get("/observations")?
            .with_query(query)
            .with_headers(headers)
            .await?
            .as_json()
            .await?;
        Ok(result)
    }
}
