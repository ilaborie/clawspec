#![allow(clippy::missing_errors_doc)]
use anyhow::Context;
use serde::Deserialize;
use utoipa::ToSchema;

use clawspec_utoipa::CallQuery;

use axum_example::observations::ListOption;
use axum_example::observations::domain::{Observation, PartialObservation};

use super::TestApp;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ListObservations {
    pub observations: Vec<Observation>,
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
            .query(query)
            .exchange()
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
            .exchange()
            .await
            .context("create observation")?
            .as_json()
            .await?;
        Ok(result)
    }
}
