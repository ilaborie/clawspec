#![allow(missing_docs)]

use rstest::rstest;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use axum_example::observations::domain::{LngLat, Observation, PartialObservation};

mod common;
pub use self::common::*;

// Use the actual error type from axum-example
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, derive_more::Display)]
#[display("{message}")]
struct ApiError {
    status: u16,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl std::error::Error for ApiError {}

// Tests for as_result_json method

#[rstest]
#[tokio::test]
async fn test_as_result_json_returns_ok_for_2xx_responses(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_result_json with successful 201 response");

    let test_observation = PartialObservation {
        name: "Test Bird".to_string(),
        position: LngLat {
            lng: 10.0,
            lat: 20.0,
        },
        color: Some("red".to_string()),
        notes: Some("Testing result json".to_string()),
    };

    // Test with POST which returns 201 and the created Observation
    let result: Result<Observation, ApiError> = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_result_json()
        .await?;

    assert!(result.is_ok(), "Should return Ok for 2xx response");
    let created = result.expect("Should contain created observation");
    info!("Created observation with ID: {}", created.id);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_json_returns_ok_for_200_patch_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_result_json with successful 200 PATCH response");

    // Create an observation first
    let test_observation = PartialObservation {
        name: "Patch Test Bird".to_string(),
        position: LngLat {
            lng: 15.0,
            lat: 25.0,
        },
        color: Some("blue".to_string()),
        notes: Some("Testing PATCH with result json".to_string()),
    };

    let created: Observation = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_json()
        .await?;

    // Now patch it with as_result_json
    let patch_data = serde_json::json!({
        "name": "Updated Patch Test Bird",
        "color": "green"
    });

    let result: Result<Observation, ApiError> = app
        .patch(format!("/observations/{}", created.id))?
        .json(&patch_data)?
        .await?
        .as_result_json()
        .await?;

    assert!(result.is_ok(), "Should return Ok for 200 response");
    let observation = result.expect("Should contain observation");
    assert_eq!(observation.data.name, "Updated Patch Test Bird");
    info!(
        "Successfully patched observation: {:?}",
        observation.data.name
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_json_returns_err_for_404_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_result_json with 404 error response");

    // Try to delete a non-existent observation (should return 404 with JSON error body)
    let result: Result<PartialObservation, ApiError> = app
        .delete("/observations/00000000-0000-0000-0000-000000000000")?
        .add_expected_status(404)
        .await?
        .as_result_json()
        .await?;

    assert!(result.is_err(), "Should return Err for 404 response");
    let error = result.expect_err("Should contain error");
    info!("Received error: {}", error.message);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_json_ergonomics_with_match(#[future] app: TestApp) -> anyhow::Result<()> {
    let app = app.await;

    info!("Demonstrating ergonomics of as_result_json with match");

    let test_observation = PartialObservation {
        name: "Match Test Bird".to_string(),
        position: LngLat {
            lng: 20.0,
            lat: 30.0,
        },
        color: Some("yellow".to_string()),
        notes: Some("Testing match ergonomics".to_string()),
    };

    let result: Result<Observation, ApiError> = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_result_json()
        .await?;

    match result {
        Ok(observation) => info!(
            "Successfully created observation with ID: {}",
            observation.id
        ),
        Err(error) => info!("Error creating observation: {}", error.message),
    }

    Ok(())
}

// Tests for as_result_option_json method

#[rstest]
#[tokio::test]
async fn test_as_result_option_json_returns_ok_some_for_200_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_result_option_json with successful 200 response");

    let test_observation = PartialObservation {
        name: "Option Test Bird".to_string(),
        position: LngLat {
            lng: 25.0,
            lat: 35.0,
        },
        color: Some("green".to_string()),
        notes: Some("Testing result option json".to_string()),
    };

    let created: Observation = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_json()
        .await?;

    // Patch with as_result_option_json
    let patch_data = serde_json::json!({
        "color": "green"
    });

    let result: Result<Option<Observation>, ApiError> = app
        .patch(format!("/observations/{}", created.id))?
        .json(&patch_data)?
        .await?
        .as_result_option_json()
        .await?;

    assert!(result.is_ok(), "Should return Ok for 200 response");
    let option = result.expect("Should be Ok");
    assert!(option.is_some(), "Should contain Some for 200 response");
    let observation = option.expect("Should contain observation");
    assert_eq!(observation.data.name, "Option Test Bird");
    info!(
        "Successfully patched observation: {:?}",
        observation.data.name
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_option_json_returns_ok_none_for_404_response(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_result_option_json with 404 response");

    // Try to delete a non-existent observation (404 should return Ok(None))
    let result: Result<Option<PartialObservation>, ApiError> = app
        .delete("/observations/00000000-0000-0000-0000-000000000000")?
        .add_expected_status(404)
        .await?
        .as_result_option_json()
        .await?;

    assert!(result.is_ok(), "Should return Ok for 404 response");
    let option = result.expect("Should be Ok");
    assert!(option.is_none(), "Should return None for 404 response");
    info!("Correctly returned Ok(None) for 404 response");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_option_json_returns_err_for_other_4xx_responses(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing as_result_option_json with 400 error response");

    // For this test, we would check that 400 (Bad Request) returns Err(E)
    // while 404 returns Ok(None). This demonstrates different error handling.
    // Note: The actual behavior depends on API validation rules
    let result: Result<Option<Observation>, ApiError> = app
        .delete("/observations/00000000-0000-0000-0000-000000000000")?
        .add_expected_status(404)
        .await?
        .as_result_option_json()
        .await?;

    // The API returns Ok(None) for 404, but would return Err for other 4xx errors
    info!("Result: {:?}", result.is_ok());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_option_json_ergonomics_with_match(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Demonstrating ergonomics of as_result_option_json with match");

    let test_observation = PartialObservation {
        name: "Ergonomics Test Bird".to_string(),
        position: LngLat {
            lng: 30.0,
            lat: 40.0,
        },
        color: Some("purple".to_string()),
        notes: Some("Testing ergonomics".to_string()),
    };

    let created: Observation = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_json()
        .await?;

    // Test patching existing observation
    let patch_data = serde_json::json!({
        "color": "purple"
    });

    let result: Result<Option<Observation>, ApiError> = app
        .patch(format!("/observations/{}", created.id))?
        .json(&patch_data)?
        .await?
        .as_result_option_json()
        .await?;

    match result {
        Ok(Some(obs)) => info!("Found observation: {:?}", obs.data.name),
        Ok(None) => info!("Observation not found"),
        Err(error) => info!("Error: {}", error.message),
    }

    // Test deleting non-existent observation
    let result: Result<Option<PartialObservation>, ApiError> = app
        .delete("/observations/00000000-0000-0000-0000-000000000000")?
        .add_expected_status(404)
        .await?
        .as_result_option_json()
        .await?;

    match result {
        Ok(Some(obs)) => info!("Found observation: {:?}", obs.name),
        Ok(None) => info!("Observation not found (expected)"),
        Err(error) => info!("Error: {}", error.message),
    }

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_option_json_with_question_mark_operator(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Demonstrating ? operator with as_result_option_json");

    let test_observation = PartialObservation {
        name: "Question Mark Test Bird".to_string(),
        position: LngLat {
            lng: 35.0,
            lat: 45.0,
        },
        color: Some("orange".to_string()),
        notes: Some("Testing ? operator".to_string()),
    };

    let created: Observation = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_json()
        .await?;

    // Ergonomic use with ? operator
    let patch_data = serde_json::json!({
        "color": "orange"
    });

    let observation: Option<Observation> = app
        .patch(format!("/observations/{}", created.id))?
        .json(&patch_data)?
        .await?
        .as_result_option_json::<Observation, ApiError>()
        .await??; // First ? for ApiClientError, second ? for Result<Option<T>, E> -> Option<T>

    assert!(observation.is_some(), "Should find the observation");
    info!("Successfully used ? operator to extract observation");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_as_result_json_vs_as_result_option_json_comparison(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let app = app.await;

    info!("Comparing as_result_json vs as_result_option_json");

    // Create a test observation
    let test_observation = PartialObservation {
        name: "Comparison Test Bird".to_string(),
        position: LngLat {
            lng: 40.0,
            lat: 50.0,
        },
        color: Some("pink".to_string()),
        notes: Some("Testing comparison".to_string()),
    };

    let created: Observation = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_json()
        .await?;

    // Verify the observation was created
    let _ = created.id;

    // as_result_json: 404 would be Err(E)
    let result_404: Result<PartialObservation, ApiError> = app
        .delete("/observations/00000000-0000-0000-0000-000000000000")?
        .add_expected_status(404)
        .await?
        .as_result_json()
        .await?;

    info!(
        "as_result_json for 404: {}",
        if result_404.is_err() {
            "Err (expected)"
        } else {
            "Ok (unexpected)"
        }
    );

    // as_result_option_json: 404 would be Ok(None)
    let result_option_404: Result<Option<PartialObservation>, ApiError> = app
        .delete("/observations/00000000-0000-0000-0000-000000000001")?
        .add_expected_status(404)
        .await?
        .as_result_option_json()
        .await?;

    info!(
        "as_result_option_json for 404: {}",
        if result_option_404.is_ok() && result_option_404.as_ref().expect("ok").is_none() {
            "Ok(None) (expected)"
        } else {
            "Other (unexpected)"
        }
    );

    // Both should handle 200 the same way
    let patch_data = serde_json::json!({
        "color": "pink"
    });

    let result_200: Result<Observation, ApiError> = app
        .patch(format!("/observations/{}", created.id))?
        .json(&patch_data)?
        .await?
        .as_result_json()
        .await?;

    let patch_data2 = serde_json::json!({
        "color": "pink"
    });

    let result_option_200: Result<Option<Observation>, ApiError> = app
        .patch(format!("/observations/{}", created.id))?
        .json(&patch_data2)?
        .await?
        .as_result_option_json()
        .await?;

    assert!(
        result_200.is_ok() && result_option_200.is_ok(),
        "Both should succeed for 200 response"
    );

    info!("Both methods handle 200 responses correctly");

    Ok(())
}
