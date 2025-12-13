#![allow(missing_docs)]

//! Integration tests for the redaction feature.
//!
//! These tests verify that redacted examples appear correctly in the generated
//! `OpenAPI` specification. The redaction feature allows replacing dynamic values
//! (like UUIDs and timestamps) with stable values for consistent documentation.

use anyhow::Context;
use rstest::rstest;
use tracing::info;
use utoipa::openapi::RefOr;

use axum_example::observations::domain::{LngLat, PartialObservation};

mod common;
pub use self::common::*;

/// Tests that redacted examples appear in the `OpenAPI` response content.
///
/// This test verifies that when using `as_json_redacted()` with redaction
/// replacements, the redacted example values appear in the generated `OpenAPI`
/// specification's response content.
///
/// The redacted values should be:
/// - id: "019aaaaa-0000-7000-8000-000000000000" (stable UUID)
/// - `created_at`: "2024-01-01T00:00:00Z" (stable timestamp)
#[rstest]
#[tokio::test]
async fn test_redacted_example_appears_in_openapi_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing that redacted examples appear in OpenAPI response content");

    let new_observation = PartialObservation {
        name: "Redaction Test Bird".to_string(),
        position: LngLat { lng: 1.0, lat: 2.0 },
        color: Some("blue".to_string()),
        notes: Some("Testing redaction example capture".to_string()),
    };

    // Create observation using redaction to replace dynamic values with stable ones
    let created = app
        .create_observation_redacted(&new_observation)
        .await
        .context("should create observation with redacted response")?;

    info!(
        "Created observation with id={} and created_at={:?}",
        created.id, created.created_at
    );

    // Verify the actual values are dynamic (not the redacted stable ones)
    let actual_id = created.id.to_string();
    let actual_created_at = created.created_at.to_string();

    assert_ne!(
        actual_id, "019aaaaa-0000-7000-8000-000000000000",
        "actual id should be a dynamic UUID, not the redacted one"
    );
    assert_ne!(
        actual_created_at, "2024-01-01T00:00:00Z",
        "actual created_at should be a dynamic timestamp, not the redacted one"
    );

    // Generate OpenAPI spec
    let openapi_spec = app.collected_openapi().await;

    // Find the POST /observations operation
    let paths = &openapi_spec.paths;
    let observations_path = paths
        .paths
        .get("/api/observations")
        .expect("should have /api/observations path");

    let post_operation = observations_path
        .post
        .as_ref()
        .expect("should have POST operation");

    // Find the 201 response
    let responses = &post_operation.responses;
    let response_201 = responses
        .responses
        .get("201")
        .expect("should have 201 response");

    let response = match response_201 {
        RefOr::T(response) => response,
        RefOr::Ref(_) => panic!("expected inline response, got reference"),
    };

    // Get the application/json content
    let json_content = response
        .content
        .get("application/json")
        .expect("should have application/json content");

    // Verify that the example contains the redacted values
    let example = json_content
        .example
        .as_ref()
        .expect("response content should have an example with redacted values");

    info!("Found example in response: {}", example);

    // Verify the example contains the REDACTED id (not the actual dynamic UUID)
    let example_id = example
        .get("id")
        .expect("example should have 'id' field")
        .as_str()
        .expect("id should be a string");

    assert_eq!(
        example_id, "019aaaaa-0000-7000-8000-000000000000",
        "example id should be the redacted stable UUID, not the actual dynamic one"
    );
    assert_ne!(
        example_id, actual_id,
        "example id should differ from the actual dynamic id"
    );

    // Verify the example contains the REDACTED created_at (not the actual dynamic timestamp)
    let example_created_at = example
        .get("created_at")
        .expect("example should have 'created_at' field")
        .as_str()
        .expect("created_at should be a string");

    assert_eq!(
        example_created_at, "2024-01-01T00:00:00Z",
        "example created_at should be the redacted stable timestamp, not the actual dynamic one"
    );
    assert_ne!(
        example_created_at, actual_created_at,
        "example created_at should differ from the actual dynamic timestamp"
    );

    // Verify the non-redacted observation data is preserved in the example
    let example_name = example
        .get("name")
        .expect("example should have 'name' field")
        .as_str()
        .expect("name should be a string");

    assert_eq!(
        example_name, "Redaction Test Bird",
        "example should contain the original name (not redacted)"
    );

    Ok(())
}

/// Tests that non-redacted responses do NOT have examples in the response content.
///
/// This test establishes the baseline behavior: regular `as_json()` calls
/// do not add examples to the response content. Only redacted responses should.
#[rstest]
#[tokio::test]
async fn test_non_redacted_response_has_no_example(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing that non-redacted responses have no example in OpenAPI");

    let new_observation = PartialObservation {
        name: "Non-Redacted Test Bird".to_string(),
        position: LngLat { lng: 3.0, lat: 4.0 },
        color: Some("green".to_string()),
        notes: None,
    };

    // Create observation WITHOUT redaction
    let _created = app
        .create_observation(&new_observation)
        .await
        .context("should create observation")?;

    // Generate OpenAPI spec
    let openapi_spec = app.collected_openapi().await;

    // Find the POST /observations operation
    let paths = &openapi_spec.paths;
    let observations_path = paths
        .paths
        .get("/api/observations")
        .expect("should have /api/observations path");

    let post_operation = observations_path
        .post
        .as_ref()
        .expect("should have POST operation");

    // Find the 201 response
    let responses = &post_operation.responses;
    let response_201 = responses
        .responses
        .get("201")
        .expect("should have 201 response");

    let response = match response_201 {
        RefOr::T(response) => response,
        RefOr::Ref(_) => panic!("expected inline response, got reference"),
    };

    // Get the application/json content
    let json_content = response
        .content
        .get("application/json")
        .expect("should have application/json content");

    // Non-redacted responses should NOT have an example
    // (This is the expected current behavior)
    assert!(
        json_content.example.is_none(),
        "non-redacted response content should not have an example"
    );

    Ok(())
}
