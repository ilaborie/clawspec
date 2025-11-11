# Breaking Changes - v0.3.0

This document outlines all breaking changes in clawspec v0.3.0 and provides migration guidance.

## Summary of Changes

This release focuses on **aggressive cleanup** following DRY, KISS, and YAGNI principles:

1. ❌ **Removed unused redaction feature** (332 lines, zero usage)
2. ❌ **Removed `as_result_json` and `as_result_option_json` methods** (~680 lines, test-only usage)
3. ✨ **Improved code quality** - Fixed DRY violations, removed duplication

**Net Result**: ~1,010 lines removed, cleaner API surface, better maintainability.

---

## 1. Removed: JSON Redaction Feature

### What Changed

The entire JSON redaction feature has been removed:
- `as_json_redacted()` method
- `RedactedResult<T>` type
- `RedactionBuilder<T>` type
- `jsonptr` dependency
- `redaction` feature flag

### Rationale

- **Zero usage**: Not used anywhere in the codebase (examples, tests, production code)
- **YAGNI violation**: Added speculatively without proven need
- **Better alternatives exist**: `insta` snapshot testing already provides redaction

### Migration Guide

If you were using `as_json_redacted()`, switch to `insta`'s built-in redaction:

#### Before (v0.2.0)
```rust
#[cfg(feature = "redaction")]
use clawspec_core::{RedactedResult, RedactionBuilder};

let result = client
    .get("/api/users/123")?
    .await?
    .as_json_redacted::<User>().await?
    .redact_replace("/id", "stable-uuid")?
    .redact_replace("/created_at", "2024-01-01T00:00:00Z")?
    .finish();

// Test assertions use real value
assert_eq!(result.value.name, "John Doe");

// Snapshots use redacted value
insta::assert_yaml_snapshot!(result.redacted);
```

#### After (v0.3.0)
```rust
// Use insta's built-in redaction - simpler and more powerful
let user: User = client
    .get("/api/users/123")?
    .await?
    .as_json()
    .await?;

// Test assertions use real value
assert_eq!(user.name, "John Doe");

// Snapshots with redaction
insta::assert_yaml_snapshot!(user, {
    ".id" => "[uuid]",
    ".created_at" => "[timestamp]"
});
```

#### Alternative: Manual Redaction
```rust
let user: User = client.get("/api/users/123")?.await?.as_json().await?;

// Manually redact before snapshot
let mut user_json = serde_json::to_value(&user)?;
user_json["id"] = json!("stable-id");
user_json["created_at"] = json!("2024-01-01T00:00:00Z");

insta::assert_yaml_snapshot!(user_json);
```

**Why this is better:**
- No extra dependencies
- More flexible redaction options
- Standard approach used by Rust community
- `insta` supports patterns, regex, and custom redaction functions

---

## 2. Removed: `as_result_json` and `as_result_option_json`

### What Changed

Two methods for handling structured error responses have been removed:
- `CallResult::as_result_json::<T, E>()`
- `CallResult::as_result_option_json::<T, E>()`
- Internal helper `process_result_json_internal()`

### Rationale

- **Test-only usage**: All 13 uses were in test files, not production code
- **Marginal benefit**: Users can handle Result/Option logic themselves
- **KISS violation**: Added unnecessary complexity to the API
- **Questionable utility**: Most APIs don't return structured errors in both success and error cases

### Migration Guide

#### Scenario 1: Success/Error Responses (replacing `as_result_json`)

##### Before (v0.2.0)
```rust
#[derive(Deserialize, ToSchema)]
struct User {
    id: u32,
    name: String,
}

#[derive(Deserialize, ToSchema)]
struct ApiError {
    code: String,
    message: String,
}

let result: Result<User, ApiError> = client
    .get("/users/123")?
    .await?
    .as_result_json()
    .await?;

match result {
    Ok(user) => println!("User: {}", user.name),
    Err(err) => println!("Error: {} - {}", err.code, err.message),
}
```

##### After (v0.3.0) - Option A: Use status codes
```rust
use http::StatusCode;

let response = client.get("/users/123")?.await?;

if response.status == StatusCode::OK {
    let user: User = response.as_json().await?;
    println!("User: {}", user.name);
} else {
    // Handle error cases
    let err: ApiError = response.as_json().await?;
    println!("Error: {} - {}", err.code, err.message);
}
```

##### After (v0.3.0) - Option B: Use error handling
```rust
match client.get("/users/123")?.await?.as_json::<User>().await {
    Ok(user) => println!("User: {}", user.name),
    Err(ApiClientError::UnexpectedStatusCode { body, .. }) => {
        // Parse error response if API returns structured errors
        if let Ok(api_err) = serde_json::from_str::<ApiError>(&body) {
            println!("Error: {} - {}", api_err.code, api_err.message);
        }
    }
    Err(e) => println!("Request failed: {}", e),
}
```

##### After (v0.3.0) - Option C: Custom helper (reusable)
```rust
// Create a reusable helper for your API
async fn api_call<T, E>(
    response: CallResult
) -> Result<Result<T, E>, ApiClientError>
where
    T: DeserializeOwned + ToSchema + 'static,
    E: DeserializeOwned + ToSchema + 'static,
{
    if response.status.is_success() {
        Ok(Ok(response.as_json().await?))
    } else {
        Ok(Err(response.as_json().await?))
    }
}

// Usage
let result: Result<User, ApiError> = api_call(
    client.get("/users/123")?.await?
).await?;
```

#### Scenario 2: Optional Responses with Errors (replacing `as_result_option_json`)

##### Before (v0.2.0)
```rust
let result: Result<Option<User>, ApiError> = client
    .get("/users/123")?
    .await?
    .as_result_option_json()
    .await?;

match result {
    Ok(Some(user)) => println!("User: {}", user.name),
    Ok(None) => println!("User not found"),
    Err(err) => println!("Error: {} - {}", err.code, err.message),
}
```

##### After (v0.3.0) - Use `as_optional_json` (still available!)
```rust
// as_optional_json handles 204/404 as None automatically
let user: Option<User> = client
    .get("/users/123")?
    .await?
    .as_optional_json()
    .await?;

match user {
    Some(user) => println!("User: {}", user.name),
    None => println!("User not found"),
}

// For error handling, use standard Rust error handling
match client.get("/users/123")?.await?.as_optional_json::<User>().await {
    Ok(Some(user)) => println!("User: {}", user.name),
    Ok(None) => println!("User not found"),
    Err(e) => println!("Request failed: {}", e),
}
```

### Impact Assessment

**Who is affected:**
- Users who used `as_result_json` or `as_result_option_json`
- Based on analysis, this was **test-only usage** in our codebase

**Who is NOT affected:**
- Users using `as_json()` ✅ Still available
- Users using `as_optional_json()` ✅ Still available
- Users using `as_text()`, `as_bytes()`, `as_raw()`, `as_empty()` ✅ All still available

---

## 3. Internal Improvements (No Breaking Changes)

### Fixed DRY Violations

**What changed internally:**
- `as_json()` and `as_optional_json()` now use shared `deserialize_and_record()` helper
- Eliminated ~200 lines of duplicated JSON deserialization logic
- Improved consistency in error handling

**User impact:** ✅ **None** - These are internal refactorings, the public API works exactly the same.

---

## Upgrade Checklist

### Step 1: Remove Redaction References

```bash
# Search for redaction usage
rg "as_json_redacted|RedactedResult|RedactionBuilder" --type rust
```

If found:
- Remove `feature = "redaction"` from `Cargo.toml`
- Replace with `insta` redaction (see migration guide above)

### Step 2: Replace Result JSON Methods

```bash
# Search for Result JSON methods
rg "as_result_json|as_result_option_json" --type rust
```

If found:
- Replace with status code checking or custom helpers (see migration guide above)
- Consider if `as_optional_json()` meets your needs

### Step 3: Update Dependencies

```toml
# Cargo.toml
[dependencies]
clawspec-core = "0.3.0"  # Update version

# Remove if present
# clawspec-core = { version = "0.2.0", features = ["redaction"] }
```

### Step 4: Run Tests

```bash
cargo test
```

---

## Rationale: Why These Changes?

### Adherence to Principles

This release focuses on adherence to fundamental software engineering principles:

**DRY (Don't Repeat Yourself)**
- ✅ Eliminated 200+ lines of duplicated JSON deserialization logic
- ✅ Consolidated error handling patterns

**KISS (Keep It Simple, Stupid)**
- ✅ Removed 4 JSON methods, kept 2 essential ones
- ✅ Simpler API surface = easier to learn and use
- ✅ Users can implement custom wrappers for complex cases

**YAGNI (You Aren't Gonna Need It)**
- ✅ Removed redaction feature (0 usage, 332 lines)
- ✅ Removed Result JSON methods (test-only usage, ~400 lines)
- ✅ Wait for real-world demand before adding features

### Benefits

1. **Smaller codebase**: ~1,010 lines removed (-27% from recent additions)
2. **Fewer dependencies**: Removed `jsonptr` dependency
3. **Clearer API**: Fewer methods to choose from
4. **Better maintainability**: Less code to maintain and test
5. **Faster compilation**: Fewer dependencies and less code
6. **Better focus**: Core functionality is clearer

### Project Philosophy Alignment

From `CLAUDE.md`:
> - Follow DRY, KISS, and YAGNI
> - Simplicity over cleverness
> - Don't add features until needed

These changes bring the codebase back in alignment with stated principles.

---

## Questions?

If you have questions about migrating or disagree with these changes:
- Open an issue: https://github.com/ilaborie/clawspec/issues
- Start a discussion: https://github.com/ilaborie/clawspec/discussions

We're happy to help with migration and hear feedback on the direction!

---

## Version History

- **v0.3.0** (2025-MM-DD): Aggressive cleanup - removed redaction and Result JSON methods
- **v0.2.0** (2025-07-19): Added redaction, Result JSON methods, refactored organization
- **v0.1.4** (2025-07-17): Stable release with core functionality
