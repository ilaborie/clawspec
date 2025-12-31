# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.1](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.4.0...clawspec-core-v0.4.1) - 2025-12-31

### Added

- *(oauth2)* Add comprehensive tests for OAuth2 token acquisition
- *(yaml)* Add serde-saphyr support for YAML serialization (Issue #131)
- *(split)* Add OpenAPI specification splitting utilities (Issue #51) ([#129](https://github.com/ilaborie/clawspec/pull/129))

### Fixed

- Bump annotate-snippets minimum version for minimal-versions CI
- Update oauth2 crate to 5.0 API

### Other

- Reduce derive_more features from 'full' to specific macros
- Simplify and consolidate documentation

## [0.4.0](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.3.0...clawspec-core-v0.4.0) - 2025-12-30

### Added

- *(oauth2)* Add OAuth2 Client Credentials authentication support (Issue #93) ([#128](https://github.com/ilaborie/clawspec/pull/128))
- *(security)* Add OpenAPI security scheme support (Issue #23) ([#127](https://github.com/ilaborie/clawspec/pull/127))
- *(redaction)* Add redact_value for arbitrary JSON value redaction

### Other

- Fix formatting for macro in collectors.rs
- *(core)* Apply DRY and YAGNI principles to reduce code duplication

## [0.3.0](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.2.3...clawspec-core-v0.3.0) - 2025-12-20

### Added

- *(redaction)* Add trait-based Redactor API with closure support
- *(redaction)* Add wildcard support using JSONPath (RFC 9535)

### Fixed

- Replace removed `doc_auto_cfg` with `doc_cfg` for docs.rs ([#120](https://github.com/ilaborie/clawspec/pull/120))

### Other

- Add unit tests to improve code coverage
- *(redaction)* Add JSONPath wildcards, closures, and RedactOptions documentation

## [0.2.3](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.2.2...clawspec-core-v0.2.3) - 2025-12-15

### Fixed

- Fix rustdoc for docs.rs ([#118](https://github.com/ilaborie/clawspec/pull/118))

## [0.2.2](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.2.1...clawspec-core-v0.2.2) - 2025-12-13

### Added

- *(redaction)* use redacted values for OpenAPI examples ([#113](https://github.com/ilaborie/clawspec/pull/113))

### Other

- *(clawspec-core)* clean up public API and minimize external dependencies ([#117](https://github.com/ilaborie/clawspec/pull/117))
- *(clawspec-core)* replace Arc<RwLock<Collectors>> with channels ([#116](https://github.com/ilaborie/clawspec/pull/116))
- add comprehensive tutorial module for clawspec-core ([#115](https://github.com/ilaborie/clawspec/pull/115))

## [0.2.1](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.2.0...clawspec-core-v0.2.1) - 2025-11-11

### Added

- Add JSON response redaction support (fixes #100) ([#102](https://github.com/ilaborie/clawspec/pull/102))

### Other

- Fix DRY violations in JSON deserialization methods ([#108](https://github.com/ilaborie/clawspec/pull/108))
- Getting output as Result<T, E> ([#106](https://github.com/ilaborie/clawspec/pull/106))
- Create branch to fix issue #94 ([#105](https://github.com/ilaborie/clawspec/pull/105))
- Clawspec - Refactor code organization and structure ([#104](https://github.com/ilaborie/clawspec/pull/104))

## [0.2.0](https://github.com/ilaborie/clawspec/compare/clawspec-core-v0.1.4...clawspec-core-v0.2.0) - 2025-07-19

### Added

- Implement authentication support for API client ([#92](https://github.com/ilaborie/clawspec/pull/92))
- Implement cookie support in API client (fixes #18) ([#91](https://github.com/ilaborie/clawspec/pull/91))
- Add full OpenAPI 3.1.0 parameter styles support ([#90](https://github.com/ilaborie/clawspec/pull/90))
- Add OpenAPI response descriptions support ([#88](https://github.com/ilaborie/clawspec/pull/88))
- Enable method chaining for CallPath::add_param ([#86](https://github.com/ilaborie/clawspec/pull/86))

### Fixed

- Enable automatic JSON schema capture for request/response bodies ([#89](https://github.com/ilaborie/clawspec/pull/89))

### Added
- Authentication support with Bearer, Basic, and API Key methods
- Cookie support for comprehensive parameter handling
- Complete OpenAPI 3.1.0 parameter styles support

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
