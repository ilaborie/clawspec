# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Clawspec is a Rust library that generates OpenAPI specification files from tests. It allows developers to create API documentation by writing tests that exercise their API endpoints, automatically capturing request/response schemas and generating OpenAPI specifications.

## Architecture

The project is structured as a Rust workspace with the following key components:

### Core Libraries
- `clawspec-core/` - Core library functionality (currently placeholder)
- `clawspec-utoipa/` - Main implementation providing OpenAPI generation via utoipa integration
- `clawspec-macro/` - Procedural macros (currently placeholder)
- `clawspec-cli/` - Command-line interface (currently placeholder)

### Key Components in clawspec-utoipa
- `ApiClient` - Main client for making HTTP requests and collecting OpenAPI data
- `ApiCall` - Builder pattern for constructing API calls with path, query, headers, and body
- `CallPath` - Path parameter handling with automatic schema generation
- `Collectors` - Internal system for gathering OpenAPI paths and schemas from test execution

## Environment Setup

This project uses [mise](https://mise.jdx.dev/) for environment management and tooling setup.

### Initial Setup
```bash
# Install tools defined in .mise.toml
mise install
```

### Environment Variables
The project sets the following environment variables via mise:
- `RUST_LOG=info` - Default logging level
- `GIT_CLIFF_OUTPUT=CHANGELOG.md` - Changelog output file

### Installed Tools
- `cargo-nextest` - Enhanced test runner
- `git-cliff` - Changelog generator
- `@anthropic-ai/claude-code` - Claude Code CLI

## Common Commands

### Mise Tasks (Recommended)
```bash
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

### Run Example
```bash
# Run the axum example server
cargo run --bin axum-example

# Generate OpenAPI spec from tests
cargo test --package axum-example generate_openapi
```

### Code Style
This project follows strict Rust coding standards:
- `unsafe_code = "deny"`
- `missing_docs = "warn"`
- Comprehensive clippy lints including `pedantic` level warnings
- Tests allow `expect` usage via clippy configuration

## Development Workflow

1. **Testing Strategy**: The library uses a test-driven approach to OpenAPI generation. Tests in the examples use the `clawspec-utoipa` client to make API calls, which automatically collect schema information.

2. **Schema Collection**: The `ApiClient` collects OpenAPI paths and component schemas during test execution, then outputs complete OpenAPI specifications.

3. **Example Structure**: The `axum-example` demonstrates the full workflow:
   - API server implementation with axum
   - Test suite that exercises endpoints
   - Automatic OpenAPI generation to `doc/openapi.yml`

## Key Files

- `lib/clawspec-utoipa/src/client/mod.rs` - Main ApiClient implementation
- `examples/axum-example/tests/generate_openapi.rs` - Example test showing OpenAPI generation
- `examples/axum-example/src/observations/` - Sample API implementation

## Testing Dependencies

The project primarily uses:
- `rstest` for parameterized tests
- `tokio` for async testing
- `insta` for snapshot testing
- `assert2` for enhanced assertions

## Configuration

- Uses conventional commits for changelog generation (git-cliff)
- Rust edition 2024, minimum version 1.85
- Workspace-level dependency management for consistent versions