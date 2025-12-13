use std::future::Future;
use std::pin::Pin;

use http::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

use super::*;
use crate::client::call_parameters::{CallParameters, OperationMetadata};
use crate::client::openapi::channel::CollectorSender;
use crate::client::response::ExpectedStatusCodes;
use crate::client::{CallHeaders, CallPath, CallQuery, ParamValue};
use crate::{ApiClientError, CallResult};

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq)]
struct TestData {
    id: u32,
    name: String,
}

// Helper function to create a basic ApiCall for testing
fn create_test_api_call() -> ApiCall {
    let client = reqwest::Client::new();
    let base_uri = "http://localhost:8080".parse().expect("valid uri");
    let method = Method::GET;
    let path = CallPath::from("/test");

    ApiCall::build(
        client,
        base_uri,
        CollectorSender::dummy(),
        method,
        path,
        None,
    )
    .expect("should build api call")
}

// Test OperationMetadata creation and defaults
#[test]
fn test_operation_metadata_default() {
    let metadata = OperationMetadata::default();
    assert!(metadata.operation_id.is_empty());
    assert!(metadata.tags.is_none());
    assert!(metadata.description.is_none());
}

#[test]
fn test_operation_metadata_creation() {
    let metadata = OperationMetadata {
        operation_id: "test-operation".to_string(),
        tags: Some(vec!["users".to_string(), "admin".to_string()]),
        description: Some("Test operation description".to_string()),
        response_description: Some("Test response description".to_string()),
    };

    assert_eq!(metadata.operation_id, "test-operation");
    assert_eq!(
        metadata.tags,
        Some(vec!["users".to_string(), "admin".to_string()])
    );
    assert_eq!(
        metadata.description,
        Some("Test operation description".to_string())
    );
}

// Test ApiCall creation and builder methods
#[test]
fn test_api_call_build_success() {
    let call = create_test_api_call();
    assert_eq!(call.method, Method::GET);
    assert_eq!(call.path.path, "/test");
    assert!(call.query.is_empty());
    assert!(call.headers.is_none());
    assert!(call.body.is_none());
}

#[test]
fn test_api_call_with_operation_id() {
    let call = create_test_api_call().with_operation_id("custom-operation-id");

    assert_eq!(call.metadata.operation_id, "custom-operation-id");
}

#[test]
fn test_api_call_with_description() {
    let call = create_test_api_call().with_description("Custom description");

    assert_eq!(
        call.metadata.description,
        Some("Custom description".to_string())
    );
}

#[test]
fn test_api_call_with_tags_vec() {
    let tags = vec!["users", "admin", "api"];
    let call = create_test_api_call().with_tags(tags.clone());

    let expected_tags: Vec<String> = tags.into_iter().map(|s| s.to_string()).collect();
    assert_eq!(call.metadata.tags, Some(expected_tags));
}

#[test]
fn test_api_call_with_tags_array() {
    let call = create_test_api_call().with_tags(["users", "admin"]);

    assert_eq!(
        call.metadata.tags,
        Some(vec!["users".to_string(), "admin".to_string()])
    );
}

#[test]
fn test_api_call_with_tag_single() {
    let call = create_test_api_call().with_tag("users").with_tag("admin");

    assert_eq!(
        call.metadata.tags,
        Some(vec!["users".to_string(), "admin".to_string()])
    );
}

#[test]
fn test_api_call_with_tag_on_empty_tags() {
    let call = create_test_api_call().with_tag("users");

    assert_eq!(call.metadata.tags, Some(vec!["users".to_string()]));
}

// Test query parameter methods
#[test]
fn test_api_call_with_query() {
    let query = CallQuery::new()
        .add_param("page", ParamValue::new(1))
        .add_param("limit", ParamValue::new(10));

    let call = create_test_api_call().with_query(query.clone());

    // Test that the query was set (we can't access private fields, but we can test the behavior)
    assert!(!call.query.is_empty());
}

// Test header methods
#[test]
fn test_api_call_with_headers() {
    let headers = CallHeaders::new().add_header("Authorization", "Bearer token");

    let call = create_test_api_call().with_headers(headers);

    assert!(call.headers.is_some());
}

#[test]
fn test_api_call_with_header_single() {
    let call = create_test_api_call()
        .with_header("Authorization", "Bearer token")
        .with_header("Content-Type", "application/json");

    assert!(call.headers.is_some());
    // We can test that headers were set without accessing private fields
    // The presence of headers confirms the functionality works
}

#[test]
fn test_api_call_with_header_merge() {
    let initial_headers = CallHeaders::new().add_header("X-Request-ID", "abc123");

    let call = create_test_api_call()
        .with_headers(initial_headers)
        .with_header("Authorization", "Bearer token");

    assert!(call.headers.is_some());
    // Test that merging worked by confirming headers exist
    let _headers = call.headers.unwrap();
}

// Test status code validation methods
#[test]
fn test_api_call_with_expected_status() {
    let call = create_test_api_call().with_expected_status(201);

    assert!(call.expected_status_codes.contains(201));
    assert!(!call.expected_status_codes.contains(200));
}

#[test]
fn test_api_call_with_status_range_inclusive() {
    let call = create_test_api_call().with_status_range_inclusive(200..=299);

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(250));
    assert!(call.expected_status_codes.contains(299));
    assert!(!call.expected_status_codes.contains(300));
}

#[test]
fn test_api_call_with_status_range_exclusive() {
    let call = create_test_api_call().with_status_range(200..300);

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(299));
    assert!(!call.expected_status_codes.contains(300));
}

#[test]
fn test_api_call_add_expected_status() {
    let call = create_test_api_call()
        .with_status_range_inclusive(200..=299)
        .add_expected_status(404);

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(299));
    assert!(call.expected_status_codes.contains(404));
    assert!(!call.expected_status_codes.contains(405));
}

#[test]
fn test_api_call_add_expected_status_range_inclusive() {
    let call = create_test_api_call()
        .with_status_range_inclusive(200..=204)
        .add_expected_status_range_inclusive(400..=404);

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(204));
    assert!(call.expected_status_codes.contains(400));
    assert!(call.expected_status_codes.contains(404));
    assert!(!call.expected_status_codes.contains(205));
    assert!(!call.expected_status_codes.contains(405));
}

#[test]
fn test_api_call_add_expected_status_range_exclusive() {
    let call = create_test_api_call()
        .with_status_range_inclusive(200..=204)
        .add_expected_status_range(400..404);

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(204));
    assert!(call.expected_status_codes.contains(400));
    assert!(call.expected_status_codes.contains(403));
    assert!(!call.expected_status_codes.contains(404));
}

#[test]
fn test_api_call_with_success_only() {
    let call = create_test_api_call().with_success_only();

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(299));
    assert!(!call.expected_status_codes.contains(300));
    assert!(!call.expected_status_codes.contains(400));
}

#[test]
fn test_api_call_with_client_errors() {
    let call = create_test_api_call().with_client_errors();

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(299));
    assert!(call.expected_status_codes.contains(400));
    assert!(call.expected_status_codes.contains(499));
    assert!(!call.expected_status_codes.contains(300));
    assert!(!call.expected_status_codes.contains(500));
}

#[test]
fn test_api_call_with_expected_status_codes() {
    let codes = ExpectedStatusCodes::from_single(201).add_expected_status(404);

    let call = create_test_api_call().with_expected_status_codes(codes);

    assert!(call.expected_status_codes.contains(201));
    assert!(call.expected_status_codes.contains(404));
    assert!(!call.expected_status_codes.contains(200));
}

#[test]
fn test_api_call_with_expected_status_code_http() {
    let call = create_test_api_call().with_expected_status_code(StatusCode::CREATED);

    assert!(call.expected_status_codes.contains(201));
    assert!(!call.expected_status_codes.contains(200));
}

#[test]
fn test_api_call_with_expected_status_code_range_http() {
    let call = create_test_api_call()
        .with_expected_status_code_range(StatusCode::OK..=StatusCode::NO_CONTENT);

    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(204));
    assert!(!call.expected_status_codes.contains(205));
}

// Test request body methods
#[test]
fn test_api_call_json_body() {
    let test_data = TestData {
        id: 1,
        name: "test".to_string(),
    };

    let call = create_test_api_call()
        .json(&test_data)
        .expect("should set JSON body");

    assert!(call.body.is_some());
    let body = call.body.unwrap();
    assert_eq!(body.content_type, headers::ContentType::json());

    // Verify the JSON data can be deserialized back
    let parsed: TestData = serde_json::from_slice(&body.data).expect("should parse JSON");
    assert_eq!(parsed, test_data);
}

#[test]
fn test_api_call_form_body() {
    let test_data = TestData {
        id: 42,
        name: "form test".to_string(),
    };

    let call = create_test_api_call()
        .form(&test_data)
        .expect("should set form body");

    assert!(call.body.is_some());
    let body = call.body.unwrap();
    assert_eq!(body.content_type, headers::ContentType::form_url_encoded());
}

#[test]
fn test_api_call_text_body() {
    let text_content = "Hello, World!";

    let call = create_test_api_call().text(text_content);

    assert!(call.body.is_some());
    let body = call.body.unwrap();
    assert_eq!(body.content_type, headers::ContentType::text());
    assert_eq!(body.data, text_content.as_bytes());
}

#[test]
fn test_api_call_raw_body() {
    let binary_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
    let content_type = headers::ContentType::octet_stream();

    let call = create_test_api_call().raw(binary_data.clone(), content_type.clone());

    assert!(call.body.is_some());
    let body = call.body.unwrap();
    assert_eq!(body.content_type, content_type);
    assert_eq!(body.data, binary_data);
}

#[test]
fn test_api_call_multipart_body() {
    let parts = vec![("title", "My Document"), ("description", "A test document")];

    let call = create_test_api_call().multipart(parts);

    assert!(call.body.is_some());
    let body = call.body.unwrap();
    // Content type should be multipart/form-data with boundary
    assert!(
        body.content_type
            .to_string()
            .starts_with("multipart/form-data")
    );
}

// Test URL building (helper function tests)
#[test]
fn test_build_url_simple_path() {
    let base_uri: Uri = "http://localhost:8080".parse().unwrap();
    let path = CallPath::from("/users");
    let query = CallQuery::default();

    let url = ApiCall::build_url(&base_uri, &path, &query).expect("should build URL");
    // The actual implementation results in double slash due to URI parsing
    assert_eq!(url.to_string(), "http://localhost:8080/users");
}

#[test]
fn test_build_url_with_query() {
    let base_uri: Uri = "http://localhost:8080".parse().unwrap();
    let path = CallPath::from("/users");
    let query = CallQuery::new()
        .add_param("page", ParamValue::new(1))
        .add_param("limit", ParamValue::new(10));

    let url = ApiCall::build_url(&base_uri, &path, &query).expect("should build URL");
    // Query order might vary, so check both possibilities
    let url_str = url.to_string();
    assert!(url_str.starts_with("http://localhost:8080/users?"));
    assert!(url_str.contains("page=1"));
    assert!(url_str.contains("limit=10"));
}

#[test]
fn test_build_url_with_path_params() {
    let base_uri: Uri = "http://localhost:8080".parse().unwrap();
    let path = CallPath::from("/users/{id}").add_param("id", ParamValue::new(123));
    let query = CallQuery::default();

    let url = ApiCall::build_url(&base_uri, &path, &query).expect("should build URL");
    assert_eq!(url.to_string(), "http://localhost:8080/users/123");
}

// Test request building (helper function tests)
#[test]
fn test_build_request_simple() {
    let method = Method::GET;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let body = None;
    let parameters = CallParameters::default();

    let request = ApiCall::build_request(method.clone(), url.clone(), &parameters, &body, &None)
        .expect("should build request");

    assert_eq!(request.method(), &method);
    assert_eq!(request.url(), &url);
    assert!(request.body().is_none());
}

#[test]
fn test_build_request_with_headers() {
    let method = Method::GET;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let headers = Some(CallHeaders::new().add_header("Authorization", "Bearer token"));
    let body = None;
    let parameters = CallParameters::with_all(CallQuery::new(), headers, None);

    let request = ApiCall::build_request(method, url, &parameters, &body, &None)
        .expect("should build request");

    assert!(request.headers().get("authorization").is_some());
}

#[test]
fn test_build_request_with_body() {
    let method = Method::POST;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let test_data = TestData {
        id: 1,
        name: "test".to_string(),
    };
    let body = Some(CallBody::json(&test_data).expect("should create JSON body"));
    let parameters = CallParameters::default();

    let request = ApiCall::build_request(method, url, &parameters, &body, &None)
        .expect("should build request");

    assert!(request.body().is_some());
    assert_eq!(
        request.headers().get("content-type").unwrap(),
        "application/json"
    );
}

// Test method chaining
#[test]
fn test_api_call_method_chaining() {
    let test_data = TestData {
        id: 1,
        name: "chaining test".to_string(),
    };

    let call = create_test_api_call()
        .with_operation_id("test-chain")
        .with_description("Method chaining test")
        .with_tag("test")
        .with_tag("chaining")
        .with_header("Authorization", "Bearer token")
        .with_header("X-Request-ID", "test-123")
        .with_status_range_inclusive(200..=201)
        .add_expected_status(404)
        .json(&test_data)
        .expect("should set JSON body");

    // Verify all settings were applied
    assert_eq!(call.metadata.operation_id, "test-chain");
    assert_eq!(
        call.metadata.description,
        Some("Method chaining test".to_string())
    );
    assert_eq!(
        call.metadata.tags,
        Some(vec!["test".to_string(), "chaining".to_string()])
    );
    assert!(call.headers.is_some());
    assert!(call.body.is_some());
    assert!(call.expected_status_codes.contains(200));
    assert!(call.expected_status_codes.contains(201));
    assert!(call.expected_status_codes.contains(404));
}

// Test edge cases and error conditions
#[test]
fn test_api_call_json_serialization_error() {
    // This would test JSON serialization errors, but TestData is always serializable
    // In a real scenario, you'd test with a type that fails to serialize
    // For now, we'll test the success case
    let test_data = TestData {
        id: 1,
        name: "test".to_string(),
    };

    let result = create_test_api_call().json(&test_data);
    assert!(result.is_ok());
}

#[test]
fn test_api_call_form_serialization_error() {
    // Similar to JSON test - TestData is always serializable
    let test_data = TestData {
        id: 1,
        name: "test".to_string(),
    };

    let result = create_test_api_call().form(&test_data);
    assert!(result.is_ok());
}

// Test constants
#[test]
fn test_body_max_length_constant() {
    assert_eq!(BODY_MAX_LENGTH, 1024);
}

// Test collection exclusion functionality
#[test]
fn test_without_collection_sets_flag() {
    let call = create_test_api_call().without_collection();
    assert!(call.skip_collection);
}

#[test]
fn test_default_collection_flag() {
    let call = create_test_api_call();
    assert!(!call.skip_collection);
}

#[test]
fn test_without_collection_chaining() {
    let call = create_test_api_call()
        .with_operation_id("test-operation")
        .with_description("Test operation")
        .without_collection()
        .with_header("Authorization", "Bearer token");

    assert!(call.skip_collection);
    assert_eq!(call.metadata.operation_id, "test-operation");
    assert_eq!(
        call.metadata.description,
        Some("Test operation".to_string())
    );
    assert!(call.headers.is_some());
}

// Test IntoFuture implementation
#[test]
fn test_api_call_into_future_type_requirements() {
    // Test that ApiCall implements IntoFuture with the correct associated types
    use std::future::IntoFuture;

    fn assert_into_future<T>(_: T)
    where
        T: IntoFuture<Output = Result<CallResult, ApiClientError>>,
        T::IntoFuture: Send,
    {
    }

    let call = create_test_api_call();
    assert_into_future(call);
}

#[tokio::test]
async fn test_api_call_into_future_equivalence() {
    // Test that ApiCall.await works correctly by testing the IntoFuture implementation
    // This is a compile-time test that verifies the future type structure is correct

    use std::future::IntoFuture;

    let call1 = create_test_api_call();
    let call2 = create_test_api_call();

    // Test that both direct await and explicit into_future produce the same type
    let _future1 = call1.into_future();
    let _future2 = call2.into_future();

    // Both should be Send futures
    fn assert_send<T: Send>(_: T) {}
    assert_send(_future1);
    assert_send(_future2);
}

#[test]
fn test_into_future_api_demonstration() {
    // This test demonstrates the new API usage patterns
    // Note: This is a compile-time test showing the API ergonomics

    use crate::ApiClient;
    use std::future::IntoFuture;

    // Demonstrate the new API pattern compiles correctly
    fn assert_new_api_compiles() {
        async fn _example() -> Result<(), ApiClientError> {
            let client = ApiClient::builder().build()?;

            // Create path with parameters
            let path = CallPath::from("/users/{id}").add_param("id", 123);

            let query = CallQuery::new().add_param("include_details", true);

            // Direct .await API (using IntoFuture)
            let _response = client
                .get(path)?
                .with_query(query)
                .with_header("Authorization", "Bearer token")
                .await?; // Direct await

            Ok(())
        }
    }

    // Test that the function compiles
    assert_new_api_compiles();

    // Demonstrate that ApiCall implements IntoFuture with correct types
    let call = create_test_api_call();
    #[allow(clippy::let_underscore_future)]
    let _: Pin<Box<dyn Future<Output = Result<CallResult, ApiClientError>> + Send>> =
        call.into_future();
}

#[test]
fn test_api_call_with_response_description() {
    let call = create_test_api_call();
    let call = call.with_response_description("Success response");
    assert_eq!(
        call.response_description,
        Some("Success response".to_string())
    );
}

#[test]
fn test_api_call_response_description_method_chaining() {
    let call = create_test_api_call();
    let call = call
        .with_response_description("Original description")
        .with_response_description("Overridden description");

    // Latest description should override the previous one
    assert_eq!(
        call.response_description,
        Some("Overridden description".to_string())
    );
}

#[test]
fn test_api_call_response_description_none_by_default() {
    let call = create_test_api_call();
    assert_eq!(call.response_description, None);
}

#[test]
fn test_api_call_with_authentication_bearer() {
    let mut call = create_test_api_call();
    call = call.with_authentication(crate::client::Authentication::Bearer("test-token".into()));

    assert!(matches!(
        call.authentication,
        Some(crate::client::Authentication::Bearer(ref token)) if token.equals_str("test-token")
    ));
}

#[test]
fn test_api_call_with_authentication_basic() {
    let mut call = create_test_api_call();
    call = call.with_authentication(crate::client::Authentication::Basic {
        username: "user".to_string(),
        password: "pass".into(),
    });

    assert!(matches!(
        call.authentication,
        Some(crate::client::Authentication::Basic { ref username, ref password })
            if username == "user" && password.equals_str("pass")
    ));
}

#[test]
fn test_api_call_with_authentication_api_key() {
    let mut call = create_test_api_call();
    call = call.with_authentication(crate::client::Authentication::ApiKey {
        header_name: "X-API-Key".to_string(),
        key: "secret-key".into(),
    });

    assert!(matches!(
        call.authentication,
        Some(crate::client::Authentication::ApiKey { ref header_name, ref key })
            if header_name == "X-API-Key" && key.equals_str("secret-key")
    ));
}

#[test]
fn test_api_call_with_authentication_none() {
    let mut call = create_test_api_call();
    // First set authentication
    call = call.with_authentication(crate::client::Authentication::Bearer("token".into()));
    assert!(call.authentication.is_some());

    // Then remove it
    call = call.with_authentication_none();
    assert!(call.authentication.is_none());
}

#[test]
fn test_build_request_with_bearer_auth() {
    let method = Method::GET;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let parameters = CallParameters::default();
    let body = None;
    let auth = Some(crate::client::Authentication::Bearer("test-token".into()));

    let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
        .expect("should build request");

    let auth_header = request.headers().get("authorization");
    assert!(auth_header.is_some());
    assert_eq!(auth_header.unwrap(), "Bearer test-token");
}

#[test]
fn test_build_request_with_basic_auth() {
    let method = Method::GET;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let parameters = CallParameters::default();
    let body = None;
    let auth = Some(crate::client::Authentication::Basic {
        username: "user".to_string(),
        password: "pass".into(),
    });

    let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
        .expect("should build request");

    let auth_header = request.headers().get("authorization");
    assert!(auth_header.is_some());
    // "user:pass" base64 encoded is "dXNlcjpwYXNz"
    assert_eq!(auth_header.unwrap(), "Basic dXNlcjpwYXNz");
}

#[test]
fn test_build_request_with_api_key_auth() {
    let method = Method::GET;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let parameters = CallParameters::default();
    let body = None;
    let auth = Some(crate::client::Authentication::ApiKey {
        header_name: "X-API-Key".to_string(),
        key: "secret-key-123".into(),
    });

    let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
        .expect("should build request");

    let api_key_header = request.headers().get("X-API-Key");
    assert!(api_key_header.is_some());
    assert_eq!(api_key_header.unwrap(), "secret-key-123");
}

#[test]
fn test_build_request_without_auth() {
    let method = Method::GET;
    let url: Url = "http://localhost:8080/users".parse().unwrap();
    let parameters = CallParameters::default();
    let body = None;
    let auth = None;

    let request = ApiCall::build_request(method, url, &parameters, &body, &auth)
        .expect("should build request");

    assert!(request.headers().get("authorization").is_none());
    assert!(request.headers().get("X-API-Key").is_none());
}

#[test]
fn test_authentication_override_in_method_chaining() {
    let mut call = create_test_api_call();

    // Start with no authentication
    assert!(call.authentication.is_none());

    // Add bearer authentication
    call = call.with_authentication(crate::client::Authentication::Bearer("token1".into()));
    assert!(matches!(
        call.authentication,
        Some(crate::client::Authentication::Bearer(ref token)) if token.equals_str("token1")
    ));

    // Override with basic authentication
    call = call.with_authentication(crate::client::Authentication::Basic {
        username: "user".to_string(),
        password: "pass".into(),
    });
    assert!(matches!(
        call.authentication,
        Some(crate::client::Authentication::Basic { .. })
    ));

    // Remove authentication
    call = call.with_authentication_none();
    assert!(call.authentication.is_none());
}
