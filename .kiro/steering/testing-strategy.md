---
inclusion: always
---

# Testing Strategy

## Testing Philosophy

Network measurement tools require high accuracy and reliability. Our testing strategy focuses on:
1. **Correctness** - Measurements must be accurate
2. **Reliability** - Tests should handle network variability
3. **Performance** - Minimize overhead in measurement code
4. **Maintainability** - Tests should be easy to understand and update

## Test Organization

### Unit Tests
Place unit tests in the same file as the implementation:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_median_calculation() {
        // Test implementation
    }
}
```

### Integration Tests
Place integration tests in module-specific `tests/` directories:
- `src/cloudflare/tests/` - Tests for API interactions

## What to Test

### Statistical Functions (`src/stats.rs`)
- Test with known inputs and expected outputs
- Test edge cases: empty vectors, single element, even/odd lengths
- Test with both integer and floating-point data
- Verify sorting doesn't mutate original data when it shouldn't

### Measurement Functions (`src/measurements.rs`)
- Test latency calculation with known durations
- Test jitter calculation with controlled variance
- Test with empty and single-element vectors
- Verify async behavior works correctly

### API Client (`src/cloudflare/client.rs`)
- Mock HTTP responses for predictable testing
- Test error handling for network failures
- Test JSON and plain text response parsing
- Verify headers are set correctly

### Request Types
- Test endpoint generation
- Test header construction
- Test body serialization
- Verify request method is correct

## Testing Network Operations

Network operations are inherently variable. When testing:
- **Mock external dependencies** - Don't rely on actual network calls in unit tests
- **Use fixtures** - Create sample response data for testing
- **Test error paths** - Simulate network failures, timeouts, invalid responses
- **Avoid flaky tests** - Don't assert exact timing values, use ranges or mocks

## Property-Based Testing

For statistical functions, consider property-based testing:
- Median should always be between min and max
- Sorting should preserve all elements
- Quartile values should be monotonically increasing
- Jitter should always be non-negative

## Test Data

When creating test data:
- Use realistic values that match actual network measurements
- Include edge cases: very fast (< 1ms), very slow (> 1000ms)
- Test with various data distributions: uniform, skewed, bimodal
- Consider floating-point precision issues

## Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests in release mode (for performance testing)
cargo test --release
```

## Continuous Integration

Tests should:
- Run on every commit
- Pass before merging
- Cover both debug and release builds
- Run clippy and rustfmt checks
