use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use tokio::sync::RwLock;

use super::domain::{Observation, ObservationId, PartialObservation, PatchObservation};
use crate::errors::RepositoryError;

type StoredData = (Timestamp, PartialObservation);

#[derive(Clone)]
pub(crate) struct ObservationRepository {
    // TODO could be improve with [dashmap](https://crates.io/crates/dashmap) - https://github.com/ilaborie/clawspec/issues/26
    data: Arc<RwLock<HashMap<ObservationId, StoredData>>>,
}

impl ObservationRepository {
    pub(crate) fn new() -> Result<Self, RepositoryError> {
        let jsonl = include_str!("./db.jsonl");
        let data = jsonl
            .lines()
            .filter(|line| !line.is_empty())
            .map(serde_json::from_str::<Observation>)
            .collect::<Result<Vec<_>, _>>()?;
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

        let mut result = data
            .into_iter()
            .map(|(id, (created_at, data))| Observation {
                id,
                created_at,
                data,
            })
            .collect::<Vec<_>>();
        // sort
        result.sort_by_key(|it| (it.data.name.clone(), it.id));
        // Limit/offset
        let result = result.into_iter().skip(offset).take(limit).collect();

        Ok(result)
    }

    pub(crate) async fn create(
        &self,
        new_observation: PartialObservation,
    ) -> Result<Observation, RepositoryError> {
        let mut data = self.data.write().await;
        let id = ObservationId::new();
        let created_at = Timestamp::now();
        data.insert(id, (created_at, new_observation.clone()));

        Ok(Observation {
            id,
            created_at,
            data: new_observation,
        })
    }

    pub(crate) async fn update(
        &self,
        id: ObservationId,
        updated_observation: PartialObservation,
    ) -> Result<(), RepositoryError> {
        let mut data = self.data.write().await;
        let Some((created_at, _)) = data.get(&id) else {
            return Err(RepositoryError::ObservationNotFound { id });
        };
        let created_at = *created_at;

        data.insert(id, (created_at, updated_observation));

        Ok(())
    }

    pub(crate) async fn patch(
        &self,
        id: ObservationId,
        patch: PatchObservation,
    ) -> Result<Observation, RepositoryError> {
        let mut data = self.data.write().await;

        let Some((created_at, value)) = data.get_mut(&id) else {
            return Err(RepositoryError::ObservationNotFound { id });
        };
        let created_at = *created_at;

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

        Ok(Observation {
            id,
            created_at,
            data: value.clone(),
        })
    }

    pub(crate) async fn delete(
        &self,
        id: ObservationId,
    ) -> Result<PartialObservation, RepositoryError> {
        let mut data = self.data.write().await;

        let Some((_, result)) = data.remove(&id) else {
            return Err(RepositoryError::ObservationNotFound { id });
        };

        Ok(result)
    }
}

impl Observation {
    fn split(self) -> (ObservationId, StoredData) {
        let Self {
            id,
            created_at,
            data,
        } = self;
        (id, (created_at, data))
    }
}
