#![allow(missing_docs)]

use anyhow::Context;
use axum_example::observations::domain::{LngLat, PartialObservation};
use rstest::rstest;

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn should_generate_openapi(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    // List
    let _list = app
        .list_observations()
        .await
        .context("should list observation")?;

    // Create
    let new_observation = PartialObservation {
        name: "Parrot".to_string(),
        position: LngLat {
            lng: 12.4,
            lat: -25.1,
        },
        color: Some("blue".to_string()),
        notes: None,
    };
    let _created = app
        .create_observation(&new_observation)
        .await
        .context("should create observation");

    app.write_openapi("./doc/openapi.yml")
        .await
        .context("writing openapi file")?;

    Ok(())
}
