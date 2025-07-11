# Performance Regression Tests

This document describes the performance regression testing framework implemented to ensure that performance optimizations from Issue #31 are maintained over time.

## Overview

The performance regression tests are designed to catch performance regressions early in the development process by:

1. **Benchmarking critical operations** that were optimized in Issue #31
2. **Establishing performance baselines** based on measured improvements
3. **Providing automated validation** of performance characteristics
4. **Monitoring allocation patterns** to prevent memory usage regressions

## Test Structure

### Core Performance Tests

The regression tests are organized into several categories:

#### 1. Path Replacement Regression Tests
- **File**: `benches/performance_regression.rs`
- **Purpose**: Ensure path parameter replacement maintains >20% improvement over format!() approach
- **Test Cases**:
  - Simple paths: `/users/{id}` → `/users/123`
  - Complex paths: `/api/{version}/users/{id}/posts/{post_id}`
  - Duplicate parameters: `/test/{id}/{id}`
  - Special characters: paths requiring URL encoding

#### 2. Collection Operations Regression Tests
- **Purpose**: Validate HashSet vs Vec performance for parameter operations
- **Test Cases**:
  - Parameter lookup and removal operations
  - Scaling behavior with parameter count
  - O(1) vs O(n) complexity validation

#### 3. String Allocation Regression Tests
- **Purpose**: Monitor string allocation patterns in hot paths
- **Test Cases**:
  - String concatenation vs format!() macro
  - Bulk string operations
  - Memory allocation efficiency

#### 4. API Client Regression Tests
- **Purpose**: End-to-end performance validation
- **Test Cases**:
  - Client builder operations
  - Path creation with parameters
  - Complex API call setup

### Original Performance Comparison Tests

The existing benchmarks continue to provide detailed performance comparisons:

#### 1. Path Replacement Benchmarks
- **File**: `benches/path_replacement.rs`
- **Purpose**: Direct comparison between original and optimized implementations
- **Results**: Should show 17-35% improvement consistently

#### 2. Performance Benchmarks
- **File**: `benches/performance.rs`
- **Purpose**: Comprehensive performance analysis of various operations

## Performance Baselines

### Established Thresholds

Based on Issue #31 optimizations, the following performance thresholds should be maintained:

| Operation | Baseline | Threshold | Notes |
|-----------|----------|-----------|-------|
| Simple path replacement | <50ns | >20% faster than format!() | Basic {param} substitution |
| Complex path replacement | <100ns | >20% faster than format!() | Multiple parameters |
| Parameter lookup | O(1) | Faster than Vec::retain() | HashSet vs Vec |
| Schema merge | <1μs | Linear scaling | Typical schema counts |
| String concatenation | ~30ns | Faster than format!() | Simple patterns |

### Performance Validation

The regression tests validate that:

1. **Path replacement** operations maintain their 17-35% performance improvement
2. **Collection operations** use HashSet (O(1)) instead of Vec::retain() (O(n))
3. **String operations** avoid unnecessary allocations
4. **Schema operations** scale appropriately with data complexity

## Running Performance Tests

### Quick Performance Check

```bash
# Run all performance regression tests
mise run perf-regression

# Run specific benchmark suites
cargo bench --bench performance_regression
cargo bench --bench path_replacement
cargo bench --bench performance
```

### Detailed Performance Analysis

```bash
# Run with detailed output
cargo bench --bench performance_regression -- --verbose

# Generate HTML reports
cargo bench --bench performance_regression
# View results in: target/criterion/*/report/index.html
```

### CI/CD Integration

The performance regression tests can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run Performance Regression Tests
  run: |
    cargo bench --bench performance_regression
    # Parse results and fail if thresholds are exceeded
```

## Interpreting Results

### Performance Metrics

When analyzing benchmark results, look for:

1. **Regression Indicators**:
   - Execution time increases >10% from baseline
   - Memory allocation increases
   - Algorithmic complexity changes (O(1) → O(n))

2. **Performance Improvements**:
   - Maintained 17-35% improvement in path replacement
   - HashSet operations consistently faster than Vec operations
   - String concatenation faster than format!() for simple patterns

### Result Analysis

#### Expected Results
```
path_replacement/original_0    time: [61.5 ns ...]
path_replacement/optimized_0   time: [47.1 ns ...]  (23% faster)
```

#### Regression Warning Signs
```
path_replacement/optimized_0   time: [65.0 ns ...]  (slower than baseline)
collection_regression/vec_*    time: [faster than hashset_*]  (wrong algorithm)
```

## Maintenance Guidelines

### Adding New Performance Tests

When adding new performance-critical features:

1. **Establish baseline** measurements before optimization
2. **Add regression tests** to prevent future regressions
3. **Document expected** performance characteristics
4. **Update thresholds** based on measured improvements

### Updating Performance Baselines

When legitimate performance improvements are made:

1. **Measure improvements** with comprehensive benchmarks
2. **Update baseline** values in regression tests
3. **Document changes** in performance characteristics
4. **Validate** that improvements are maintained

## Integration with Development Workflow

### Pre-commit Hooks

Consider adding performance regression tests to pre-commit hooks:

```bash
# .pre-commit-config.yaml
- repo: local
  hooks:
    - id: performance-regression
      name: Performance Regression Tests
      entry: cargo bench --bench performance_regression
      language: system
      pass_filenames: false
```

### Pull Request Validation

For critical performance changes:

1. **Run regression tests** before submitting PR
2. **Include performance** impact in PR description
3. **Validate baselines** are maintained or improved
4. **Document** any performance characteristic changes

## Troubleshooting

### Common Issues

1. **Benchmark Compilation Errors**:
   - Ensure all dependencies are available
   - Check that benchmarks don't access private modules
   - Verify trait bounds are satisfied

2. **Performance Degradation**:
   - Check if optimizations were accidentally removed
   - Verify algorithm complexity hasn't changed
   - Review memory allocation patterns

3. **Inconsistent Results**:
   - Run benchmarks multiple times
   - Check system load during benchmarking
   - Ensure consistent hardware/software environment

### Performance Debugging

When performance regressions are detected:

1. **Profile the code** to identify bottlenecks
2. **Compare with baseline** implementations
3. **Check for algorithmic** complexity changes
4. **Review memory allocation** patterns
5. **Validate optimizations** are still in place

## References

- [Issue #31: Optimize performance in path handling and schema operations](https://github.com/ilaborie/clawspec/issues/31)
- [Performance Improvements Documentation](../PERFORMANCE_IMPROVEMENTS.md)
- [Criterion.rs Benchmarking Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)