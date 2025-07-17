# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.1.3...clawspec-core-v0.1.4) - 2025-07-17

### Fixed

- Remove double slashes from URL building ([#84](https://github.com/ilaborie/clawspec/pull/84))

## [0.1.3](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.1.2...clawspec-core-v0.1.3) - 2025-07-15

### Fixed

- Update README documentation with correct crate names and imports ([#81](https://github.com/ilaborie/clawspec/pull/81))

### Other

- Update README.md

## [0.1.2](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.1.1...clawspec-core-v0.1.2) - 2025-07-15

### Added

- Add documentation and code cleanup improvements ([#79](https://github.com/ilaborie/clawspec/pull/79))

## [0.1.1](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.1.0...clawspec-core-v0.1.1) - 2025-07-14

### Other

- release v0.1.0 ([#75](https://github.com/ilaborie/clawspec/pull/75))

## [0.1.0](https://github.com/ilaborie/clawspec/releases/tag/clawspec-core-v0.1.0) - 2025-07-14

### Added

- Add without_collection() method to exclude API calls from OpenAPI documentation ([#73](https://github.com/ilaborie/clawspec/pull/73))
- Implement IntoFuture for ApiCall to enable ergonomic .await syntax ([#72](https://github.com/ilaborie/clawspec/pull/72))
- Add comprehensive API documentation and improve codebase quality ([#69](https://github.com/ilaborie/clawspec/pull/69))
- Add configurable max retry attempts and fix lint issues
- Implement Result-based TestServer API with HealthStatus enum
- Implement comprehensive generic TestApp framework for async server testing
- Add OpenAPI info and servers metadata with comprehensive builder API ([#62](https://github.com/ilaborie/clawspec/pull/62))
- Add declarative macro for ExpectedStatusCodes with range syntax ([#61](https://github.com/ilaborie/clawspec/pull/61))
- Resolve schema name conflicts and enhance example generation (closes #25)
- Standardize builder patterns and enhance developer experience (Issue #33) ([#58](https://github.com/ilaborie/clawspec/pull/58))
- Add comprehensive unit tests and implement Rust coding standards
- [**breaking**] Implements a prototype

### Other

- Add missing license and description fields to Cargo.toml files ([#74](https://github.com/ilaborie/clawspec/pull/74))
- Add comprehensive error path testing and improve test coverage
- Improve TestServer API and fix implementation details
- Add comprehensive merge behavior safety test
- Address code review feedback and optimize schema performance
- 🧹 Cleanup: Resolve workspace structure and library implementations ([#56](https://github.com/ilaborie/clawspec/pull/56))
