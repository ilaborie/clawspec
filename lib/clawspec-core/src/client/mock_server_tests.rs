//! Mock server integration tests for comprehensive coverage.
//!
//! These tests exercise code paths in execution.rs, result.rs, operation.rs,
//! channel.rs, mod.rs, and security.rs using wiremock mock servers.

use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::{
    ApiClient, ApiClientError, ApiKeyLocation, CallPath, ExpectedStatusCodes, SecurityRequirement,
    SecurityScheme,
};

/// Test user type for JSON responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
struct User {
    id: u32,
    name: String,
    email: String,
}

/// Test error response type.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
struct ApiError {
    code: String,
    message: String,
}

/// Test request body type.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct CreateUserRequest {
    name: String,
    email: String,
}

/// Helper to create an ApiClient pointing to a mock server.
async fn client_for_mock(mock_server: &MockServer) -> ApiClient {
    let uri: http::Uri = mock_server.uri().parse().expect("valid URI");
    ApiClient::builder()
        .with_host(uri.host().expect("should have host"))
        .with_port(uri.port_u16().expect("should have port"))
        .build()
        .expect("should build client")
}

/// Helper to create an ApiClient with security schemes pointing to a mock server.
async fn client_with_security(mock_server: &MockServer) -> ApiClient {
    let uri: http::Uri = mock_server.uri().parse().expect("valid URI");
    ApiClient::builder()
        .with_host(uri.host().expect("should have host"))
        .with_port(uri.port_u16().expect("should have port"))
        .with_security_scheme("bearerAuth", SecurityScheme::bearer())
        .with_security_scheme(
            "apiKey",
            SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header),
        )
        .with_default_security(SecurityRequirement::new("bearerAuth"))
        .build()
        .expect("should build client")
}

// =============================================================================
// Tests for execution.rs - HTTP request execution
// =============================================================================

mod execution_tests {
    use super::*;

    #[tokio::test]
    async fn should_handle_successful_json_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 1,
                "name": "John Doe",
                "email": "john@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let user: User = client
            .call(
                http::Method::GET,
                CallPath::from("/users/{id}").add_param("id", 1),
            )
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_json()
            .await
            .expect("should parse JSON");

        assert_eq!(user.id, 1);
        assert_eq!(user.name, "John Doe");
        assert_eq!(user.email, "john@example.com");
    }

    #[tokio::test]
    async fn should_handle_unexpected_status_code() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/999"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result = client
            .call(
                http::Method::GET,
                CallPath::from("/users/{id}").add_param("id", 999),
            )
            .expect("should create call")
            .await;

        assert!(result.is_err());
        match result {
            Err(ApiClientError::UnexpectedStatusCode { status_code, body }) => {
                assert_eq!(status_code, 500);
                assert_eq!(body, "Internal Server Error");
            }
            other => panic!("Expected UnexpectedStatusCode, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn should_handle_unexpected_status_with_large_body_truncation() {
        let mock_server = MockServer::start().await;

        // Create a response body larger than 1024 characters
        let large_body = "x".repeat(2000);

        Mock::given(method("GET"))
            .and(path("/large-error"))
            .respond_with(ResponseTemplate::new(500).set_body_string(large_body.clone()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result = client
            .get("/large-error")
            .expect("should create call")
            .await;

        assert!(result.is_err());
        match result {
            Err(ApiClientError::UnexpectedStatusCode { body, .. }) => {
                assert!(body.contains("(truncated)"));
                assert!(body.len() < large_body.len());
            }
            other => panic!("Expected UnexpectedStatusCode, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn should_handle_expected_status_codes() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/users"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": 42,
                "name": "New User",
                "email": "new@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let user: User = client
            .post("/users")
            .expect("should create call")
            .json(&CreateUserRequest {
                name: "New User".to_string(),
                email: "new@example.com".to_string(),
            })
            .expect("should set JSON body")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(201))
            .await
            .expect("request should succeed")
            .as_json()
            .await
            .expect("should parse JSON");

        assert_eq!(user.id, 42);
    }

    #[tokio::test]
    async fn should_handle_without_collection() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        // Make a call with without_collection to test that code path
        client
            .get("/health")
            .expect("should create call")
            .without_collection()
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify no paths were collected
        let openapi = client.collected_openapi().await;
        assert!(openapi.paths.paths.is_empty());
    }

    #[tokio::test]
    async fn should_handle_request_with_all_parameter_types() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/123"))
            .and(query_param("limit", "10"))
            .and(header("X-Request-Id", "req-001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": 1, "name": "User 1", "email": "user1@example.com"}
            ])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let _: Vec<User> = client
            .call(
                http::Method::GET,
                CallPath::from("/users/{id}").add_param("id", 123),
            )
            .expect("should create call")
            .with_header("X-Request-Id", "req-001")
            .with_query(crate::client::CallQuery::new().add_param("limit", 10))
            .await
            .expect("request should succeed")
            .as_json()
            .await
            .expect("should parse JSON");
    }

    #[tokio::test]
    async fn should_handle_request_with_cookies() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/protected"))
            .and(header("cookie", "session=abc123; theme=dark"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": "secret"})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        client
            .get("/protected")
            .expect("should create call")
            .with_cookie("session", "abc123")
            .with_cookie("theme", "dark")
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");
    }
}

// =============================================================================
// Tests for result.rs - Response processing
// =============================================================================

mod result_tests {
    use super::*;
    use crate::client::RawBody;

    #[tokio::test]
    async fn should_handle_as_json_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 1,
                "name": "Test User",
                "email": "test@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let user: User = client
            .get("/user")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_json()
            .await
            .expect("should parse JSON");

        assert_eq!(user.name, "Test User");
    }

    #[tokio::test]
    async fn should_handle_as_json_deserialization_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/bad-json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(json!({
                        "wrong_field": "value"
                    })),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<User, ApiClientError> = client
            .get("/bad-json")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_json()
            .await;

        assert!(result.is_err());
        match result {
            Err(ApiClientError::JsonError { path, .. }) => {
                // The path might be "." (root), empty, or contain a specific field name
                // depending on where the deserialization error occurred
                assert!(
                    path == "."
                        || path.is_empty()
                        || path.contains("id")
                        || path.contains("name")
                        || path.contains("email"),
                    "Expected path to be root, empty, or contain field name, got: {path}"
                );
            }
            other => panic!("Expected JsonError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn should_handle_as_optional_json_with_content() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 1,
                "name": "User 1",
                "email": "user1@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let user: Option<User> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", 1),
            )
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_optional_json()
            .await
            .expect("should parse JSON");

        assert!(user.is_some());
        assert_eq!(user.as_ref().map(|u| u.id), Some(1));
    }

    #[tokio::test]
    async fn should_handle_as_optional_json_with_204_no_content() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/empty"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let user: Option<User> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "empty"),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(204))
            .await
            .expect("request should succeed")
            .as_optional_json()
            .await
            .expect("should handle 204");

        assert!(user.is_none());
    }

    #[tokio::test]
    async fn should_handle_as_optional_json_with_404_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/notfound"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let user: Option<User> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "notfound"),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(404))
            .await
            .expect("request should succeed")
            .as_optional_json()
            .await
            .expect("should handle 404");

        assert!(user.is_none());
    }

    #[tokio::test]
    async fn should_handle_as_result_json_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/success"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 1,
                "name": "Success User",
                "email": "success@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<User, ApiError> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "success"),
            )
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_result_json()
            .await
            .expect("should parse response");

        assert!(result.is_ok());
        assert_eq!(result.expect("should be Ok").name, "Success User");
    }

    #[tokio::test]
    async fn should_handle_as_result_json_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/error"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "code": "INVALID_REQUEST",
                "message": "Invalid user ID"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<User, ApiError> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "error"),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(400))
            .await
            .expect("request should succeed")
            .as_result_json()
            .await
            .expect("should parse response");

        assert!(result.is_err());
        let err = result.expect_err("should be Err");
        assert_eq!(err.code, "INVALID_REQUEST");
        assert_eq!(err.message, "Invalid user ID");
    }

    #[tokio::test]
    async fn should_handle_as_result_option_json_with_none() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/missing"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<Option<User>, ApiError> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "missing"),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(404))
            .await
            .expect("request should succeed")
            .as_result_option_json()
            .await
            .expect("should parse response");

        assert!(result.is_ok());
        assert!(result.expect("should be Ok").is_none());
    }

    #[tokio::test]
    async fn should_handle_as_result_option_json_with_some() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/found"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 42,
                "name": "Found User",
                "email": "found@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<Option<User>, ApiError> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "found"),
            )
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_result_option_json()
            .await
            .expect("should parse response");

        assert!(result.is_ok());
        let user = result.expect("should be Ok").expect("should be Some");
        assert_eq!(user.id, 42);
    }

    #[tokio::test]
    async fn should_handle_as_result_option_json_with_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/forbidden"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "code": "FORBIDDEN",
                "message": "Access denied"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<Option<User>, ApiError> = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", "forbidden"),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(403))
            .await
            .expect("request should succeed")
            .as_result_option_json()
            .await
            .expect("should parse response");

        assert!(result.is_err());
        let err = result.expect_err("should be Err");
        assert_eq!(err.code, "FORBIDDEN");
    }

    #[tokio::test]
    async fn should_handle_as_text() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/text"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Hello, World!"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let mut response = client
            .get("/text")
            .expect("should create call")
            .await
            .expect("request should succeed");

        let text = response.as_text().await.expect("should get text");

        assert_eq!(text, "Hello, World!");
    }

    #[tokio::test]
    async fn should_handle_as_bytes() {
        let mock_server = MockServer::start().await;
        let binary_data = vec![0x00, 0x01, 0x02, 0x03, 0xFF];

        Mock::given(method("GET"))
            .and(path("/binary"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .set_body_bytes(binary_data.clone()),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let mut response = client
            .get("/binary")
            .expect("should create call")
            .await
            .expect("request should succeed");

        let bytes = response.as_bytes().await.expect("should get bytes");

        assert_eq!(bytes, binary_data.as_slice());
    }

    #[tokio::test]
    async fn should_handle_as_raw_with_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/raw-json"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"key": "value"})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let raw = client
            .get("/raw-json")
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(201))
            .await
            .expect("request should succeed")
            .as_raw()
            .await
            .expect("should get raw");

        assert_eq!(raw.status_code(), http::StatusCode::CREATED);
        assert!(raw.content_type().is_some());
        assert!(
            raw.content_type()
                .expect("has content type")
                .contains("json")
        );
        assert!(matches!(raw.body(), RawBody::Text(_)));
        assert!(raw.text().is_some());
        assert!(raw.text().expect("has text").contains("value"));
    }

    #[tokio::test]
    async fn should_handle_as_raw_with_binary() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/raw-binary"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .set_body_bytes(vec![0x01, 0x02, 0x03]),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let raw = client
            .get("/raw-binary")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_raw()
            .await
            .expect("should get raw");

        assert_eq!(raw.status_code(), http::StatusCode::OK);
        assert!(matches!(raw.body(), RawBody::Binary(_)));
        assert!(raw.bytes().is_some());
    }

    #[tokio::test]
    async fn should_handle_as_raw_with_empty() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/item/1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let raw = client
            .call(
                http::Method::DELETE,
                CallPath::from("/item/{id}").add_param("id", 1),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(204))
            .await
            .expect("request should succeed")
            .as_raw()
            .await
            .expect("should get raw");

        assert_eq!(raw.status_code(), http::StatusCode::NO_CONTENT);
        assert!(raw.is_empty());
        assert!(matches!(raw.body(), RawBody::Empty));
    }

    #[tokio::test]
    async fn should_handle_as_empty() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/resource/42"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        client
            .call(
                http::Method::DELETE,
                CallPath::from("/resource/{id}").add_param("id", 42),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(204))
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");
    }

    #[tokio::test]
    async fn should_handle_as_text_error_for_json_content() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/json-not-text"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"key": "value"})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let mut response = client
            .get("/json-not-text")
            .expect("should create call")
            .await
            .expect("request should succeed");

        let result = response.as_text().await;

        assert!(result.is_err());
        match result {
            Err(ApiClientError::UnsupportedTextOutput { .. }) => {}
            other => panic!("Expected UnsupportedTextOutput, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn should_handle_as_bytes_error_for_text_content() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/text-not-bytes"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Hello"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let mut response = client
            .get("/text-not-bytes")
            .expect("should create call")
            .await
            .expect("request should succeed");

        let result = response.as_bytes().await;

        assert!(result.is_err());
        match result {
            Err(ApiClientError::UnsupportedBytesOutput { .. }) => {}
            other => panic!("Expected UnsupportedBytesOutput, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn should_handle_as_json_error_for_text_content() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/text-not-json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Not JSON"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let result: Result<User, ApiClientError> = client
            .get("/text-not-json")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_json()
            .await;

        assert!(result.is_err());
        match result {
            Err(ApiClientError::UnsupportedJsonOutput { .. }) => {}
            other => panic!("Expected UnsupportedJsonOutput, got {other:?}"),
        }
    }
}

// =============================================================================
// Tests for operation.rs - Operation building and merging
// =============================================================================

mod operation_tests {
    use super::*;

    #[tokio::test]
    async fn should_build_operation_with_request_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/users"))
            .and(body_json(json!({
                "name": "New User",
                "email": "new@example.com"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": 1,
                "name": "New User",
                "email": "new@example.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        let _: User = client
            .post("/users")
            .expect("should create call")
            .json(&CreateUserRequest {
                name: "New User".to_string(),
                email: "new@example.com".to_string(),
            })
            .expect("should set body")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(201))
            .await
            .expect("request should succeed")
            .as_json()
            .await
            .expect("should parse JSON");

        // Verify OpenAPI includes request body
        let openapi = client.collected_openapi().await;
        let post_op = openapi
            .paths
            .paths
            .get("/users")
            .expect("should have path")
            .post
            .as_ref()
            .expect("should have POST");

        assert!(post_op.request_body.is_some());
        let request_body = post_op
            .request_body
            .as_ref()
            .expect("should have request body");
        assert!(request_body.content.contains_key("application/json"));
    }

    #[tokio::test]
    async fn should_build_operation_with_security_requirements() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/protected"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": "secret"})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_with_security(&mock_server).await;

        client
            .get("/protected")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify OpenAPI includes security
        let openapi = client.collected_openapi().await;

        // Check security schemes in components
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.security_schemes.contains_key("bearerAuth"));
        assert!(components.security_schemes.contains_key("apiKey"));

        // Check default security requirement
        let security = openapi.security.as_ref().expect("should have security");
        assert!(!security.is_empty());
    }

    #[tokio::test]
    async fn should_merge_operations_on_same_path() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/items"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/items"))
            .and(query_param("offset", "5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        // First call with limit parameter
        client
            .get("/items")
            .expect("should create call")
            .with_query(crate::client::CallQuery::new().add_param("limit", 10))
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Second call with offset parameter
        client
            .get("/items")
            .expect("should create call")
            .with_query(crate::client::CallQuery::new().add_param("offset", 5))
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify both parameters are merged in OpenAPI
        let openapi = client.collected_openapi().await;
        let get_op = openapi
            .paths
            .paths
            .get("/items")
            .expect("should have path")
            .get
            .as_ref()
            .expect("should have GET");

        let params = get_op.parameters.as_ref().expect("should have parameters");
        let param_names: Vec<_> = params.iter().map(|p| p.name.as_str()).collect();
        assert!(param_names.contains(&"limit"));
        assert!(param_names.contains(&"offset"));
    }

    #[tokio::test]
    async fn should_generate_automatic_tags_from_path() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/observations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        client
            .get("/api/v1/observations")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify automatic tags are generated
        let openapi = client.collected_openapi().await;
        let tags = openapi.tags.as_ref().expect("should have tags");
        let tag_names: Vec<_> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(tag_names.contains(&"observations"));
    }

    #[tokio::test]
    async fn should_handle_nested_path_actions() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/observations/import"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"imported": 10})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        client
            .post("/observations/import")
            .expect("should create call")
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify OpenAPI has the import path
        let openapi = client.collected_openapi().await;
        assert!(openapi.paths.paths.contains_key("/observations/import"));
    }

    #[tokio::test]
    async fn should_handle_custom_operation_metadata() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/special"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        client
            .get("/special")
            .expect("should create call")
            .with_operation_id("getSpecialResource")
            .with_tags(["custom", "special"])
            .with_description("A special endpoint")
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify custom metadata in OpenAPI
        let openapi = client.collected_openapi().await;
        let get_op = openapi
            .paths
            .paths
            .get("/special")
            .expect("should have path")
            .get
            .as_ref()
            .expect("should have GET");

        assert_eq!(get_op.operation_id.as_deref(), Some("getSpecialResource"));
        assert_eq!(get_op.description.as_deref(), Some("A special endpoint"));

        let tags = get_op.tags.as_ref().expect("should have tags");
        assert!(tags.contains(&"custom".to_string()));
        assert!(tags.contains(&"special".to_string()));
    }

    #[tokio::test]
    async fn should_override_security_per_operation() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/public"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_with_security(&mock_server).await;

        client
            .get("/public")
            .expect("should create call")
            .with_securities([SecurityRequirement::new("apiKey")])
            .await
            .expect("request should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify operation-level security in OpenAPI
        let openapi = client.collected_openapi().await;
        let get_op = openapi
            .paths
            .paths
            .get("/public")
            .expect("should have path")
            .get
            .as_ref()
            .expect("should have GET");

        let security = get_op.security.as_ref().expect("should have security");
        assert!(!security.is_empty());
    }
}

// =============================================================================
// Tests for mod.rs - ApiClient and OpenAPI generation
// =============================================================================

mod client_tests {
    use super::*;
    use crate::client::security::OAuth2Flows;

    #[tokio::test]
    async fn should_collect_paths_from_multiple_endpoints() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/posts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/users/1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        // Make multiple calls
        client
            .get("/users")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        client
            .get("/posts")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        client
            .call(
                http::Method::DELETE,
                CallPath::from("/users/{id}").add_param("id", 1),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(204))
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify collected paths
        let paths = client.collected_paths().await;
        assert!(paths.paths.contains_key("/users"));
        assert!(paths.paths.contains_key("/posts"));
        assert!(paths.paths.contains_key("/users/{id}"));
    }

    #[tokio::test]
    async fn should_generate_complete_openapi_spec() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/item"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(
                {"id": 1, "name": "Item 1", "email": "item1@test.com"}
            )))
            .expect(1)
            .mount(&mock_server)
            .await;

        let uri: http::Uri = mock_server.uri().parse().expect("valid URI");
        let mut client = ApiClient::builder()
            .with_host(uri.host().expect("should have host"))
            .with_port(uri.port_u16().expect("should have port"))
            .with_info_simple("Test API", "1.0.0")
            .with_description("A test API")
            .add_server_simple(mock_server.uri(), "Test server")
            .build()
            .expect("should build client");

        let _: User = client
            .get("/api/item")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_json()
            .await
            .expect("should parse");

        let openapi = client.collected_openapi().await;

        // Verify info
        assert_eq!(openapi.info.title, "Test API");
        assert_eq!(openapi.info.version, "1.0.0");
        assert_eq!(openapi.info.description.as_deref(), Some("A test API"));

        // Verify servers
        let servers = openapi.servers.as_ref().expect("should have servers");
        assert!(!servers.is_empty());

        // Verify paths
        assert!(openapi.paths.paths.contains_key("/api/item"));

        // Verify components (schemas)
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.schemas.contains_key("User"));
    }

    #[tokio::test]
    async fn should_register_schema_manually() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        // Manually register a schema
        client.register_schema::<ApiError>().await;

        client
            .get("/test")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        // Verify schema is registered
        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.schemas.contains_key("ApiError"));
    }

    #[tokio::test]
    async fn should_handle_all_http_methods() {
        let mock_server = MockServer::start().await;

        for method_str in ["GET", "POST", "PUT", "DELETE", "PATCH"] {
            Mock::given(method(method_str))
                .and(path(format!("/{}", method_str.to_lowercase())))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
                .expect(1)
                .mount(&mock_server)
                .await;
        }

        let mut client = client_for_mock(&mock_server).await;

        client
            .get("/get")
            .expect("GET")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        client
            .post("/post")
            .expect("POST")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        client
            .put("/put")
            .expect("PUT")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        client
            .delete("/delete")
            .expect("DELETE")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        client
            .patch("/patch")
            .expect("PATCH")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        let openapi = client.collected_openapi().await;
        assert!(openapi.paths.paths.get("/get").expect("/get").get.is_some());
        assert!(
            openapi
                .paths
                .paths
                .get("/post")
                .expect("/post")
                .post
                .is_some()
        );
        assert!(openapi.paths.paths.get("/put").expect("/put").put.is_some());
        assert!(
            openapi
                .paths
                .paths
                .get("/delete")
                .expect("/delete")
                .delete
                .is_some()
        );
        assert!(
            openapi
                .paths
                .paths
                .get("/patch")
                .expect("/patch")
                .patch
                .is_some()
        );
    }

    #[tokio::test]
    async fn should_handle_oauth2_security_scheme() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/oauth-protected"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let uri: http::Uri = mock_server.uri().parse().expect("valid URI");
        let mut client = ApiClient::builder()
            .with_host(uri.host().expect("should have host"))
            .with_port(uri.port_u16().expect("should have port"))
            .with_security_scheme(
                "oauth2",
                SecurityScheme::OAuth2 {
                    flows: Box::new(OAuth2Flows::authorization_code(
                        "https://auth.example.com/authorize",
                        "https://auth.example.com/token",
                        [("read:users", "Read user data")],
                    )),
                    description: Some("OAuth2 authentication".to_string()),
                },
            )
            .build()
            .expect("should build client");

        client
            .get("/oauth-protected")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.security_schemes.contains_key("oauth2"));
    }
}

// =============================================================================
// Tests for security.rs - Security scheme configurations
// =============================================================================

mod security_tests {
    use super::*;
    use crate::client::security::{OAuth2Flow, OAuth2Flows, OAuth2ImplicitFlow};

    #[tokio::test]
    async fn should_convert_all_api_key_locations() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let uri: http::Uri = mock_server.uri().parse().expect("valid URI");
        let mut client = ApiClient::builder()
            .with_host(uri.host().expect("should have host"))
            .with_port(uri.port_u16().expect("should have port"))
            .with_security_scheme(
                "headerKey",
                SecurityScheme::api_key("X-API-Key", ApiKeyLocation::Header)
                    .with_description("API key in header"),
            )
            .with_security_scheme(
                "queryKey",
                SecurityScheme::api_key("api_key", ApiKeyLocation::Query)
                    .with_description("API key in query"),
            )
            .with_security_scheme(
                "cookieKey",
                SecurityScheme::api_key("session", ApiKeyLocation::Cookie)
                    .with_description("API key in cookie"),
            )
            .build()
            .expect("should build client");

        client
            .get("/test")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.security_schemes.contains_key("headerKey"));
        assert!(components.security_schemes.contains_key("queryKey"));
        assert!(components.security_schemes.contains_key("cookieKey"));
    }

    #[tokio::test]
    async fn should_convert_openid_connect_scheme() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let uri: http::Uri = mock_server.uri().parse().expect("valid URI");
        let mut client = ApiClient::builder()
            .with_host(uri.host().expect("should have host"))
            .with_port(uri.port_u16().expect("should have port"))
            .with_security_scheme(
                "oidc",
                SecurityScheme::openid_connect(
                    "https://auth.example.com/.well-known/openid-configuration",
                )
                .with_description("OpenID Connect"),
            )
            .build()
            .expect("should build client");

        client
            .get("/test")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.security_schemes.contains_key("oidc"));
    }

    #[tokio::test]
    async fn should_convert_oauth2_with_all_flows() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&mock_server)
            .await;

        let uri: http::Uri = mock_server.uri().parse().expect("valid URI");

        let flows = OAuth2Flows {
            authorization_code: Some(OAuth2Flow {
                authorization_url: Some("https://auth.example.com/authorize".to_string()),
                token_url: "https://auth.example.com/token".to_string(),
                refresh_url: Some("https://auth.example.com/refresh".to_string()),
                scopes: [("read".to_string(), "Read access".to_string())]
                    .into_iter()
                    .collect(),
            }),
            client_credentials: Some(OAuth2Flow {
                authorization_url: None,
                token_url: "https://auth.example.com/token".to_string(),
                refresh_url: None,
                scopes: [("api".to_string(), "API access".to_string())]
                    .into_iter()
                    .collect(),
            }),
            implicit: Some(OAuth2ImplicitFlow {
                authorization_url: "https://auth.example.com/authorize".to_string(),
                refresh_url: Some("https://auth.example.com/refresh".to_string()),
                scopes: [("implicit".to_string(), "Implicit access".to_string())]
                    .into_iter()
                    .collect(),
            }),
            password: Some(OAuth2Flow {
                authorization_url: None,
                token_url: "https://auth.example.com/token".to_string(),
                refresh_url: Some("https://auth.example.com/refresh".to_string()),
                scopes: [("password".to_string(), "Password access".to_string())]
                    .into_iter()
                    .collect(),
            }),
        };

        let mut client = ApiClient::builder()
            .with_host(uri.host().expect("should have host"))
            .with_port(uri.port_u16().expect("should have port"))
            .with_security_scheme(
                "oauth2",
                SecurityScheme::OAuth2 {
                    flows: Box::new(flows),
                    description: Some("OAuth2 with all flows".to_string()),
                },
            )
            .build()
            .expect("should build client");

        client
            .get("/test")
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_empty()
            .await
            .expect("should complete");

        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.security_schemes.contains_key("oauth2"));
    }

    #[test]
    fn should_handle_description_for_all_scheme_types() {
        // Test that with_description works for all security scheme variants
        let bearer = SecurityScheme::bearer().with_description("Bearer token");
        assert!(matches!(
            bearer,
            SecurityScheme::Bearer {
                description: Some(ref d),
                ..
            } if d == "Bearer token"
        ));

        let basic = SecurityScheme::basic().with_description("Basic auth");
        assert!(matches!(
            basic,
            SecurityScheme::Basic {
                description: Some(ref d)
            } if d == "Basic auth"
        ));

        let api_key =
            SecurityScheme::api_key("key", ApiKeyLocation::Header).with_description("API key auth");
        assert!(matches!(
            api_key,
            SecurityScheme::ApiKey {
                description: Some(ref d),
                ..
            } if d == "API key auth"
        ));

        let oidc =
            SecurityScheme::openid_connect("https://example.com").with_description("OIDC auth");
        assert!(matches!(
            oidc,
            SecurityScheme::OpenIdConnect {
                description: Some(ref d),
                ..
            } if d == "OIDC auth"
        ));

        let oauth2 = SecurityScheme::OAuth2 {
            flows: Box::default(),
            description: None,
        }
        .with_description("OAuth2 auth");
        assert!(matches!(
            oauth2,
            SecurityScheme::OAuth2 {
                description: Some(ref d),
                ..
            } if d == "OAuth2 auth"
        ));
    }
}

// =============================================================================
// Tests for channel.rs - Schema and operation collection
// =============================================================================

mod channel_tests {
    use super::*;

    #[tokio::test]
    async fn should_collect_schemas_from_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/user/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(
                {"id": 1, "name": "User 1", "email": "user1@test.com"}
            )))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        let _: User = client
            .call(
                http::Method::GET,
                CallPath::from("/user/{id}").add_param("id", 1),
            )
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_json()
            .await
            .expect("should parse");

        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.schemas.contains_key("User"));
    }

    #[tokio::test]
    async fn should_collect_schemas_from_request_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/users"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": 1,
                "name": "New User",
                "email": "new@test.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        let _: User = client
            .post("/users")
            .expect("should create call")
            .json(&CreateUserRequest {
                name: "New User".to_string(),
                email: "new@test.com".to_string(),
            })
            .expect("should set body")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(201))
            .await
            .expect("should succeed")
            .as_json()
            .await
            .expect("should parse");

        let openapi = client.collected_openapi().await;
        let components = openapi.components.as_ref().expect("should have components");
        assert!(components.schemas.contains_key("CreateUserRequest"));
        assert!(components.schemas.contains_key("User"));
    }

    #[tokio::test]
    async fn should_register_responses_with_different_status_codes() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/items/success"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 1,
                "name": "Item",
                "email": "item@test.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/items/created"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": 2,
                "name": "Created Item",
                "email": "created@test.com"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut client = client_for_mock(&mock_server).await;

        let _: User = client
            .call(
                http::Method::GET,
                CallPath::from("/items/{status}").add_param("status", "success"),
            )
            .expect("should create call")
            .await
            .expect("should succeed")
            .as_json()
            .await
            .expect("should parse");

        let _: User = client
            .call(
                http::Method::GET,
                CallPath::from("/items/{status}").add_param("status", "created"),
            )
            .expect("should create call")
            .with_expected_status_codes(ExpectedStatusCodes::from_single(201))
            .await
            .expect("should succeed")
            .as_json()
            .await
            .expect("should parse");

        let openapi = client.collected_openapi().await;
        let get_op = openapi
            .paths
            .paths
            .get("/items/{status}")
            .expect("should have path")
            .get
            .as_ref()
            .expect("should have GET");

        // Both status codes should be registered
        assert!(get_op.responses.responses.contains_key("200"));
        assert!(get_op.responses.responses.contains_key("201"));
    }

    #[tokio::test]
    async fn should_handle_concurrent_requests() {
        let mock_server = MockServer::start().await;

        for i in 1..=5 {
            Mock::given(method("GET"))
                .and(path(format!("/item/{i}")))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": i,
                    "name": format!("Item {i}"),
                    "email": format!("item{i}@test.com")
                })))
                .expect(1)
                .mount(&mock_server)
                .await;
        }

        let mut client = client_for_mock(&mock_server).await;

        // Make concurrent requests
        let mut handles = vec![];
        for i in 1..=5 {
            let client_clone = client.clone();
            let path = format!("/item/{i}");
            handles.push(tokio::spawn(async move {
                let c = client_clone;
                c.get(&*path)
                    .expect("should create call")
                    .await
                    .expect("should succeed")
                    .as_empty()
                    .await
                    .expect("should complete");
            }));
        }

        for handle in handles {
            handle.await.expect("task should complete");
        }

        // Verify all paths are collected
        let openapi = client.collected_openapi().await;
        for i in 1..=5 {
            assert!(openapi.paths.paths.contains_key(&format!("/item/{i}")));
        }
    }
}

// =============================================================================
// Tests for response content type handling
// =============================================================================

mod content_type_tests {
    use super::*;

    #[tokio::test]
    async fn should_handle_other_content_type() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/xml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("<root><item>value</item></root>", "application/xml"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let mut response = client
            .get("/xml")
            .expect("should create call")
            .await
            .expect("should succeed");

        let raw = response.as_raw().await.expect("should get raw");

        let content_type = raw.content_type().expect("has content type");
        assert!(
            content_type.contains("xml"),
            "Expected content type to contain 'xml', got: {content_type}"
        );
        assert!(raw.text().expect("has text").contains("<root>"));
    }

    #[tokio::test]
    async fn should_handle_html_content_type() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/html"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html")
                    .set_body_string("<html><body>Hello</body></html>"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = client_for_mock(&mock_server).await;

        let mut response = client
            .get("/html")
            .expect("should create call")
            .await
            .expect("should succeed");

        let text = response.as_text().await.expect("should get text");

        assert!(text.contains("<html>"));
    }
}
