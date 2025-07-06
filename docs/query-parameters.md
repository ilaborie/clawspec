# Query Parameters Guide

This guide explains how to use the query parameter system in clawspec for building type-safe HTTP requests with OpenAPI 3.1 compliance.

## Overview

The query parameter system provides:

- **Type Safety**: Compile-time guarantees for parameter types
- **OpenAPI 3.1 Support**: Automatic schema generation and style compliance
- **Flexible Serialization**: Support for different parameter styles
- **Builder Pattern**: Fluent API for constructing queries

## Quick Start

```rust
use clawspec_utoipa::client::query::{CallQuery, DisplayQuery, SerializableQuery, QueryStyle};

let query = CallQuery::new()
    .add_param("search", DisplayQuery("hello world"))
    .add_param("limit", DisplayQuery(10))
    .add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]));
```

## Parameter Types

### DisplayQuery

Use `DisplayQuery` for simple types that implement `std::fmt::Display`:

```rust
// String literals and owned strings
.add_param("name", DisplayQuery("John Doe"))
.add_param("title", DisplayQuery(title_string))

// Numbers
.add_param("age", DisplayQuery(30))
.add_param("price", DisplayQuery(19.99))

// Booleans
.add_param("active", DisplayQuery(true))
.add_param("verified", DisplayQuery(false))
```

### SerializableQuery

Use `SerializableQuery` for complex types like arrays or structs that implement `Serialize`:

```rust
// Arrays with default form style
.add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]))

// Arrays with specific styles
.add_param("categories", SerializableQuery::with_style(
    vec!["tech", "programming"], 
    QueryStyle::SpaceDelimited
))

.add_param("ids", SerializableQuery::with_style(
    vec![1, 2, 3], 
    QueryStyle::PipeDelimited
))
```

## OpenAPI 3.1 Query Styles

The system supports three OpenAPI 3.1 query parameter styles:

### Form Style (Default)

Arrays are repeated as separate parameters:

```
Input:  vec!["rust", "web", "api"]
Output: ?tags=rust&tags=web&tags=api
```

### Space Delimited Style

Array values are joined with spaces (URL-encoded as `+` or `%20`):

```
Input:  vec!["tech", "programming"]
Output: ?categories=tech+programming
```

### Pipe Delimited Style

Array values are joined with pipes (URL-encoded as `%7C`):

```
Input:  vec![1, 2, 3]
Output: ?ids=1%7C2%7C3
```

## Custom Types

You can use custom types as query parameters by implementing the required traits:

### Using Display

```rust
#[derive(Debug, Clone)]
enum SortOrder {
    Asc,
    Desc,
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortOrder::Asc => write!(f, "asc"),
            SortOrder::Desc => write!(f, "desc"),
        }
    }
}

// Usage
.add_param("sort", DisplayQuery(SortOrder::Desc))
```

### Using Serialize

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Active,
    Inactive,
    Pending,
}

// Usage
.add_param("status", SerializableQuery::new(Status::Active))
```

## Error Handling

The query parameter system has specific limitations:

### Supported Types

- ✅ Strings, numbers, booleans, null values
- ✅ Arrays of supported types
- ✅ Custom types implementing Display or Serialize appropriately

### Unsupported Types

- ❌ Objects (JSON objects are not valid query parameters)
- ❌ Nested arrays containing objects
- ❌ Complex nested structures

### Runtime Errors

Unsupported types will result in `ApiClientError::UnsupportedQueryParameterValue`:

```rust
// This will cause a runtime error
let invalid_query = CallQuery::new()
    .add_param("config", SerializableQuery::new(json!({"key": "value"})));

match invalid_query.to_query_string() {
    Err(ApiClientError::UnsupportedQueryParameterValue { value }) => {
        println!("Unsupported parameter value: {}", value);
    }
    _ => {}
}
```

## Best Practices

### 1. Choose the Right Wrapper

- Use `DisplayQuery` for simple types (strings, numbers, booleans)
- Use `SerializableQuery` for arrays and serializable enums/structs
- Avoid complex objects in query parameters

### 2. Consider Parameter Styles

- Use **Form style** (default) for most array parameters
- Use **Space delimited** when the API expects space-separated values
- Use **Pipe delimited** when the API expects pipe-separated values

### 3. Type Safety

```rust
// Good: Type-safe parameter building
let query = CallQuery::new()
    .add_param("limit", DisplayQuery(limit_value))
    .add_param("tags", SerializableQuery::new(tag_vec));

// Avoid: String concatenation (loses type safety)
let query_string = format!("?limit={}&tags={}", limit, tags.join(","));
```

### 4. Error Handling

Always handle potential serialization errors:

```rust
match query.to_query_string() {
    Ok(query_string) => {
        // Use the query string
    }
    Err(ApiClientError::UnsupportedQueryParameterValue { value }) => {
        // Handle unsupported parameter type
    }
    Err(e) => {
        // Handle other errors
    }
}
```

## Integration with HTTP Calls

Query parameters integrate seamlessly with the HTTP client:

```rust
use clawspec_utoipa::client::{ApiClient, query::{CallQuery, DisplayQuery}};

let client = ApiClient::new("https://api.example.com").unwrap();

let query = CallQuery::new()
    .add_param("search", DisplayQuery("rust programming"))
    .add_param("limit", DisplayQuery(10));

let response = client
    .get("/api/posts")
    .query(query)
    .exchange()
    .await?;
```

## OpenAPI Schema Generation

The query parameter system automatically generates OpenAPI schemas:

```rust
// This generates proper OpenAPI parameter definitions
let query = CallQuery::new()
    .add_param("search", DisplayQuery("example"))
    .add_param("tags", SerializableQuery::with_style(
        vec!["rust", "web"], 
        QueryStyle::SpaceDelimited
    ));

// Generates OpenAPI parameters like:
// - name: search
//   in: query
//   required: false
//   schema:
//     type: string
//   style: form
// - name: tags
//   in: query
//   required: false
//   schema:
//     type: string
//   style: spaceDelimited
```

## Complete Example

```rust
use clawspec_utoipa::client::query::{CallQuery, DisplayQuery, SerializableQuery, QueryStyle};

fn build_search_query(
    search_term: &str,
    page: u32,
    tags: Vec<String>,
    categories: Vec<String>,
    sort_order: SortOrder,
) -> CallQuery {
    CallQuery::new()
        // Simple parameters
        .add_param("q", DisplayQuery(search_term))
        .add_param("page", DisplayQuery(page))
        .add_param("sort", DisplayQuery(sort_order))
        
        // Array parameters with different styles
        .add_param("tags", SerializableQuery::new(tags))
        .add_param("categories", SerializableQuery::with_style(
            categories, 
            QueryStyle::SpaceDelimited
        ))
}
```

This system ensures your query parameters are type-safe, properly encoded, and compliant with OpenAPI 3.1 specifications.