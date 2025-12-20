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

/// Tests wildcard redaction with `JSONPath` for array elements.
///
/// This test verifies that `JSONPath` wildcards like `$.observations[*].id` correctly
/// redact all matching fields in an array with a single call.
#[rstest]
#[tokio::test]
async fn test_wildcard_redaction_in_array(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing wildcard redaction with JSONPath for array elements");

    // Create multiple observations to test wildcard redaction
    let observations_to_create = vec![
        PartialObservation {
            name: "First Bird".to_string(),
            position: LngLat { lng: 1.0, lat: 1.0 },
            color: Some("red".to_string()),
            notes: None,
        },
        PartialObservation {
            name: "Second Bird".to_string(),
            position: LngLat { lng: 2.0, lat: 2.0 },
            color: Some("blue".to_string()),
            notes: None,
        },
        PartialObservation {
            name: "Third Bird".to_string(),
            position: LngLat { lng: 3.0, lat: 3.0 },
            color: Some("green".to_string()),
            notes: None,
        },
    ];

    // Create the observations
    for obs in &observations_to_create {
        app.create_observation(obs).await?;
    }

    // List observations with wildcard redaction
    let result = app.list_observations_redacted(None).await?;

    info!(
        "Listed {} observations with wildcard redaction",
        result.value.observations.len()
    );

    // Verify we got at least 3 observations
    assert!(
        result.value.observations.len() >= 3,
        "should have at least 3 observations"
    );

    // Verify the actual values are dynamic (different UUIDs)
    let ids: Vec<_> = result
        .value
        .observations
        .iter()
        .map(|obs| obs.id.to_string())
        .collect();

    // All actual IDs should be unique
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        ids.len(),
        "all observation IDs should be unique"
    );

    // None of the actual IDs should be the redacted value
    for id in &ids {
        assert_ne!(
            id, "019aaaaa-0000-7000-8000-000000000000",
            "actual IDs should not be the redacted stable UUID"
        );
    }

    // Verify the redacted JSON has all IDs replaced with the stable value
    let redacted_observations = result
        .redacted
        .get("observations")
        .expect("redacted should have observations")
        .as_array()
        .expect("observations should be an array");

    for (i, redacted_obs) in redacted_observations.iter().enumerate() {
        let redacted_id = redacted_obs
            .get("id")
            .expect("redacted observation should have id")
            .as_str()
            .expect("id should be a string");

        assert_eq!(
            redacted_id, "019aaaaa-0000-7000-8000-000000000000",
            "redacted observation[{i}].id should be the stable UUID"
        );

        let redacted_created_at = redacted_obs
            .get("created_at")
            .expect("redacted observation should have created_at")
            .as_str()
            .expect("created_at should be a string");

        assert_eq!(
            redacted_created_at, "2024-01-01T00:00:00Z",
            "redacted observation[{i}].created_at should be the stable timestamp"
        );

        // Verify non-redacted fields are preserved (name should be a non-empty string)
        let name = redacted_obs
            .get("name")
            .expect("redacted observation should have name")
            .as_str()
            .expect("name should be a string");

        assert!(
            !name.is_empty(),
            "observation name should be preserved and non-empty"
        );
    }

    // Generate OpenAPI spec and verify the example is included
    let openapi_spec = app.collected_openapi().await;

    let paths = &openapi_spec.paths;
    let observations_path = paths
        .paths
        .get("/api/observations")
        .expect("should have /api/observations path");

    let get_operation = observations_path
        .get
        .as_ref()
        .expect("should have GET operation");

    let responses = &get_operation.responses;
    let response_200 = responses
        .responses
        .get("200")
        .expect("should have 200 response");

    let response = match response_200 {
        RefOr::T(response) => response,
        RefOr::Ref(_) => panic!("expected inline response, got reference"),
    };

    let json_content = response
        .content
        .get("application/json")
        .expect("should have application/json content");

    // Verify the example exists and contains redacted values
    let example = json_content
        .example
        .as_ref()
        .expect("response should have example with wildcard-redacted values");

    let example_observations = example
        .get("observations")
        .expect("example should have observations")
        .as_array()
        .expect("observations should be an array");

    // All IDs in the example should be the redacted stable UUID
    for (i, obs) in example_observations.iter().enumerate() {
        let example_id = obs.get("id").and_then(|val| val.as_str());
        assert_eq!(
            example_id,
            Some("019aaaaa-0000-7000-8000-000000000000"),
            "example observation[{i}].id should be the redacted stable UUID"
        );
    }

    info!("Wildcard redaction test passed successfully");

    Ok(())
}

/// Tests closure-based redaction with path-aware index extraction.
///
/// This test verifies that closures can be used to generate stable,
/// distinguishable IDs based on array index: `obs-0`, `obs-1`, `obs-2`, etc.
#[rstest]
#[tokio::test]
async fn test_closure_based_redaction_with_index(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing closure-based redaction with index-aware ID generation");

    // Create multiple observations to test closure-based redaction
    let observations_to_create = vec![
        PartialObservation {
            name: "Alpha Bird".to_string(),
            position: LngLat { lng: 1.0, lat: 1.0 },
            color: Some("red".to_string()),
            notes: None,
        },
        PartialObservation {
            name: "Beta Bird".to_string(),
            position: LngLat { lng: 2.0, lat: 2.0 },
            color: Some("blue".to_string()),
            notes: None,
        },
        PartialObservation {
            name: "Gamma Bird".to_string(),
            position: LngLat { lng: 3.0, lat: 3.0 },
            color: Some("green".to_string()),
            notes: None,
        },
    ];

    // Create the observations
    for obs in &observations_to_create {
        app.create_observation(obs).await?;
    }

    // List observations with closure-based index redaction
    let result = app.list_observations_with_indexed_ids(None).await?;

    info!(
        "Listed {} observations with closure-based redaction",
        result.value.observations.len()
    );

    // Verify we got at least 3 observations
    assert!(
        result.value.observations.len() >= 3,
        "should have at least 3 observations"
    );

    // Verify the actual values are dynamic (different UUIDs)
    let actual_ids: Vec<_> = result
        .value
        .observations
        .iter()
        .map(|obs| obs.id.to_string())
        .collect();

    // All actual IDs should be unique
    let unique_ids: std::collections::HashSet<_> = actual_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        actual_ids.len(),
        "all observation IDs should be unique"
    );

    // Verify the redacted JSON has index-based IDs (obs-0, obs-1, obs-2, etc.)
    let redacted_observations = result
        .redacted
        .get("observations")
        .expect("redacted should have observations")
        .as_array()
        .expect("observations should be an array");

    for (idx, redacted_obs) in redacted_observations.iter().enumerate() {
        let redacted_id = redacted_obs
            .get("id")
            .expect("redacted observation should have id")
            .as_str()
            .expect("id should be a string");

        let expected_id = format!("obs-{idx}");
        assert_eq!(
            redacted_id, expected_id,
            "redacted observation[{idx}].id should be '{expected_id}', got '{redacted_id}'"
        );
    }

    // Generate OpenAPI spec and verify the example is included
    let openapi_spec = app.collected_openapi().await;

    let paths = &openapi_spec.paths;
    let observations_path = paths
        .paths
        .get("/api/observations")
        .expect("should have /api/observations path");

    let get_operation = observations_path
        .get
        .as_ref()
        .expect("should have GET operation");

    let responses = &get_operation.responses;
    let response_200 = responses
        .responses
        .get("200")
        .expect("should have 200 response");

    let response = match response_200 {
        RefOr::T(response) => response,
        RefOr::Ref(_) => panic!("expected inline response, got reference"),
    };

    let json_content = response
        .content
        .get("application/json")
        .expect("should have application/json content");

    // Verify the example exists and contains closure-generated values
    let example = json_content
        .example
        .as_ref()
        .expect("response should have example with closure-generated values");

    let example_observations = example
        .get("observations")
        .expect("example should have observations")
        .as_array()
        .expect("observations should be an array");

    // Verify IDs in the example are index-based (obs-0, obs-1, etc.)
    for (idx, obs) in example_observations.iter().enumerate() {
        let example_id = obs.get("id").and_then(|val| val.as_str());
        let expected_id = format!("obs-{idx}");
        assert_eq!(
            example_id,
            Some(expected_id.as_str()),
            "example observation[{idx}].id should be '{expected_id}'"
        );
    }

    info!("Closure-based redaction test passed successfully");

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
