use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::domain::{Observation, ObservationId, PartialObservation, PatchObservation};
use crate::errors::RepositoryError;

#[derive(Clone)]
pub(crate) struct ObservationRepository {
    // TODO could be improve with [dashmap](https://crates.io/crates/dashmap) - https://github.com/ilaborie/clawspec/issues/26
    data: Arc<RwLock<HashMap<ObservationId, PartialObservation>>>,
}

impl ObservationRepository {
    pub(crate) fn new() -> Result<Self, RepositoryError> {
        let json = include_str!("./db.json");
        let data = serde_json::from_str::<Vec<Observation>>(json)?;
        let data = data.into_iter().map(Observation::split).collect();
        let data = Arc::new(RwLock::new(data));

        Ok(Self { data })
    }

    pub(crate) async fn list(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Observation>, RepositoryError> {
        let data = self.data.read().await;
        let data = data.clone();

        let mut result = data.into_iter().map(Observation::from).collect::<Vec<_>>();
        // sort
        result.sort_by_key(|it| (it.data.name.to_string(), it.id));
        // Limit/offset
        let result = result.into_iter().skip(offset).take(limit).collect();

        Ok(result)
    }

    pub(crate) async fn create(
        &self,
        new_observation: PartialObservation,
    ) -> Result<ObservationId, RepositoryError> {
        let mut data = self.data.write().await;
        let id = {
            let max = data.keys().map(|it| it.0).max().unwrap_or(0);
            ObservationId(max + 1)
        };
        data.insert(id, new_observation);

        Ok(id)
    }

    pub(crate) async fn update(
        &self,
        id: ObservationId,
        updated_observation: PartialObservation,
    ) -> Result<(), RepositoryError> {
        let mut data = self.data.write().await;
        if !data.contains_key(&id) {
            return Err(RepositoryError::ObservationNotFound { id });
        }

        data.insert(id, updated_observation);

        Ok(())
    }

    pub(crate) async fn patch(
        &self,
        id: ObservationId,
        patch: PatchObservation,
    ) -> Result<Observation, RepositoryError> {
        let mut data = self.data.write().await;

        let Some(value) = data.get_mut(&id) else {
            return Err(RepositoryError::ObservationNotFound { id });
        };

        let PatchObservation {
            name,
            position,
            color,
            notes,
        } = patch;
        if let Some(name) = name {
            value.name = name;
        }
        if let Some(position) = position {
            value.position = position;
        }
        value.color = color;
        value.notes = notes;

        let result = Observation::from((id, value.clone()));
        Ok(result)
    }

    pub(crate) async fn delete(
        &self,
        id: ObservationId,
    ) -> Result<PartialObservation, RepositoryError> {
        let mut data = self.data.write().await;

        let Some(result) = data.remove(&id) else {
            return Err(RepositoryError::ObservationNotFound { id });
        };

        Ok(result)
    }
}

impl Observation {
    fn split(self) -> (ObservationId, PartialObservation) {
        let Self { id, data } = self;
        (id, data)
    }
}

impl From<(ObservationId, PartialObservation)> for Observation {
    fn from(value: (ObservationId, PartialObservation)) -> Self {
        let (id, data) = value;
        Self { id, data }
    }
}
