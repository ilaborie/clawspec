//! Example demonstrating how to split an `OpenAPI` specification into multiple files.
//!
//! This test shows how to use the `split` module to organize schemas into separate files
//! for better modularity and reusability.

use std::fs;

use anyhow::Context;
use clawspec_core::register_schemas;
use clawspec_core::split::{ExtractSchemasByPredicate, OpenApiSplitExt, SplitSchemasByTag};

use axum_example::extractors::ExtractorError;
use axum_example::observations::domain::{
    LngLat, Observation, PartialObservation, PatchObservation,
};
use axum_example::observations::{FlatObservation, ImportResponse, UploadResponse};

mod common;
pub use self::common::*;

/// Demonstrates splitting an `OpenAPI` spec by extracting error schemas into a separate file.
///
/// This is useful when you want to share error types across multiple API specifications
/// or keep error definitions separate from domain schemas.
#[tokio::test]
async fn test_split_by_predicate_extracts_error_schemas() -> anyhow::Result<()> {
    init_tracing();
    let mut app = TestApp::start().await?;

    // Register schemas
    register_schemas!(
        app,
        ExtractorError,
        FlatObservation,
        PartialObservation,
        PatchObservation,
        LngLat,
        ImportResponse,
        UploadResponse
    )
    .await;

    // Make some API calls to populate the spec
    app.list_observations(None).await?;

    // Get the collected OpenAPI spec
    let spec = app.collected_openapi().await;

    // Split: extract all schemas containing "Error" into errors.yaml
    let splitter = ExtractSchemasByPredicate::new("errors.yaml", |name| name.contains("Error"));
    let result = spec.split_with(splitter);

    // Verify the split
    assert!(
        !result.is_unsplit(),
        "Should have extracted at least one error schema"
    );
    assert_eq!(result.fragment_count(), 1);

    let fragment = result.fragments.first().expect("should have one fragment");
    assert_eq!(fragment.path.to_string_lossy(), "errors.yaml");

    // The fragment should contain ExtractorError
    assert!(
        fragment.content.schemas.contains_key("ExtractorError"),
        "ExtractorError should be in the fragment"
    );

    // The main spec should have a $ref to the external file
    let main_components = result
        .main
        .components
        .as_ref()
        .expect("should have components");

    if let Some(utoipa::openapi::RefOr::Ref(ref_entry)) =
        main_components.schemas.get("ExtractorError")
    {
        assert!(
            ref_entry.ref_location.contains("errors.yaml"),
            "Should reference external file"
        );
    } else {
        panic!("ExtractorError should be an external reference in main spec");
    }

    // Demonstrate serialization using utoipa's yaml feature
    let main_yaml = result.main.to_yaml()?;
    let errors_yaml = serde_json::to_string_pretty(&fragment.content)?;

    assert!(main_yaml.contains("$ref"));
    assert!(errors_yaml.contains("ExtractorError"));

    Ok(())
}

/// Demonstrates splitting schemas by tag - schemas used by multiple tags go to common file.
#[tokio::test]
async fn test_split_by_tag_organizes_schemas() -> anyhow::Result<()> {
    init_tracing();
    let mut app = TestApp::start().await?;

    // Register schemas
    register_schemas!(
        app,
        ExtractorError,
        Observation,
        FlatObservation,
        PartialObservation,
        PatchObservation,
        LngLat,
        ImportResponse,
        UploadResponse
    )
    .await;

    // Make calls with different tags
    app.list_observations(None).await?;

    // Get the collected OpenAPI spec
    let spec = app.collected_openapi().await;

    // Analyze schema usage by tag
    let splitter = SplitSchemasByTag::new("common-types.yaml");

    // Note: SplitSchemasByTag analyzes which schemas are used by which operation tags
    // - Schemas used by only one tag -> {tag}.yaml
    // - Schemas used by multiple tags -> common-types.yaml
    let result = spec.split_with(splitter);

    // The result depends on how the schemas are actually used in operations
    // For this demo, we just verify the structure is correct
    if !result.is_unsplit() {
        for fragment in &result.fragments {
            let json = serde_json::to_string(&fragment.content)?;
            assert!(!json.is_empty());
        }
    }

    Ok(())
}

/// Demonstrates a real-world use case: generating split `OpenAPI` files.
#[tokio::test]
async fn test_write_split_openapi_files() -> anyhow::Result<()> {
    init_tracing();
    let mut app = TestApp::start().await?;

    // Register schemas
    register_schemas!(
        app,
        ExtractorError,
        FlatObservation,
        PartialObservation,
        PatchObservation,
        LngLat,
        ImportResponse,
        UploadResponse
    )
    .await;

    app.list_observations(None).await?;

    let spec = app.collected_openapi().await;

    // Extract response/error schemas into a separate file
    let splitter =
        ExtractSchemasByPredicate::new("schemas/responses.yaml", |name| name.contains("Response"));
    let result = spec.split_with(splitter);

    // Create output directory
    let output_dir = std::path::Path::new("./doc/split");
    fs::create_dir_all(output_dir).context("creating output directory")?;

    // Write fragment files as JSON (could also use yaml if serde_yaml is added)
    for fragment in &result.fragments {
        let fragment_path = output_dir.join(&fragment.path);

        // Create parent directories if needed
        if let Some(parent) = fragment_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&fragment.content)?;
        fs::write(&fragment_path, json).context("writing fragment file")?;
    }

    // Write main spec
    let main_yaml = result.main.to_yaml()?;
    fs::write(output_dir.join("openapi.yaml"), main_yaml).context("writing main openapi file")?;

    // Verify files were created
    assert!(output_dir.join("openapi.yaml").exists());

    // Cleanup
    fs::remove_dir_all(output_dir).ok();

    Ok(())
}
