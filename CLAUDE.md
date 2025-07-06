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
- `cargo-audit` - Security vulnerability scanner
- `cargo-nextest` - Enhanced test runner
- `git-cliff` - Changelog generator
- `@anthropic-ai/claude-code` - Claude Code CLI

## Common Commands

### Mise Tasks (Recommended)
```bash
# Run security audit with cargo audit
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
- **Type inference**: Prefer turbofish syntax (`::<T>()`) over variable type annotations for better readability
- **Derive ordering**: Use this order: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`, `Default`. Then for serde: `Serialize`, `Deserialize` (without `serde::` prefix). For other derives use fully qualified names like `sqlx::FromRow`, `derive_more::Debug`, or `clap::Parser`
- **Testing**: Prefer inline insta snapshot testing over multiple assertions. Use external snapshots if they exceed 10 lines

### TODO Comments and Issue Tracking
When adding TODO comments to the codebase, always include a link to the corresponding GitHub issue:
```rust
// TODO: Description of what needs to be done - https://github.com/ilaborie/clawspec/issues/N
```

This practice ensures:
- All TODOs are tracked and prioritized in GitHub issues
- Context and implementation details are documented in the issue
- Progress can be tracked and assigned to contributors
- TODOs don't get lost or forgotten over time

Before adding a new TODO, check if a relevant GitHub issue already exists, or create one with:
- Clear description of the problem/improvement
- Implementation guidance and acceptance criteria
- Priority level and complexity estimate

### Git Commit Policy
**IMPORTANT**: Claude must ALWAYS ask for explicit user confirmation before creating any git commits or pushes.

Never commit changes automatically, even if:
- The changes seem minor or obvious
- Tests are passing
- The user requested changes to files

Always follow this process:
1. Make the requested changes to files
2. Run any necessary tests or checks
3. Show a summary of what was changed
4. **ASK**: "Should I commit these changes?" or "Would you like me to create a commit?"
5. Wait for explicit user confirmation before running `git commit`

This ensures the user maintains full control over the git history and can review changes before they are committed.

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

## Testing Guidelines

- In Rust test, try to use inline insta snapshot testing instead of list of assertions. If the snapshot is above 10 lines long, the snapshot should not be inlined