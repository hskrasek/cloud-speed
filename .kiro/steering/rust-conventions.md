---
inclusion: always
---

# Rust Development Conventions

## Code Style and Formatting

- Use `rustfmt` for all code formatting (configuration in `rustfmt.toml`)
- Follow Rust naming conventions:
  - `snake_case` for functions, variables, and modules
  - `PascalCase` for types, traits, and enums
  - `SCREAMING_SNAKE_CASE` for constants and statics
- Use `clippy` for linting and catching common mistakes
- Prefer explicit types when it improves clarity, but leverage type inference when obvious

## Error Handling

- Use `Result<T, Box<dyn Error>>` for functions that can fail
- Prefer `?` operator for error propagation over explicit matching
- Use `panic!` only for unrecoverable errors or programming bugs
- Provide meaningful error messages that help users understand what went wrong

## Async/Await Patterns

- This project uses `tokio` as the async runtime
- Use `async fn` for asynchronous operations
- Prefer `join!` for concurrent operations that should run in parallel
- Use `join_all` for dynamic collections of futures
- Keep async functions focused and composable

## Testing

- Place unit tests in the same file as the code being tested using `#[cfg(test)]`
- Place integration tests in a `tests/` subdirectory within each module
- Use descriptive test names that explain what is being tested
- Test both success and error cases
- For network operations, consider mocking external dependencies

## Dependencies

- Minimize external dependencies when possible
- Prefer well-maintained crates with active communities
- Use feature flags to reduce binary size (e.g., `default-features = false`)
- Document why specific dependencies are chosen in commit messages

## Performance Considerations

- This is a network measurement tool - accuracy and precision are critical
- Use appropriate data structures (Vec for sequential data, HashMap for lookups)
- Prefer `f64` for measurements to maintain precision
- Profile before optimizing - measure first, then improve
- The `release-lto` profile is optimized for production builds

## Module Organization

- Keep related functionality together in modules
- Use `mod.rs` for module definitions and re-exports
- Separate concerns: client code, request types, test implementations, statistics
- Make internal implementation details private by default
- Export only what's necessary through `pub` declarations

## Documentation

- Document public APIs with `///` doc comments
- Include examples in doc comments when helpful
- Explain non-obvious implementation decisions with `//` comments
- Keep comments up-to-date with code changes
