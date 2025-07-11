#![allow(missing_docs)]

use anyhow::Context;
use axum_example::extractors::ExtractorError;
use axum_example::observations::domain::{LngLat, PartialObservation, PatchObservation};
use axum_example::observations::{FlatObservation, ImportResponse, ListOption, UploadResponse};
use clawspec_utoipa::{CallHeaders, register_schemas};
use headers::ContentType;
use rstest::rstest;
use tracing::info;

mod common;
pub use self::common::*;

#[rstest]
#[tokio::test]
async fn should_generate_openapi(#[future] app: TestApp) -> anyhow::Result<()> {
    let mut app = app.await;

    // Register missing schemas manually to fix the missing schema issues
    register_schemas!(
        app,
        ExtractorError,
        FlatObservation,
        PartialObservation,
        PatchObservation,
        LngLat,
        ImportResponse,
        UploadResponse
    );

    basic_crud(&mut app).await?;
    alternate_content_types(&mut app).await?;
    test_error_cases(&mut app).await?;

    app.write_openapi("./doc/openapi.yml")
        .await
        .context("writing openapi file")?;

    Ok(())
}

async fn basic_crud(app: &mut TestApp) -> anyhow::Result<()> {
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

    info!("List observations with headers to demonstrate header parameter collection");
    let _list_with_headers = app
        .list_observations_with_headers(
            Some(ListOption {
                offset: 0,
                limit: 10,
            }),
            12345,
            "req-abc-123-def",
        )
        .await
        .context("should list observations with custom headers")?;

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

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn alternate_content_types(app: &mut TestApp) -> anyhow::Result<()> {
    // Test 1: Create observation with JSON (existing format)
    let json_observation = PartialObservation {
        name: "JSON Bird".to_string(),
        position: LngLat { lng: 1.0, lat: 2.0 },
        color: Some("blue".to_string()),
        notes: Some("Created via JSON".to_string()),
    };
    let _json_result = app
        .post("/observations")?
        .json(&json_observation)?
        .exchange()
        .await
        .context("should create observation via JSON")?;

    // Test 2: Create observation with form-encoded data
    // Note: Form encoding doesn't support nested objects, so we create a flattened version

    let flat_observation = FlatObservation {
        name: "Form Bird".to_string(),
        position: LngLat { lng: 2.5, lat: 3.5 },
        color: Some("orange".to_string()),
        notes: Some("Created via form encoding".to_string()),
    };

    let _form_result = app
        .post("/observations")?
        .form(&flat_observation)?
        .add_expected_status(400) // Form validation may fail
        .exchange()
        .await
        .context("should create observation via form encoding")?;

    // Test 3: Create observation with XML data
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<PartialObservation>
    <name>XML Bird</name>
    <position>
        <lng>3.5</lng>
        <lat>4.5</lat>
    </position>
    <color>red</color>
    <notes>Created via XML</notes>
</PartialObservation>"#;
    let _xml_result = app
        .post("/observations")?
        .raw(xml_data.as_bytes().to_vec(), ContentType::xml())
        .exchange()
        .await
        .context("should create observation via XML")?;

    // Test 4: Import multiple observations using streaming JSON
    let import_data = vec![
        PartialObservation {
            name: "Bulk Bird 1".to_string(),
            position: LngLat {
                lng: 10.0,
                lat: 20.0,
            },
            color: Some("green".to_string()),
            notes: None,
        },
        PartialObservation {
            name: "Bulk Bird 2".to_string(),
            position: LngLat {
                lng: 11.0,
                lat: 21.0,
            },
            color: Some("yellow".to_string()),
            notes: Some("Imported in bulk".to_string()),
        },
    ];

    // Serialize as newline-delimited JSON (NDJSON)
    let mut ndjson_data = Vec::new();
    for observation in &import_data {
        let json_line = serde_json::to_vec(observation).context("should serialize observation")?;
        ndjson_data.extend(json_line);
        ndjson_data.push(b'\n');
    }

    let import_result = app
        .post("/observations/import")?
        .raw(ndjson_data, ContentType::octet_stream())
        .exchange()
        .await
        .context("should import observations via streaming JSON")?
        .as_json::<ImportResponse>()
        .await
        .context("should deserialize import response")?;

    info!(
        "Import result: imported {} observations",
        import_result.imported
    );

    // Test 5: Upload observations using multipart/form-data
    let multipart_data = vec![
        (
            "observation1",
            r#"{"name":"Multipart Bird 1","position":{"lng":15.0,"lat":25.0},"color":"purple","notes":"Uploaded via multipart"}"#,
        ),
        (
            "observation2",
            r#"{"name":"Multipart Bird 2","position":{"lng":16.0,"lat":26.0},"color":"pink","notes":"Another multipart upload"}"#,
        ),
    ];

    let upload_result = app
        .post("/observations/upload")?
        .multipart(multipart_data)
        .exchange()
        .await
        .context("should upload observations via multipart")?
        .as_json::<UploadResponse>()
        .await
        .context("should deserialize upload response")?;

    info!(
        "Upload result: uploaded {} observations",
        upload_result.uploaded
    );

    Ok(())
}

async fn test_error_cases(app: &mut TestApp) -> anyhow::Result<()> {
    // Test 1: Unsupported media type error - should capture error response in OpenAPI
    let unsupported_error = app
        .post("/observations")?
        .raw(b"Some PDF content".to_vec(), "application/pdf".parse()?)
        .exchange()
        .await?
        .as_json::<TestClientError>()
        .await?;
    assert_eq!(unsupported_error.status, 415);

    // Test 2: Invalid JSON error - should capture error response in OpenAPI
    let _invalid_json_result = app
        .post("/observations")?
        .raw(b"{ invalid json }".to_vec(), ContentType::json())
        .exchange()
        .await?
        .as_json::<TestClientError>()
        .await?;

    let headers = CallHeaders::new()
        .add_header("X-Test-Case", "error-scenario")
        .add_header("X-Expected-Status", "400");

    // Test 3: Test error with custom headers to show how headers work with error responses
    let _error_with_headers_result = app
        .post("/observations")?
        .with_headers(headers)
        .raw(
            b"definitely not valid json or xml".to_vec(),
            ContentType::json(),
        )
        .exchange()
        .await?
        .as_json::<TestClientError>()
        .await?;

    Ok(())
}
