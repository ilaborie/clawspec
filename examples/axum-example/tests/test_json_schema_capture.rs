#![allow(missing_docs)]

use anyhow::Context;
use clawspec_core::register_schemas;
use rstest::rstest;
use tracing::info;

use axum_example::observations::domain::{LngLat, PartialObservation};

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn test_json_schema_capture_without_explicit_registration(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let mut app = app.await;

    register_schemas!(app, LngLat).await;

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

    let created_id = app
        .post("/observations")?
        .json(&new_observation)?
        .await
        .context("should create observation")?
        .as_json::<u32>()
        .await
        .context("should deserialize created observation ID")?;

    info!("Created observation with ID: {}", created_id);

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

    Ok(())
}
