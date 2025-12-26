---
inclusion: always
---

# CLI Design Guidelines

## User Experience Principles

1. **Fast by default** - Start showing results immediately
2. **Clear output** - Users should understand what they're seeing
3. **Flexible formats** - Support both human and machine-readable output
4. **Sensible defaults** - Work well without configuration

## Command-Line Interface

### Argument Design
Use `clap` with derive macros for argument parsing:
- Keep flags short and memorable
- Provide both short (`-j`) and long (`--json`) forms
- Use sensible defaults (human-readable output)
- Document each flag clearly

### Current Flags
- `-j, --json` - Output results in JSON format
- `-p, --pretty` - Pretty-print JSON (requires `--json`)
- `-v, --verbose` - Increase logging verbosity (can be repeated)

### Adding New Flags
When adding new options:
- Consider if it should be a flag (boolean) or option (takes a value)
- Think about interactions with existing flags
- Update help text and documentation
- Consider environment variable alternatives for CI/CD use

## Output Formatting

### Human-Readable Output
- Use colors to highlight important information:
  - **White bold** for labels
  - **Bright blue** for server/network info
  - **Bright red** for latency/jitter
  - **Yellow** for individual test speeds
  - **Bright cyan** for final results
- Align values for easy scanning
- Use consistent spacing and indentation
- Show progress as tests run (don't leave users waiting silently)

### JSON Output
- Use `serde` for serialization
- Provide both compact and pretty-printed options
- Include all relevant data:
  - Timestamp
  - Server metadata
  - Location information
  - All measurements
  - Calculated statistics
- Use consistent field naming (snake_case)
- Handle special cases (e.g., `100kb` as field name)

### Example Output Structure
```
Server Location: San Francisco (SFO)
Your network:   Example ISP (AS12345)
Your IP:        203.0.113.1 (US)
Latency:        15.23 ms
Jitter:         2.45 ms
100kB speed:    45.67 Mbps
1MB speed:      89.12 Mbps
Download speed: 95.34 Mbps
Upload speed:   42.56 Mbps
```

## Error Handling

### User-Facing Errors
- Provide clear, actionable error messages
- Suggest solutions when possible
- Don't expose internal implementation details
- Use appropriate exit codes

### Examples
```rust
// Good
"Failed to connect to speed.cloudflare.com. Check your internet connection."

// Bad
"Error: reqwest::Error { kind: Connect, source: ... }"
```

## Logging

### Verbosity Levels
- **Default (no -v)**: Only show results, no debug info
- **-v (Info)**: Show measurement details as they happen
- **-vv (Debug)**: Show detailed timing and API interactions
- **-vvv (Trace)**: Show everything including HTTP details

### What to Log
- Info: Individual measurement results
- Debug: API requests/responses, calculation steps
- Trace: Low-level HTTP details, timing breakdowns

### Logging Best Practices
- Use structured logging with `log` crate
- Don't log in hot paths (inside tight loops)
- Log before and after significant operations
- Include relevant context (which test, iteration number, etc.)

## Progress Indication

### Current Behavior
- Results appear as tests complete
- No explicit progress bar (tests are fast enough)

### Future Considerations
If tests become longer:
- Add progress indicators for long-running tests
- Show which test is currently running
- Display estimated time remaining
- Use `indicatif` crate for progress bars

## Compatibility

### Terminal Support
- Detect if output is a TTY
- Disable colors when piping to files
- Handle narrow terminals gracefully
- Support common terminal emulators

### Platform Support
- Works on Linux, macOS, Windows
- Use platform-agnostic APIs
- Test on multiple platforms before release
- Document platform-specific behavior if any

## Documentation

### Help Text
- Keep it concise but informative
- Show examples of common usage
- Explain what each flag does
- Include version information

### Man Pages / README
- Provide detailed usage examples
- Explain what each measurement means
- Document JSON output schema
- Include troubleshooting section
