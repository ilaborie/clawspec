# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Clawspec is a Rust library that generates OpenAPI specification files from tests. It allows developers to create API documentation by writing tests that exercise their API endpoints, automatically capturing request/response schemas and generating OpenAPI specifications.

## Architecture

The project is structured as a Rust workspace with the following components:

### Core Libraries
- `clawspec-core/` - Core library with HTTP client and OpenAPI generation functionality
- `clawspec-macro/` - Procedural macros (future: URI templating support)
- `clawspec-cli/` - Command-line interface (placeholder)

### Key Components in clawspec-core
- `ApiClient` - Main client for making HTTP requests and collecting OpenAPI data
- `ApiCall` - Builder pattern for constructing API calls with path, query, headers, and body
- `CallPath` - Path parameter handling with automatic schema generation
- `CallQuery` - Query parameter handling with multiple serialization styles
- `CallHeaders` - HTTP header parameter handling
- `Collectors` - Internal system for gathering OpenAPI paths and schemas from test execution

## Environment Setup

This project uses [mise](https://mise.jdx.dev/) for environment management and tooling setup.

### Initial Setup
```bash
# Install tools defined in .mise.toml
mise install
```

### Environment Variables
- `RUST_LOG=info` - Default logging level
- `GIT_CLIFF_OUTPUT=CHANGELOG.md` - Changelog output file

### Installed Tools
- `cargo-audit` - Security vulnerability scanner
- `cargo-nextest` - Enhanced test runner
- `git-cliff` - Changelog generator
- `@anthropic-ai/claude-code` - Claude Code CLI

## Development Commands

### Mise Tasks (Recommended)
```bash
# Run security audit
mise run audit

# Run comprehensive pre-push checks: format, lint, and test
mise run check

# Auto-format code and apply clippy suggested fixes
mise run fix

# Run clippy linting with strict warnings as errors
mise run lint

# Run all tests using nextest and include doctests
mise run test

# Run insta snapshot tests with nextest and review changes
mise run test:review

# Lint generated OpenAPI files
mise spectral
```

### Direct Cargo Commands
```bash
# Build
cargo build

# Test
cargo test
# Using nextest (recommended)
cargo nextest run

# Linting
cargo clippy

# Generate changelog
git cliff
```

### Example Usage
```bash
# Run the axum example server
cargo run --bin axum-example

# Generate OpenAPI spec from tests
cargo test --package axum-example generate_openapi
```

## Code Standards and Guidelines

### Rust Coding Standards
- `unsafe_code = "deny"`
- `missing_docs = "warn"`
- Comprehensive clippy lints including `pedantic` level warnings
- Tests allow `expect` usage via clippy configuration

### Code Style Preferences
- **Type inference**: Prefer turbofish syntax (`::<T>()`) over variable type annotations
- **Derive ordering**: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`, `Default`. Then serde: `Serialize`, `Deserialize` (without `serde::` prefix). For other derives use fully qualified names like `derive_more::Debug`
- **Error handling**: Avoid `unwrap()`, use `expect("message")` in tests, proper error handling in production code
- **String types**: Prefer `&str` over `String` when possible
- **Memory**: Prefer `as_ref()` over `.clone()` unless necessary

### Testing Guidelines
- Use inline insta snapshot testing instead of multiple assertions
- Use external snapshots if they exceed 10 lines
- Do not test `Debug`, `Display`, and `Clone` behavior in unit tests
- Use `expect()` instead of `unwrap()` in test code

### Documentation Guidelines
- Avoid mentioning specific OpenAPI version numbers in documentation
- All public APIs should have comprehensive documentation
- Include examples in documentation

## Git Workflow

### Commit Policy
**IMPORTANT**: Claude must ALWAYS ask for explicit user confirmation before creating any git commits or pushes.

Process:
1. Make requested changes to files
2. Run necessary tests or checks
3. Show summary of changes
4. **ASK**: "Should I commit these changes?"
5. Wait for explicit user confirmation before running `git commit`

### TODO Comments and Issue Tracking
Always include GitHub issue links in TODO comments:
```rust
// TODO: Description of what needs to be done - https://github.com/ilaborie/clawspec/issues/N
```

Benefits:
- All TODOs are tracked and prioritized in GitHub issues
- Context and implementation details documented
- Progress can be tracked and assigned
- TODOs don't get lost over time

## Development Workflow

### Testing Strategy
The library uses a test-driven approach to OpenAPI generation:
1. Tests exercise API endpoints using `clawspec-core` client
2. Client automatically collects schema information during test execution
3. Complete OpenAPI specifications are generated from collected data

### Example Structure
The `axum-example` demonstrates the full workflow:
- API server implementation with axum
- Test suite that exercises endpoints
- Automatic OpenAPI generation to `doc/openapi.yml`

## Key Files and Locations

### Core Implementation
- `lib/clawspec-core/src/client/mod.rs` - Main ApiClient implementation
- `lib/clawspec-core/src/client/path.rs` - Path parameter handling
- `lib/clawspec-core/src/client/query.rs` - Query parameter handling
- `lib/clawspec-core/src/client/headers.rs` - Header parameter handling

### Examples and Tests
- `examples/axum-example/tests/generate_openapi.rs` - Example test showing OpenAPI generation
- `examples/axum-example/src/observations/` - Sample API implementation

## Configuration and Dependencies

### Project Configuration
- Rust edition 2024, minimum version 1.88
- Workspace-level dependency management for consistent versions
- Uses conventional commits for changelog generation (git-cliff)

### Primary Dependencies
- `utoipa` - OpenAPI schema generation
- `reqwest` - HTTP client
- `serde` - Serialization/deserialization
- `tokio` - Async runtime

### Testing Dependencies
- `rstest` - Parameterized tests
- `insta` - Snapshot testing
- `assert2` - Enhanced assertions

## Quality Assurance

### Pre-commit Checks
Always run `mise check` after completing work to ensure:
- Code formatting is correct
- Linting passes without warnings
- All tests pass
- Generated OpenAPI files are valid

### CI/Development Best Practices
- Fix all issues identified by `mise check`
- Use `mise spectral` to validate generated OpenAPI files
- Run comprehensive test suite before submitting changes
- Maintain high test coverage