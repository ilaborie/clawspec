#![allow(missing_docs)]

use clawspec_core::ApiClientError;
use rstest::rstest;
use tracing::info;

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn test_unexpected_status_code_error(#[future] app: TestApp) -> anyhow::Result<()> {
    let app = app.await;

    info!("Testing UnexpectedStatusCode error path");

    // Test 1: Expect 200 but get 404 from nonexistent endpoint
    let result = app
        .get("/nonexistent-endpoint")?
        .set_expected_status(200)
        .exchange()
        .await;

    match result {
        Err(ApiClientError::UnexpectedStatusCode { status_code, body }) => {
            assert_eq!(status_code, 404, "Expected 404 status code");
            // Note: 404 responses may have empty bodies, which is valid
            info!(
                "✓ Successfully caught UnexpectedStatusCode error: {} - {}",
                status_code, body
            );
        }
        Err(error) => anyhow::bail!(
            "Expected ApiClientError::UnexpectedStatusCode, got: {:?}",
            error
        ),
        Ok(_) => anyhow::bail!("Expected UnexpectedStatusCode error, but request succeeded"),
    }

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_sucessful_endpoint_bad_status(#[future] app: TestApp) -> anyhow::Result<()> {
    let app = app.await;

    let result = app
        .get("/observations")?
        .set_expected_status(201)
        .exchange()
        .await;

    match result {
        Err(ApiClientError::UnexpectedStatusCode { status_code, body }) => {
            assert_eq!(status_code, 200, "Expected 200 status code");
            info!(
                "✓ Successfully caught UnexpectedStatusCode error: {} - {}",
                status_code, body
            );
        }
        Err(error) => anyhow::bail!(
            "Expected ApiClientError::UnexpectedStatusCode, got: {:?}",
            error
        ),
        Ok(_) => anyhow::bail!("Expected UnexpectedStatusCode error, but request succeeded"),
    }

    Ok(())
}
