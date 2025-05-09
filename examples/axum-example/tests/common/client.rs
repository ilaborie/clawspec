#![allow(clippy::missing_errors_doc)]
use anyhow::Context;
use serde::Deserialize;
use utoipa::ToSchema;

use axum_example::observations::domain::{Observation, PartialObservation};

use super::TestApp;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ListObservations {
    pub observations: Vec<Observation>,
}

impl TestApp {
    pub async fn list_observations(&mut self) -> anyhow::Result<ListObservations> {
        let result = self.get("/observations")?.exchange().await?.as_json()?;
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
            .as_json()?;
        Ok(result)
    }
}
