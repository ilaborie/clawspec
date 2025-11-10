# ResultCollector Trait Implementation Plan

## Overview
Implement a `ResultCollector` trait to abstract the transformation of `CallResult` into various output types, reducing code duplication and providing a cleaner architecture for result processing.

## Current State Analysis

### Existing Methods
The current `CallResult` implementation has the following methods:
- `as_json<T>()` - Deserializes JSON responses
- `as_text()` - Extracts text responses 
- `as_bytes()` - Extracts binary responses
- `as_raw()` - Returns complete raw response data
- `as_empty()` - Processes empty responses

### Issues with Current Design
1. **Code Duplication**: Each method repeats similar patterns for schema collection
2. **Inflexible Error Handling**: Each method has specialized error variants
3. **No Extensibility**: Cannot easily add new response processors
4. **Tight Coupling**: Response processing logic is tightly coupled to CallResult

## Design Architecture

### Core Trait Definition
```rust
pub trait ResultCollector {
    type Output;
    type Err: std::error::Error + Send + Sync + 'static;

    fn collect(&mut self, call_result: CallResult) -> Result<Self::Output, Self::Err>;
}
```

### Concrete Collectors

#### 1. JsonCollector
```rust
pub struct JsonCollector<T> {
    _phantom: PhantomData<T>,
}

impl<T> ResultCollector for JsonCollector<T>
where
    T: DeserializeOwned + ToSchema + 'static,
{
    type Output = T;
    type Err = JsonCollectorError;

    fn collect(&mut self, call_result: CallResult) -> Result<T, JsonCollectorError>;
}
```

#### 2. TextCollector
```rust
pub struct TextCollector;

impl ResultCollector for TextCollector {
    type Output = String;
    type Err = TextCollectorError;

    fn collect(&mut self, call_result: CallResult) -> Result<String, TextCollectorError>;
}
```

#### 3. BytesCollector
```rust
pub struct BytesCollector;

impl ResultCollector for BytesCollector {
    type Output = Vec<u8>;
    type Err = BytesCollectorError;

    fn collect(&mut self, call_result: CallResult) -> Result<Vec<u8>, BytesCollectorError>;
}
```

#### 4. RawCollector
```rust
pub struct RawCollector;

impl ResultCollector for RawCollector {
    type Output = RawResult;
    type Err = RawCollectorError;

    fn collect(&mut self, call_result: CallResult) -> Result<RawResult, RawCollectorError>;
}
```

#### 5. EmptyCollector
```rust
pub struct EmptyCollector;

impl ResultCollector for EmptyCollector {
    type Output = ();
    type Err = EmptyCollectorError;

    fn collect(&mut self, call_result: CallResult) -> Result<(), EmptyCollectorError>;
}
```

### Error Handling Strategy

#### Generic Collector Error
```rust
#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum CollectorError {
    #[display("JSON collector error: {0}")]
    Json(JsonCollectorError),
    
    #[display("Text collector error: {0}")]
    Text(TextCollectorError),
    
    #[display("Bytes collector error: {0}")]
    Bytes(BytesCollectorError),
    
    #[display("Raw collector error: {0}")]
    Raw(RawCollectorError),
    
    #[display("Empty collector error: {0}")]
    Empty(EmptyCollectorError),
}
```

#### Add to ApiClientError
```rust
/// Result collector processing error
#[display("Collector error: {error}")]
#[from]
CollectorError(CollectorError),
```

### CallResult Integration

#### New collect() Method
```rust
impl CallResult {
    pub async fn collect<C>(&mut self, mut collector: C) -> Result<C::Output, ApiClientError>
    where
        C: ResultCollector,
    {
        collector.collect(self).map_err(|e| {
            ApiClientError::CollectorError(/* convert error appropriately */)
        })
    }
}
```

#### Refactored as_* Methods
```rust
impl CallResult {
    pub async fn as_json<T>(&mut self) -> Result<T, ApiClientError>
    where
        T: DeserializeOwned + ToSchema + 'static,
    {
        self.collect(JsonCollector::<T>::new()).await
    }

    pub async fn as_text(&mut self) -> Result<String, ApiClientError> {
        self.collect(TextCollector).await
    }

    pub async fn as_bytes(&mut self) -> Result<Vec<u8>, ApiClientError> {
        self.collect(BytesCollector).await
    }

    pub async fn as_raw(&mut self) -> Result<RawResult, ApiClientError> {
        self.collect(RawCollector).await
    }

    pub async fn as_empty(&mut self) -> Result<(), ApiClientError> {
        self.collect(EmptyCollector).await
    }
}
```

## Implementation Strategy

### Phase 1: Create the Results Module
1. Create `lib/clawspec-core/src/client/results.rs`
2. Define the `ResultCollector` trait
3. Create all specific collector error types
4. Add the generic `CollectorError` type

### Phase 2: Implement Concrete Collectors
1. Implement `JsonCollector<T>`
2. Implement `TextCollector`
3. Implement `BytesCollector`
4. Implement `RawCollector`
5. Implement `EmptyCollector`

### Phase 3: Update Error Handling
1. Add `CollectorError` variant to `ApiClientError`
2. Implement proper error conversion from collector errors

### Phase 3: Refactor CallResult
1. Add the generic `collect()` method to `CallResult`
2. Refactor existing `as_*` methods to use collectors internally
3. Ensure backward compatibility

### Phase 4: Testing & Documentation
1. Write comprehensive unit tests for each collector
2. Write integration tests for the new collect() method
3. Update documentation with examples
4. Test error propagation

## Benefits of This Design

### DRY (Don't Repeat Yourself)
- Eliminates code duplication across as_* methods
- Centralizes response processing logic
- Reduces maintenance overhead

### KISS (Keep It Simple, Stupid)
- Clear separation of concerns
- Simple trait interface
- Straightforward error handling

### YAGNI (You Aren't Gonna Need It)
- Only implements currently needed functionality
- Extensible design for future needs
- No over-engineering

### Additional Benefits
1. **Extensibility**: Easy to add new response processors
2. **Testability**: Each collector can be tested independently
3. **Reusability**: Collectors can be reused across different contexts
4. **Type Safety**: Strong typing through generic parameters
5. **Error Clarity**: Specific error types for each collector

## Backward Compatibility

The existing `as_*` methods will remain unchanged in their public API. They will internally use the new collectors, ensuring that existing code continues to work without modification.

## File Structure

```
lib/clawspec-core/src/client/
├── results.rs           # New module containing ResultCollector trait and implementations
├── collectors.rs        # Updated to use new results module 
├── error.rs            # Updated with CollectorError
└── mod.rs              # Updated exports
```

## Example Usage

```rust
// Current usage (still works)
let user: User = call_result.as_json().await?;

// New explicit collector usage  
let user: User = call_result.collect(JsonCollector::<User>::new()).await?;

// Custom collector implementation
struct CustomCollector;
impl ResultCollector for CustomCollector {
    type Output = CustomType;
    type Err = CustomError;
    
    fn collect(&mut self, call_result: CallResult) -> Result<CustomType, CustomError> {
        // Custom processing logic
    }
}

let custom_result = call_result.collect(CustomCollector).await?;
```