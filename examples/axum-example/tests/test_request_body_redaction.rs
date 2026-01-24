#![allow(missing_docs)]

//! Integration tests for the request body redaction feature.
//!
//! These tests verify that redacted request body examples appear correctly in the
//! generated `OpenAPI` specification. The request body redaction feature allows
//! hiding sensitive values (like passwords, API keys, internal notes) from the
//! `OpenAPI` documentation while sending the real values in the HTTP request.

use anyhow::Context;
use rstest::rstest;
use tracing::info;
use utoipa::openapi::RefOr;

use axum_example::observations::domain::{LngLat, PartialObservation};

mod common;
pub use self::common::*;

/// Tests that redacted request body examples appear in the `OpenAPI` request content.
///
/// This test verifies that when using `json_redacted()` with redaction
/// replacements, the redacted example values appear in the generated `OpenAPI`
/// specification's request body content.
///
/// The redacted value should be:
/// - notes: `[INTERNAL_NOTES_REDACTED]` (sensitive internal notes hidden)
#[rstest]
#[tokio::test]
async fn test_redacted_request_body_appears_in_openapi(
    #[future] app: TestApp,
) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing that redacted request body examples appear in OpenAPI request content");

    let new_observation = PartialObservation {
        name: "Request Body Redaction Test Bird".to_string(),
        position: LngLat { lng: 1.0, lat: 2.0 },
        color: Some("red".to_string()),
        notes: Some("SENSITIVE: Internal tracking code ABC-123".to_string()),
    };

    // Create observation using request body redaction
    let created = app
        .create_observation_with_redacted_request(&new_observation)
        .await
        .context("should create observation with redacted request body")?;

    info!(
        "Created observation with id={} and notes={:?}",
        created.id, created.data.notes
    );

    // Verify the actual observation has the original notes (not redacted)
    let actual_notes = created.data.notes.as_deref();
    assert_eq!(
        actual_notes,
        Some("SENSITIVE: Internal tracking code ABC-123"),
        "actual notes should be the original value, sent in the HTTP request"
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

    // Find the request body - it's already a RequestBody, not RefOr
    let request_body = post_operation
        .request_body
        .as_ref()
        .expect("should have request body");

    // Get the application/json content
    let json_content = request_body
        .content
        .get("application/json")
        .expect("should have application/json content");

    // Verify that the example contains the redacted value
    let example = json_content
        .example
        .as_ref()
        .expect("request body content should have an example with redacted values");

    info!("Found example in request body: {}", example);

    // Verify the example contains the REDACTED notes (not the actual sensitive value)
    let example_notes = example
        .get("notes")
        .expect("example should have 'notes' field")
        .as_str()
        .expect("notes should be a string");

    assert_eq!(
        example_notes, "[INTERNAL_NOTES_REDACTED]",
        "example notes should be the redacted placeholder, not the actual sensitive value"
    );
    assert_ne!(
        example_notes, "SENSITIVE: Internal tracking code ABC-123",
        "example notes should not contain the actual sensitive value"
    );

    // Verify non-redacted fields are preserved in the example
    let example_name = example
        .get("name")
        .expect("example should have 'name' field")
        .as_str()
        .expect("name should be a string");

    assert_eq!(
        example_name, "Request Body Redaction Test Bird",
        "example should contain the original name (not redacted)"
    );

    info!("Request body redaction test passed successfully");

    Ok(())
}

/// Tests that both request and response body can be redacted together.
///
/// This test verifies the full redaction workflow where:
/// - Request body has sensitive notes redacted
/// - Response body has dynamic id and `created_at` redacted
#[rstest]
#[tokio::test]
async fn test_full_request_and_response_redaction(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing combined request and response body redaction");

    let new_observation = PartialObservation {
        name: "Full Redaction Test Bird".to_string(),
        position: LngLat { lng: 3.0, lat: 4.0 },
        color: Some("blue".to_string()),
        notes: Some("SECRET: This should not appear in docs".to_string()),
    };

    // Create observation with full redaction (both request and response)
    let created = Box::pin(app.create_observation_with_full_redaction(&new_observation))
        .await
        .context("should create observation with full redaction")?;

    info!(
        "Created observation with id={} and notes={:?}",
        created.id, created.data.notes
    );

    // Verify the actual observation has the original notes
    assert_eq!(
        created.data.notes.as_deref(),
        Some("SECRET: This should not appear in docs"),
        "actual notes should be the original value"
    );

    // Verify the actual id is dynamic (not the redacted value)
    assert_ne!(
        created.id.to_string(),
        "019aaaaa-0000-7000-8000-000000000000",
        "actual id should be a dynamic UUID"
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

    // Check request body example - it's already a RequestBody, not RefOr
    let request_body = post_operation
        .request_body
        .as_ref()
        .expect("should have request body");

    let request_content = request_body
        .content
        .get("application/json")
        .expect("should have application/json content");

    let request_example = request_content
        .example
        .as_ref()
        .expect("request body should have example");

    // Verify request body has redacted notes
    let request_notes = request_example
        .get("notes")
        .and_then(serde_json::Value::as_str)
        .expect("request example should have notes");

    assert_eq!(
        request_notes, "[INTERNAL_NOTES_REDACTED]",
        "request body example notes should be redacted"
    );

    // Check response example (201 response)
    let responses = &post_operation.responses;
    let response_201 = responses
        .responses
        .get("201")
        .expect("should have 201 response");

    let response = match response_201 {
        RefOr::T(response) => response,
        RefOr::Ref(_) => panic!("expected inline response, got reference"),
    };

    let response_content = response
        .content
        .get("application/json")
        .expect("should have application/json content");

    let response_example = response_content
        .example
        .as_ref()
        .expect("response should have example");

    // Verify response has redacted id and created_at
    let response_id = response_example
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("response example should have id");

    assert_eq!(
        response_id, "019aaaaa-0000-7000-8000-000000000000",
        "response body example id should be the redacted stable UUID"
    );

    let response_created_at = response_example
        .get("created_at")
        .and_then(serde_json::Value::as_str)
        .expect("response example should have created_at");

    assert_eq!(
        response_created_at, "2024-01-01T00:00:00Z",
        "response body example created_at should be the redacted stable timestamp"
    );

    info!("Full request and response redaction test passed successfully");

    Ok(())
}

/// Tests that non-redacted request bodies still have examples from `json()`.
///
/// This test establishes the baseline behavior: regular `json()` calls
/// produce examples (from the serialized value), while `json_redacted()`
/// with explicit redactions produces the redacted example.
#[rstest]
#[tokio::test]
async fn test_request_body_example_from_regular_json(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    info!("Testing that regular json() calls produce request body examples");

    let new_observation = PartialObservation {
        name: "Regular JSON Test Bird".to_string(),
        position: LngLat { lng: 5.0, lat: 6.0 },
        color: Some("green".to_string()),
        notes: Some("Regular notes here".to_string()),
    };

    // Create observation with regular json() (no redaction)
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

    // Find the request body - it's already a RequestBody, not RefOr
    let request_body = post_operation
        .request_body
        .as_ref()
        .expect("should have request body");

    // Get the application/json content
    let json_content = request_body
        .content
        .get("application/json")
        .expect("should have application/json content");

    // Regular json() calls should produce an example with the actual values
    let example = json_content
        .example
        .as_ref()
        .expect("regular json() should produce an example");

    // The example should have the original notes (not redacted)
    let example_notes = example.get("notes").and_then(serde_json::Value::as_str);

    assert_eq!(
        example_notes,
        Some("Regular notes here"),
        "regular json() example should have the original notes"
    );

    info!("Regular JSON request body example test passed successfully");

    Ok(())
}
