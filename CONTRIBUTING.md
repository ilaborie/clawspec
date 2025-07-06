# Contributing to Clawspec

Thank you for your interest in contributing to Clawspec! This document provides guidelines and information for contributors.

## Code of Conduct

This project adheres to a Code of Conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (version 1.85 or later)
- [mise](https://mise.jdx.dev/) for development tooling
- Git

### Development Setup

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/yourusername/clawspec.git
   cd clawspec
   ```

2. **Install development tools**
   ```bash
   mise install
   ```

3. **Run tests to verify setup**
   ```bash
   mise run test
   ```

## Development Workflow

### Available Tasks

We use mise for task management. Here are the main development tasks:

```bash
# Run all checks (format, lint, test)
mise run check

# Auto-format code and apply clippy fixes
mise run fix

# Run security audit
mise run audit

# Run tests with snapshot review
mise run test:review
```

### Making Changes

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow the existing code style
   - Add tests for new functionality
   - Update documentation as needed

3. **Run quality checks**
   ```bash
   mise run check
   ```

4. **Commit your changes**
   ```bash
   git commit -m "feat: add your feature description"
   ```
   
   Use [Conventional Commits](https://conventionalcommits.org/) format:
   - `feat:` for new features
   - `fix:` for bug fixes
   - `docs:` for documentation changes
   - `test:` for test additions/changes
   - `refactor:` for code refactoring
   - `chore:` for maintenance tasks

5. **Push and create a Pull Request**
   ```bash
   git push origin feature/your-feature-name
   ```

## Code Style

- **Formatting**: Code is automatically formatted with `rustfmt`
- **Linting**: Use `cargo clippy` with strict warnings
- **Documentation**: Add doc comments for public APIs
- **Tests**: Include tests for new functionality

### Code Quality Standards

- All clippy warnings must be resolved
- Code coverage should not decrease
- New public APIs must be documented
- Breaking changes require justification and migration guide

## Testing

### Running Tests

```bash
# Run all tests
mise run test

# Run tests with coverage
cargo tarpaulin --out html

# Run specific test
cargo test test_name

# Run tests for specific package
cargo test -p clawspec-core
```

### Test Guidelines

- Write unit tests for individual functions
- Write integration tests for workflows
- Use snapshot testing for OpenAPI generation
- Include error case testing
- Mock external dependencies appropriately

## Documentation

### Code Documentation

- Add rustdoc comments for all public items
- Include examples in documentation
- Document error conditions and panics
- Keep examples up to date

### Project Documentation

- Update README.md for user-facing changes
- Update CLAUDE.md for development setup changes
- Add examples for new features

## Submitting Changes

### Pull Request Process

1. **Fill out the PR template** completely
2. **Link related issues** using "Fixes #123"
3. **Ensure CI passes** - all checks must be green
4. **Request review** from maintainers
5. **Address feedback** promptly

### PR Requirements

- [ ] Descriptive title and description
- [ ] Tests added/updated for changes
- [ ] Documentation updated if needed
- [ ] No merge conflicts
- [ ] CI checks pass
- [ ] Code review approved

## Release Process

Releases are automated:

1. **Version updates** happen via PR to main
2. **Tags trigger releases** (format: `v1.2.3`)
3. **Changelogs** are generated automatically
4. **Crates.io publishing** happens automatically for stable releases

## Architecture Guidelines

### Project Structure

- `lib/clawspec-core/` - Core functionality
- `lib/clawspec-utoipa/` - utoipa integration
- `lib/clawspec-macro/` - Procedural macros
- `lib/clawspec-cli/` - Command-line interface
- `examples/` - Usage examples

### Design Principles

- **Test-driven OpenAPI generation** - Generate specs from tests
- **Framework agnostic** - Support multiple web frameworks
- **Developer friendly** - Intuitive APIs and good error messages
- **Performance conscious** - Minimal runtime overhead

## Getting Help

- **Documentation**: Check README.md and rustdocs
- **Issues**: Search existing issues before creating new ones
- **Discussions**: Use GitHub Discussions for questions
- **Code review**: Maintainers will review PRs promptly

## Recognition

Contributors will be recognized in:
- CHANGELOG.md for significant contributions
- GitHub contributors list
- Release notes for major features

Thank you for contributing to Clawspec! 🚀