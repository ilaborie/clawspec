use std::time::{Duration, Instant};

use anyhow::Context;

use crate::observations::repository::ObservationRepository;

/// The application state
#[derive(Clone, axum::extract::FromRef)]
pub struct AppState {
    #[from_ref(skip)]
    started_at: Instant,
    repository: ObservationRepository,
}

impl AppState {
    /// Create a state
    ///
    /// # Errors
    ///
    /// Fail if the repository cannot be built
    pub fn new() -> anyhow::Result<Self> {
        let started_at = Instant::now();
        let repository = ObservationRepository::new().context("building repository")?;

        Ok(Self {
            started_at,
            repository,
        })
    }
}

impl AppState {
    pub(crate) fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }
}
