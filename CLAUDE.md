# Clawspec - AI Agent Development Reference

## CRITICAL RULES - ENFORCE THESE ALWAYS

### NEVER:
- Use `unwrap()` in production code - use proper error handling
- Use `unwrap()` in test code - use `expect()` with descriptive messages
- Commit code that doesn't compile
- Skip running checks before committing
- Use variable type annotations when turbofish syntax is clearer
- Use `grep` or `find` - use `rg` and `fd` instead
- Add comments unless explicitly requested
- Test `Debug`, `Display`, or `Clone` behavior in unit tests
- Mention specific OpenAPI version numbers in documentation

### ALWAYS:
- Use `cargo nextest run` for testing (preferred over `cargo test`)
- Run `mise run check` before committing
- Update tests with implementation changes
- Use descriptive variable names
- Follow DRY, KISS, and YAGNI principles
- Work in git branches
- Prefer turbofish syntax (`::<T>()`) over type annotations
- Use `_` inside turbofish when compiler can infer inner type
- Prefer `&str` over `String` when possible
- Prefer `as_ref()` over `.clone()` unless necessary
- Use inline insta snapshot testing for complex assertions
- Include GitHub issue links in TODO comments: `// TODO: Description - https://github.com/ilaborie/clawspec/issues/N`

---

## Project Context

**Tech Stack**: Rust + utoipa + reqwest + tokio
**Architecture**: Library for test-driven OpenAPI specification generation
**Philosophy**: KISS, DRY, YAGNI, type safety first, async-first design, builder pattern
**Test Runner**: cargo-nextest (preferred)

**Core Concept**: Write tests that exercise your API → Automatically generate OpenAPI specs from the test execution

---

## Architecture Map

```
clawspec/
├── lib/
│   ├── clawspec-core/        # Core library
│   │   ├── src/
│   │   │   ├── client/       # HTTP client and collectors
│   │   │   │   ├── mod.rs    # ApiClient - main entry point
│   │   │   │   ├── call.rs   # ApiCall - builder for API calls
│   │   │   │   ├── path.rs   # CallPath - path parameters
│   │   │   │   ├── query.rs  # CallQuery - query parameters
│   │   │   │   └── headers.rs # CallHeaders - header parameters
│   │   │   ├── collectors/   # OpenAPI data collection system
│   │   │   └── lib.rs
│   │   └── tests/            # Unit and integration tests
│   └── clawspec-macro/       # Procedural macros (future)
├── examples/
│   └── axum-example/         # Complete example with Axum
│       ├── src/              # API implementation
│       ├── tests/            # Tests that generate OpenAPI
│       └── doc/openapi.yml   # Generated OpenAPI spec
└── .mise.toml                # Tool configuration

```

**Data Flow**: Test → ApiClient → HTTP Request → Collectors → OpenAPI Schema → YAML Output

---

## Development Commands

```bash
# Quality Checks (ask before running)
mise run check            # Format + lint + test (pre-push checks)
mise run lint             # Clippy linting only
mise run fix              # Auto-fix formatting and linting
cargo fmt                 # Format code only
cargo clippy              # Lint code only

# Testing
mise run test             # Comprehensive test suite (nextest + doc tests)
cargo nextest run         # Fast test runner (preferred)
cargo test                # Standard test runner
cargo test --doc          # Documentation tests
mise run test:review      # Run insta snapshot tests and review changes

# OpenAPI Validation
mise spectral             # Lint generated OpenAPI files

# Security
mise run audit            # Run security vulnerability scan

# Build
cargo build               # Development build
cargo build --release     # Production build
cargo check               # Check compilation without building

# Examples
cargo run --bin axum-example                          # Run example server
cargo test --package axum-example generate_openapi   # Generate OpenAPI from tests

# Changelog
git cliff                 # Generate changelog
```

---

## Code Patterns & Rules

### Rust Rules
- **Edition 2024** - use latest language features
- **No `unwrap()` in production** - use `?`, `expect()`, or proper error handling
- **Use `expect()` in tests** - never `unwrap()`, always provide descriptive messages
- **Prefer `&str` over `String`** when possible
- **Prefer turbofish syntax** - `method::<Type>()` over `let x: Type = method()`
- **Use `as_ref()`** over `.clone()` unless clone is necessary
- **Unsafe code denied** - `unsafe_code = "deny"` in workspace
- **Missing docs warned** - `missing_docs = "warn"` for public APIs

### Derive Macro Order
```rust
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
    Serialize, Deserialize,  // Without serde:: prefix
    utoipa::ToSchema,        // Qualified names for other derives
    derive_more::Debug,
)]
```

### Code Style
- **DRY** - extract reusable logic into functions, modules, or traits
- **KISS** - simplicity over cleverness
- **YAGNI** - don't add features until needed
- **Self-documenting** - minimal comments, clear names
- **Async-first** - use Tokio runtime throughout
- **Builder pattern** - use `with_*` prefix for setters, `add_*` for accumulating

### Builder Pattern for API Calls
```rust
// ApiCall builder pattern example
let response = client
    .call("GET", "/observations/{id}")
    .with_path("id", observation_id)
    .with_query("include", "details")
    .with_header("Authorization", format!("Bearer {token}"))
    .send()
    .await?;
```

### Error Handling Pattern
```rust
use derive_more::{Display, Error, From};

#[derive(Debug, Display, Error, From)]
pub enum ClientError {
    #[display("HTTP request failed: {_0}")]
    Request(reqwest::Error),

    #[display("Schema collection failed: {reason}")]
    #[from(ignore)]
    SchemaCollection { reason: String },
}
```

### Testing Pattern with Insta Snapshots
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;

    #[test]
    fn should_build_path_with_parameters() {
        let path = CallPath::new("/users/{id}/posts/{post_id}")
            .with_param("id", "user123")
            .with_param("post_id", 42);

        // Use snapshot testing for complex assertions
        assert_json_snapshot!(path, @r#"
        {
          "template": "/users/{id}/posts/{post_id}",
          "params": {
            "id": "user123",
            "post_id": 42
          }
        }
        "#);
    }

    #[tokio::test]
    async fn should_collect_openapi_schema() {
        let client = ApiClient::new("http://localhost:8080");

        // Exercise API
        client
            .call("GET", "/observations/{id}")
            .with_path("id", "obs-123")
            .send()
            .await
            .expect("API call should succeed");

        // Generate OpenAPI spec
        let openapi = client.generate_openapi_spec();

        // Snapshot test the generated spec
        assert_json_snapshot!(openapi);
    }
}
```

### File Naming Conventions
- Modules: `snake_case.rs` (e.g., `client/path.rs`, `client/query.rs`)
- Types: `PascalCase` (e.g., `ApiClient`, `CallPath`, `CallQuery`)
- Functions: `snake_case` (e.g., `with_path`, `generate_spec`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_TIMEOUT`)

### Import Formatting

**Always follow this consistent import structure:**

1. **Group imports** into logical sections (with blank lines between groups):
   - Standard library (`std::`)
   - External crates (alphabetically ordered)
   - Internal crate imports (`crate::`)

2. **Alphabetical ordering** within each group

3. **Consolidate imports** from the same module:
   ```rust
   // ❌ BAD - Multiple imports from same module
   use crate::client::ApiClient;
   use crate::client::ApiCall;

   // ✅ GOOD - Consolidated
   use crate::client::{ApiCall, ApiClient};
   ```

**Example:**
```rust
use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use utoipa::ToSchema;

use crate::client::{ApiCall, ApiClient};
use crate::collectors::SchemaCollector;
```

---

## 📁 File Organization

### Key Locations
- **Core Client**: `lib/clawspec-core/src/client/` - HTTP client and API call builders
- **Collectors**: `lib/clawspec-core/src/collectors/` - OpenAPI data collection
- **Path Handling**: `lib/clawspec-core/src/client/path.rs` - Path parameter processing
- **Query Handling**: `lib/clawspec-core/src/client/query.rs` - Query parameter processing
- **Header Handling**: `lib/clawspec-core/src/client/headers.rs` - Header parameter processing
- **Examples**: `examples/axum-example/` - Full working example with Axum
- **Generated Specs**: `examples/axum-example/doc/openapi.yml` - Example output

### Architecture Layers
1. **Client Layer** - HTTP requests, builder pattern for API calls
2. **Collector Layer** - Schema collection, OpenAPI path gathering
3. **Generator Layer** - OpenAPI specification generation from collected data

---

## Change Approach

### Adding New Parameter Type Support

When adding support for a new parameter type (e.g., cookie parameters, form data):

1. **Create Parameter Handler**: Add new module in `lib/clawspec-core/src/client/` (e.g., `cookies.rs`)
2. **Define Builder**: Implement builder pattern with `with_*` and `add_*` methods
3. **Schema Collection**: Add schema collection logic for the parameter type
4. **Integrate with ApiCall**: Add methods to `ApiCall` builder
5. **Update Collectors**: Ensure collectors capture the new parameter type
6. **Add Tests**: Comprehensive unit tests with snapshot testing
7. **Update Documentation**: Add examples to module documentation
8. **Update Example**: Add usage example in `axum-example`
9. **Validate OpenAPI**: Run `mise spectral` to validate generated spec

### Adding New OpenAPI Features

When adding support for new OpenAPI features (e.g., security schemes, webhooks):

1. **Review utoipa Capabilities**: Check if utoipa already supports the feature
2. **Design Collection Strategy**: Plan how to collect the data during test execution
3. **Extend Collectors**: Add new collector types if needed
4. **Update Generator**: Modify OpenAPI generation logic
5. **Add Tests**: Test both collection and generation phases
6. **Validate Output**: Ensure generated OpenAPI is valid
7. **Update Documentation**: Document the new feature with examples

### Extending Query Parameter Serialization

When adding new query serialization styles (beyond form, spaceDelimited, etc.):

1. **Study OpenAPI Spec**: Understand the serialization format
2. **Extend CallQuery**: Add new method to `CallQuery` builder
3. **Implement Serialization**: Add serialization logic in `client/query.rs`
4. **Schema Generation**: Ensure proper schema is generated
5. **Add Tests**: Cover various data types with the new style
6. **Update Examples**: Show usage in example tests

### Before Coding
1. **Review existing patterns** in the codebase
2. **Check for similar implementations** already present
3. **Understand the OpenAPI specification** requirements
4. **Ask for clarification** if requirements are unclear

### Implementation Strategy
- **Direct modifications** - Edit files in place, don't create copies
- **Small incremental changes** - Avoid big rewrites
- **Follow established patterns** - Maintain consistency with existing builders
- **Update tests** along with implementation
- **Use snapshot testing** for complex OpenAPI output validation

### Code Review Checklist
- Rust compilation passes (`cargo check`)
- Tests updated and passing (`cargo nextest run`)
- Snapshots reviewed if changed (`mise run test:review`)
- No clippy warnings (`cargo clippy`)
- Formatted correctly (`cargo fmt`)
- Follows builder pattern conventions
- Proper error handling (no `unwrap()`)
- Documentation updated with examples
- Generated OpenAPI validated (`mise spectral`)

### Resource Management
- Do not run checks after every code change
- When a feature or fix seems complete, ask: "Do you want me to run checks now?"
- Perform checks pre-commit rather than at each step, unless explicitly requested

---

## AI Agent Guidelines

### When to Ask Questions
- Multiple valid approaches exist for OpenAPI generation
- Schema collection strategy is unclear
- Potential breaking changes to public API identified
- Architecture decisions needed (e.g., new collector types)
- OpenAPI specification interpretation ambiguous

### What NOT to Do
- Assume OpenAPI specification details without verification
- Create test files/copies for experimentation
- Suggest major architectural rewrites
- Use `unwrap()` or bypass error handling
- Test `Debug`, `Display`, or `Clone` implementations
- Add comments unless explicitly requested

### Optimal Response Pattern
1. Acknowledge the request and context
2. Review existing code if modifications are needed
3. Implement following established patterns (especially builder pattern)
4. Explain key decisions made (especially for schema collection)
5. Suggest running checks when feature is complete

### Checks and Resource Usage
- Do not run checks after every code change
- When a feature or fix seems complete, ask: "Do you want me to run checks (format, lint, test) now?"
- Typically perform checks pre-commit rather than at each step, unless explicitly requested

---

## Common Tasks Templates

### Adding a New Parameter Type

```rust
// lib/clawspec-core/src/client/cookies.rs

use std::collections::HashMap;
use serde::Serialize;
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};

/// Builder for cookie parameters
#[derive(Debug, Clone, Default)]
pub struct CallCookies {
    cookies: HashMap<String, serde_json::Value>,
}

impl CallCookies {
    /// Create a new cookie parameter builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cookie parameter
    pub fn with_cookie<K, V>(mut self, name: K, value: V) -> Self
    where
        K: Into<String>,
        V: Serialize,
    {
        let value = serde_json::to_value(value)
            .expect("Failed to serialize cookie value");
        self.cookies.insert(name.into(), value);
        self
    }

    /// Generate OpenAPI parameters
    pub fn to_openapi_params(&self) -> Vec<Parameter> {
        self.cookies
            .iter()
            .map(|(name, value)| {
                ParameterBuilder::new()
                    .name(name)
                    .parameter_in(ParameterIn::Cookie)
                    .required(utoipa::openapi::Required::True)
                    .schema(Some(schema_from_value(value)))
                    .build()
            })
            .collect()
    }
}
```

### Extending ApiCall Builder

```rust
// lib/clawspec-core/src/client/call.rs

impl ApiCall {
    /// Add cookie parameters to the call
    pub fn with_cookies(mut self, cookies: CallCookies) -> Self {
        self.cookies = Some(cookies);
        self
    }

    /// Add a single cookie parameter
    pub fn with_cookie<K, V>(mut self, name: K, value: V) -> Self
    where
        K: Into<String>,
        V: Serialize,
    {
        let cookies = self.cookies.take().unwrap_or_default();
        self.cookies = Some(cookies.with_cookie(name, value));
        self
    }
}
```

### Adding Collector Tests

```rust
// lib/clawspec-core/tests/cookies_test.rs

use clawspec_core::client::{ApiClient, CallCookies};
use insta::assert_json_snapshot;

#[tokio::test]
async fn should_collect_cookie_parameters() {
    let client = ApiClient::new("http://localhost:8080");

    client
        .call("GET", "/user/preferences")
        .with_cookie("session_id", "abc123")
        .with_cookie("theme", "dark")
        .send()
        .await
        .expect("Call should succeed");

    let openapi = client.generate_openapi_spec();

    // Snapshot test the cookie parameters in the generated spec
    assert_json_snapshot!(
        openapi.paths.paths["/user/preferences"].operations["get"].parameters,
        @r#"
        [
          {
            "name": "session_id",
            "in": "cookie",
            "required": true,
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "theme",
            "in": "cookie",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ]
        "#
    );
}
```

### Example Test Generating OpenAPI

```rust
// examples/axum-example/tests/generate_openapi.rs

use clawspec_core::client::ApiClient;
use std::fs;

#[tokio::test]
async fn generate_openapi() {
    let base_url = "http://localhost:8080";
    let client = ApiClient::new(base_url)
        .with_info("Observations API", "1.0.0")
        .with_description("API for managing observations");

    // Exercise all endpoints
    client
        .call("GET", "/observations")
        .with_query("limit", 10)
        .with_query("offset", 0)
        .send()
        .await
        .expect("List observations should succeed");

    client
        .call("GET", "/observations/{id}")
        .with_path("id", "obs-123")
        .send()
        .await
        .expect("Get observation should succeed");

    client
        .call("POST", "/observations")
        .with_json_body(&json!({
            "title": "New observation",
            "description": "Test"
        }))
        .send()
        .await
        .expect("Create observation should succeed");

    // Generate and save OpenAPI spec
    let openapi = client.generate_openapi_spec();
    let yaml = serde_saphyr::to_string(&openapi)
        .expect("Should serialize to YAML");

    fs::write("doc/openapi.yml", yaml)
        .expect("Should write OpenAPI file");
}
```

---

## Git Workflow

### Commit Policy

**IMPORTANT**: Claude must ALWAYS ask for explicit user confirmation before creating any git commits or pushes.

Process:

1. Make requested changes to files
2. Run necessary tests or checks
3. Show summary of changes
4. **ASK**: "Should I commit these changes?"
5. Wait for explicit user confirmation before running `git commit`

### Branching Strategy

- When working on a GitHub issue, start from up-to-date main branch
- Create a new working branch for the feature/fix (unless explicitly mentioned otherwise)
- Use descriptive branch names

### TODO Comments

Always include GitHub issue links in TODO comments:

```rust
// TODO: Add support for OAuth2 security schemes - https://github.com/ilaborie/clawspec/issues/42
```

---

## Context Hints for Better Assistance

- Assume senior Rust developer knowledge — focus on efficient solutions
- Prioritize maintainability over clever solutions
- Consider existing codebase patterns (especially builder pattern) before suggesting new approaches
- Validate at library boundaries, trust internal types
- Keep async/await throughout for consistency with Tokio runtime
- Prefer compile-time guarantees (type system) over runtime checks
- OpenAPI generation is test-driven - focus on what can be collected during test execution
- Snapshot testing is preferred for complex OpenAPI validation
- The library should be easy to use - prioritize ergonomics in builder APIs

---

## References

- Clawspec GitHub - https://github.com/ilaborie/clawspec
- OpenAPI Specification - https://spec.openapis.org/oas/latest.html
- utoipa Documentation - https://docs.rs/utoipa
- reqwest Documentation - https://docs.rs/reqwest
- Tokio Documentation - https://docs.rs/tokio
- insta (Snapshot Testing) - https://docs.rs/insta
- cargo-nextest - https://nexte.st/
- Claude Code Best Practices - https://www.anthropic.com/engineering/claude-code-best-practices

---

_This reference is optimized for AI-assisted development. Follow these patterns for consistent, maintainable code that aligns with project architecture and standards._
