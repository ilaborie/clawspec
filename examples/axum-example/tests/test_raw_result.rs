#![allow(missing_docs)]

use rstest::rstest;
use tracing::info;

use axum_example::observations::domain::{LngLat, PartialObservation};

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn test_raw_result_api(#[future] app: TestApp) -> anyhow::Result<()> {
    use axum::http::StatusCode;
    use clawspec_core::RawBody;

    let app = app.await;

    info!("Testing RawResult API with different response types");

    // Test 1: JSON response
    let test_observation = PartialObservation {
        name: "Raw Result Test Bird".to_string(),
        position: LngLat {
            lng: 10.0,
            lat: 20.0,
        },
        color: Some("red".to_string()),
        notes: Some("Testing raw result API".to_string()),
    };

    let create_result = app
        .post("/observations")?
        .json(&test_observation)?
        .await?
        .as_raw()
        .await?;

    // Verify we get complete response information
    assert_eq!(create_result.status_code(), StatusCode::CREATED);
    assert!(create_result.content_type().is_some());
    match create_result.body() {
        RawBody::Text(text) => {
            info!("Created observation ID from raw result: {}", text);
            // Should be a valid JSON number
            let _id: u32 = text.parse()?;
        }
        RawBody::Binary(_) | RawBody::Empty => {
            anyhow::bail!("Expected text body for JSON response")
        }
    }

    // Test 2: List response (JSON array)
    let list_result = app.get("/observations")?.await?.as_raw().await?;

    assert_eq!(list_result.status_code(), StatusCode::OK);
    if let Some(content_type) = list_result.content_type() {
        assert_eq!(content_type.to_string(), "application/json");
    }

    match list_result.body() {
        RawBody::Text(json_text) => {
            info!("Raw JSON response: {}", json_text);
            // Should be valid JSON array
            let _observations: serde_json::Value = serde_json::from_str(json_text)?;
        }
        RawBody::Binary(_) | RawBody::Empty => {
            anyhow::bail!("Expected text body for JSON list response")
        }
    }

    // Test 3: Health check (JSON response)
    let health_result = app.get("/health")?.await?.as_raw().await?;

    assert_eq!(health_result.status_code(), StatusCode::OK);
    match health_result.body() {
        RawBody::Text(text) => {
            info!("Health check raw result: {}", text);
            // Health endpoint returns JSON with status and uptime
            assert!(text.contains("\"status\":\"OK\""));
        }
        RawBody::Empty => {
            info!("Health check returned empty body");
        }
        RawBody::Binary(_) => anyhow::bail!("Expected text or empty body for health check"),
    }

    // Test 4: Binary/raw data with error response
    let error_result = app
        .post("/observations")?
        .raw(
            b"Invalid binary data".to_vec(),
            "application/octet-stream".parse()?,
        )
        .await?
        .as_raw()
        .await?;

    assert_eq!(
        error_result.status_code(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    match error_result.body() {
        RawBody::Text(error_text) => {
            info!("Error response from raw result: {}", error_text);
            // Should contain error information
            assert!(error_text.contains("415") || error_text.contains("Unsupported"));
        }
        RawBody::Binary(_) | RawBody::Empty => {
            anyhow::bail!("Expected text body for error response")
        }
    }

    Ok(())
}
