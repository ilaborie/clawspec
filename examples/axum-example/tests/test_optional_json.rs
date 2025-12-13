#![allow(missing_docs)]

use rstest::rstest;
use tracing::info;

use axum_example::observations::domain::{LngLat, Observation, PartialObservation};

mod common;
pub use self::common::*;

#[derive(serde::Deserialize, utoipa::ToSchema)]
struct ObservationsList {
    observations: Vec<Observation>,
}

#[rstest]
#[tokio::test]
async fn test_as_optional_json_returns_some_for_successful_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_optional_json with successful response");

    let test_observation = PartialObservation {
        name: "Test Bird".to_string(),
        position: LngLat {
            lng: 10.0,
            lat: 20.0,
        },
        color: Some("red".to_string()),
        notes: Some("Testing optional json".to_string()),
    };

    // Test with POST which returns 201 and the created Observation
    let created: Option<Observation> = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_optional_json()
        .await?;

    assert!(
        created.is_some(),
        "Should return Some for successful 201 response"
    );
    info!(
        "Created observation with ID: {:?}",
        created.map(|obs| obs.id)
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_optional_json_returns_some_for_list_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_optional_json with list response");

    let test_observation = PartialObservation {
        name: "List Test Bird".to_string(),
        position: LngLat {
            lng: 15.0,
            lat: 25.0,
        },
        color: Some("green".to_string()),
        notes: Some("Testing list with optional json".to_string()),
    };

    // Create an observation first
    let _created: Observation = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_json()
        .await?;

    // Test with GET /observations which returns 200 and a list
    let result: Option<ObservationsList> =
        app.get("/observations")?.await?.as_optional_json().await?;

    assert!(
        result.is_some(),
        "Should return Some for successful list response"
    );
    let observations_list = result.expect("Observations list should be present");
    assert!(
        !observations_list.observations.is_empty(),
        "Observations list should not be empty"
    );
    info!(
        "Retrieved {} observations",
        observations_list.observations.len()
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_optional_json_returns_none_for_404(#[future] app: TestApp) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_optional_json with 404 response");

    // Try to delete a non-existent observation (should return 404)
    let result: Option<Observation> = app
        .delete("/observations/00000000-0000-0000-0000-000000000000")?
        .add_expected_status(404)
        .await?
        .as_optional_json()
        .await?;

    assert!(result.is_none(), "Should return None for 404 response");
    info!("Correctly returned None for 404 response");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_optional_json_ergonomics_comparison(#[future] app: TestApp) -> anyhow::Result<()> {
    let app = app.await;

    info!("Demonstrating ergonomics of as_optional_json vs manual status checking");

    // Try to delete a non-existent observation
    let observation: Option<Observation> = app
        .delete("/observations/00000000-0000-0000-0000-000000000001")?
        .add_expected_status(404)
        .await?
        .as_optional_json()
        .await?;

    // Clean ergonomic handling with Option<T>
    if let Some(obs) = observation {
        info!("Found observation: {:?}", obs.data.name);
    } else {
        info!("Observation not found - handled gracefully with Option<T>");
    }

    Ok(())
}
