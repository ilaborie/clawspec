#![allow(missing_docs)]

use anyhow::Context;
use axum_example::observations::ListOption;
use axum_example::observations::domain::{LngLat, PartialObservation, PatchObservation};
use rstest::rstest;
use tracing::info;

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn should_generate_openapi(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    // List observations with default parameters (no query params)
    let _list_default = app
        .list_observations(None)
        .await
        .context("should list observations with default parameters")?;

    info!("Create observation to get an ID for path parameter examples");
    let new_observation = PartialObservation {
        name: "Parrot".to_string(),
        position: LngLat {
            lng: 12.4,
            lat: -25.1,
        },
        color: Some("blue".to_string()),
        notes: None,
    };

    info!("Create an observation");
    let created_id = app
        .create_observation(&new_observation)
        .await
        .context("should create observation")?;

    info!("List observations with query parameters for examples");
    let _list_with_offset = app
        .list_observations(Some(ListOption {
            offset: 5,
            limit: 10,
        }))
        .await
        .context("should list observations with offset")?;

    info!("List observations with different pagination for more examples");
    let _list_with_limit = app
        .list_observations(Some(ListOption {
            offset: 0,
            limit: 20,
        }))
        .await
        .context("should list observations with larger limit")?;

    info!("Update observation");
    let updated_observation = PartialObservation {
        name: "Updated Parrot".to_string(),
        position: LngLat {
            lng: 13.5,
            lat: -26.2,
        },
        color: Some("green".to_string()),
        notes: Some("Updated via API".to_string()),
    };
    app.update_observation(created_id, &updated_observation)
        .await
        .context("updating observation")?;

    info!("Patch observation");
    let patch = PatchObservation {
        name: Some("Partially Updated Parrot".to_string()),
        color: Some("red".to_string()),
        ..Default::default()
    };
    app.patch_observation(created_id, &patch)
        .await
        .context("patching an observation")?;

    info!("Delete observation");
    app.delete_observation(created_id)
        .await
        .context("deleting observation")?;

    app.write_openapi("./doc/openapi.yml")
        .await
        .context("writing openapi file")?;

    Ok(())
}
