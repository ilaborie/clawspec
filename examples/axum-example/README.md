# Axum Example - Bird Observation API

This example demonstrates how to use Clawspec to generate OpenAPI documentation from tests for an Axum-based REST API.

## Running the Project

```bash
# Start the server
cargo run --bin axum-example

# The API will be available at http://localhost:3000
```

## REST API Overview

The Bird Observation API provides endpoints for managing bird observations with support for:

- CRUD operations on observations
- Multiple content types (JSON, XML, form-encoded)
- Bulk import/upload operations
- Partial updates via PATCH

See the generated [OpenAPI specification](./doc/openapi.yml) for complete API documentation.

## Generating OpenAPI Documentation

The OpenAPI specification is generated automatically by running the test suite:

```bash
# Generate OpenAPI spec
cargo test --package axum-example generate_openapi

# The specification will be written to doc/openapi.yml
```

### How the Test Works

The [`generate_openapi.rs`](./tests/generate_openapi.rs) test file demonstrates Clawspec's capabilities:

1. **Test Setup**: Creates a `TestApp` instance that wraps the Axum server with Clawspec's `ApiClient`
2. **Schema Registration**: Manually registers domain types that need to be included in the OpenAPI spec:

   ```rust
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
   ```

3. **Test Scenarios**: The test exercises different API features through multiple functions:
   - **`basic_crud`**: Tests standard CRUD operations (Create, Read, Update, Delete)
     - Creates observations with path parameters
     - Lists observations with query parameters (pagination)
     - Demonstrates header parameter collection
     - Updates and patches observations
     - Deletes observations
   - **`alternate_content_types`**: Tests various content type handlers
     - JSON (standard format)
     - Form-encoded data (using flattened structure)
     - XML data
     - Bulk import via newline-delimited JSON (NDJSON)
     - Multipart upload for multiple observations
   - **`test_error_cases`**: Captures error response schemas
     - Unsupported media type (415 error)
     - Invalid JSON parsing errors
     - Custom headers with error responses
   - **`demonstrate_tags_and_metadata`**: Shows OpenAPI organization features
     - Single and multiple tags for operation grouping
     - Operation descriptions for better documentation
     - Administrative and bulk operation tagging

4. **OpenAPI Generation**: After running all test scenarios, the collected schema and path information is written to `doc/openapi.yml`:
   ```rust
   app.write_openapi("./doc/openapi.yml").await?;
   ```

The test automatically captures:

- Request/response schemas from actual API calls
- Path and query parameters
- Header parameters
- Multiple content types per endpoint
- Error response schemas
- Operation metadata (tags, descriptions)

This approach ensures the OpenAPI documentation stays synchronized with the actual API implementation, as any changes to the API will be reflected when the tests run.

## Response Handling Patterns

Clawspec provides several methods for handling API responses:

| Method | Use Case |
|--------|----------|
| `as_json::<T>()` | Standard JSON deserialization |
| `as_optional_json::<T>()` | When response may be empty or null |
| `as_result_json::<T, E>()` | For APIs with typed error responses |
| `as_result_option_json::<T, E>()` | Combining Result and Option |
| `as_text()` | Plain text responses |
| `as_bytes()` | Binary data |
| `as_empty()` | No-body responses (e.g., 204 No Content) |
| `as_raw()` | Raw response access before schema collection |

See the test files in `tests/` for examples of each pattern.
