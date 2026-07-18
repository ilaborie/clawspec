#![allow(missing_docs)]

use anyhow::Context;
use rstest::rstest;
use tracing::info;

use axum_example::observations::domain::{LngLat, Observation, PartialObservation};

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn test_json_schema_capture_without_explicit_registration(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let mut app = app.await;

    // No explicit `register_schemas!(app, LngLat)` call: LngLat is never registered directly
    // and is never itself a top-level body/response type. It is only reached as a nested field
    // (of PartialObservation, and transitively of the flattened Observation response), so it
    // must be captured automatically via utoipa's recursive ToSchema::schemas() walk.
    info!("Testing JSON schema capture with only create observation endpoint");
    let new_observation = PartialObservation {
        name: "Test Bird for Schema Capture".to_string(),
        position: LngLat {
            lng: 12.4,
            lat: -25.1,
        },
        color: Some("blue".to_string()),
        notes: Some("Testing automatic schema capture".to_string()),
    };

    let created = app
        .post("/observations")?
        .json(&new_observation)?
        .await
        .context("should create observation")?
        .as_json::<Observation>()
        .await
        .context("should deserialize created observation")?;

    info!("Created observation with ID: {}", created.id);

    // Generate OpenAPI spec to verify schemas were captured
    let openapi_spec = app.collected_openapi().await;

    // Write to a test-specific file to inspect
    insta::assert_yaml_snapshot!(openapi_spec);

    // Verify that the main schema is now captured automatically
    let components = openapi_spec.components.as_ref().expect("having components");
    let schemas = &components.schemas;

    // This should succeed now
    assert!(
        schemas.contains_key("PartialObservation"),
        "PartialObservation schema should be captured"
    );
    assert!(
        schemas.contains_key("LngLat"),
        "LngLat schema should be captured automatically as a nested type, with no explicit registration"
    );

    Ok(())
}
