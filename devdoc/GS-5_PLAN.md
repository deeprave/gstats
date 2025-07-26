# GS-5 Implementation Plan: Implement Logging

## Project: Git Repository Analytics Tool (gstats)
**YouTrack Issue:** GS-5 - "Implement logging"  
**Date:** 26 July 2025  
**Phase:** Infrastructure Enhancement - Logging System  

## Overview
This implementation plan establishes a comprehensive logging system for gstats with structured logging capabilities. The logging system should provide configurable log levels, timestamp formatting (YYYY-MM-DD HH:mm:ss), and proper integration with the existing CLI and git analysis modules. This enhances observability and debugging capabilities for the application.

## Requirements Analysis
Based on the YouTrack issue description and additional requirements:
- Set up logger to report in format: YYYY-MM-DD HH:mm:ss
- Support JSON format output with structured logging
- Support file output with optional independent log level
- Use a logger for messages throughout the application
- Integrate with existing CLI and git detection modules
- Provide configurable log levels for different use cases
- JSON format should include: timestamp, loglevel, message, detail (initially empty/optional)

## Acceptance Criteria
- [ ] Logging system integrated with structured timestamp format (YYYY-MM-DD HH:mm:ss)
- [ ] Support for both text and JSON log output formats
- [ ] JSON format includes: timestamp, loglevel, message, detail (optional/empty initially)
- [ ] Configurable log levels (error, warn, info, debug, trace)
- [ ] Logger integrated into main application flow and all modules
- [ ] CLI option to control log level (--verbose, --quiet flags)
- [ ] CLI option to control log format (--log-format json)
- [ ] CLI option for file output (--log-file) with optional independent log level
- [ ] Console and file logging can have different log levels
- [ ] Proper error logging for git operations and CLI failures
- [ ] Log messages are human-readable and informative
- [ ] Comprehensive test coverage for logging functionality

## Implementation Steps

### 1. Add Dependencies and Setup (RED Phase)
**Objective:** Add logging dependencies and create failing tests

#### 1.1 Add Logging Dependencies
- [ ] Add `log` crate to Cargo.toml for logging interface
- [ ] Add `env_logger` crate for simple logging implementation
- [ ] Add `chrono` crate for timestamp formatting
- [ ] Add `serde` and `serde_json` crates for JSON serialization
- [ ] Run `cargo check` to verify dependencies

**Verification Test:** Dependencies added and project compiles ✅

#### 1.2 Create Failing Tests for Logging
- [ ] Create test module for logging functionality
- [ ] Write failing test for logger initialization
- [ ] Write failing test for timestamp format validation
- [ ] Write failing test for JSON format output structure
- [ ] Write failing test for log level filtering
- [ ] Write failing test for format switching (text vs JSON)
- [ ] Write failing test for file output functionality
- [ ] Write failing test for independent file log levels
- [ ] Run `cargo test` to confirm tests fail (RED phase)

**Verification Test:** All new logging tests fail as expected ✅

### 2. Implement Basic Logging Infrastructure (GREEN Phase)
**Objective:** Create basic logging setup and initialization

#### 2.1 Create Logging Module
- [ ] Create `src/logging.rs` module for logging configuration
- [ ] Define log format enum (Text, Json)
- [ ] Define log destination enum (Console, File, Both)
- [ ] Implement JSON log entry structure with timestamp, loglevel, message, detail
- [ ] Implement logger initialization function with format and destination support
- [ ] Configure timestamp format (YYYY-MM-DD HH:mm:ss)
- [ ] Set up default log level handling for console and file
- [ ] Export logging functions from module

**Verification Test:** Logging module compiles and basic structure exists ✅

#### 2.2 Integrate Logger with Main Application
- [ ] Update main.rs to initialize logging system
- [ ] Add logging calls to main application flow
- [ ] Replace `println!` statements with appropriate log levels
- [ ] Ensure logger is initialized before any log calls

**Verification Test:** Main application initializes logging correctly ✅

### 3. Add CLI Integration for Log Control (GREEN Phase)
**Objective:** Add command line options for log level control

#### 3.1 Extend CLI Arguments
- [ ] Add verbose flag (`-v`, `--verbose`) to CLI arguments
- [ ] Add quiet flag (`-q`, `--quiet`) to CLI arguments
- [ ] Add debug flag (`--debug`) for maximum logging
- [ ] Add log format flag (`--log-format`) with options: text, json
- [ ] Add log file flag (`--log-file`) for file output path
- [ ] Add file log level flag (`--file-log-level`) for independent file logging level
- [ ] Update CLI parsing to handle new flags

**Verification Test:** CLI parsing accepts new logging flags ✅

#### 3.2 Connect CLI to Logging Configuration
- [ ] Update logging initialization to accept log level, format, and file options from CLI
- [ ] Implement log level mapping (quiet=error, normal=info, verbose=debug, debug=trace)
- [ ] Implement format selection (text vs JSON output)
- [ ] Implement file output configuration with independent log level
- [ ] Ensure CLI flags properly control logging output, format, and destinations
- [ ] Add validation for conflicting flags and file access permissions

**Verification Test:** CLI flags control logging output correctly ✅

### 4. Integrate Logging Throughout Application (GREEN Phase)
**Objective:** Add structured logging to all modules

#### 4.1 Update Git Module with Logging
- [ ] Add logging to git repository detection functions
- [ ] Log git operations (successful detection, validation failures)
- [ ] Use appropriate log levels (info for success, warn for issues, error for failures)
- [ ] Add context information to log messages

**Verification Test:** Git module operations are properly logged ✅

#### 4.2 Update CLI Module with Logging
- [ ] Add logging to argument parsing operations
- [ ] Log configuration and setup information
- [ ] Add debug logging for development assistance
- [ ] Ensure user-friendly log messages

**Verification Test:** CLI operations are properly logged ✅

### 5. Error Handling and Integration (GREEN Phase)
**Objective:** Ensure robust error logging and proper integration

#### 5.1 Enhanced Error Logging
- [ ] Update error handling to use logging instead of `eprintln!`
- [ ] Add structured error context to log messages
- [ ] Implement proper error propagation with logging
- [ ] Ensure critical errors are always logged regardless of log level

**Verification Test:** Error conditions are properly logged ✅

#### 5.2 Performance and Output Optimization
- [ ] Ensure logging doesn't impact application performance significantly
- [ ] Optimize log message formatting for readability
- [ ] Handle log output buffering appropriately for both console and file
- [ ] Add file creation and permission error handling
- [ ] Implement file locking considerations for concurrent access
- [ ] Add log rotation considerations for future use

**Verification Test:** Logging performance is acceptable ✅

### 6. Testing and Documentation (REFACTOR Phase)
**Objective:** Ensure comprehensive test coverage and clean code

#### 6.1 Complete Test Suite
- [ ] Write unit tests for logging module functionality
- [ ] Write unit tests for JSON format serialization
- [ ] Write unit tests for file output functionality
- [ ] Write integration tests for CLI logging control
- [ ] Write integration tests for format switching
- [ ] Write integration tests for file logging with independent levels
- [ ] Write tests for error logging scenarios
- [ ] Add tests for timestamp format validation
- [ ] Add tests for JSON structure validation
- [ ] Add tests for file permission and creation scenarios
- [ ] Ensure all tests pass (GREEN phase confirmation)

**Verification Test:** Full test suite passes with comprehensive coverage ✅

#### 6.2 Code Quality and Documentation
- [ ] Add API documentation to logging functions
- [ ] Ensure log messages are clear and actionable
- [ ] Refactor code for clarity and maintainability
- [ ] Add module-level documentation for logging

**Verification Test:** Code quality standards met, documentation complete ✅

### 7. Future Logging Foundation
**Objective:** Prepare foundation for advanced logging features

#### 7.1 Design for Extensibility
- [ ] Structure logging to support future log output formats beyond JSON and text
- [ ] Design JSON structure to support future detail field enhancements (MDC context)
- [ ] Design for future log destinations (files, remote logging)
- [ ] Ensure current implementation doesn't block future enhancements
- [ ] Document extension points for future logging features (detail field, additional JSON fields)

**Verification Test:** Architecture supports future logging enhancements ✅

## Testing Strategy

### TDD Approach:
1. **Unit Tests:** Individual logging functions and configuration
2. **Integration Tests:** CLI integration and module logging
3. **Error Case Tests:** Error logging and edge cases
4. **Performance Tests:** Logging overhead validation

### Test Categories:
```rust
// Unit tests for logging configuration
#[cfg(test)]
mod logging_tests {
    // Test logger initialization
    // Test timestamp formatting
    // Test log level filtering
}

// Integration tests for CLI logging
// tests/logging_integration_test.rs
// Test CLI flags controlling log output
```

## Dependencies to Add

```toml
[dependencies]
log = "0.4"
env_logger = "0.11"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Expected CLI Usage

```bash
# Default logging (info level, text format, console only)
gstats /path/to/repo

# Verbose logging (debug level, text format, console only)
gstats --verbose /path/to/repo
gstats -v /path/to/repo

# Quiet logging (error only, text format, console only)
gstats --quiet /path/to/repo
gstats -q /path/to/repo

# Maximum debugging (text format, console only)
gstats --debug /path/to/repo

# JSON format logging (console only)
gstats --log-format json /path/to/repo
gstats --verbose --log-format json /path/to/repo

# File output with same log level as console
gstats --log-file gstats.log /path/to/repo
gstats --verbose --log-file gstats.log /path/to/repo

# File output with independent log level
gstats --quiet --log-file gstats.log --file-log-level debug /path/to/repo
gstats --log-file gstats.log --file-log-level trace /path/to/repo

# JSON format to file
gstats --log-format json --log-file gstats.log /path/to/repo
```

## Expected Log Format

### Text Format
```
2025-07-26 14:30:45 [INFO] Analyzing git repository at: /path/to/repo
2025-07-26 14:30:45 [DEBUG] Git repository detected successfully
2025-07-26 14:30:45 [WARN] Large repository detected, analysis may take time
2025-07-26 14:30:45 [ERROR] Failed to access git repository: Permission denied
```

### JSON Format
```json
{"timestamp":"2025-07-26 14:30:45","loglevel":"INFO","message":"Analyzing git repository at: /path/to/repo"}
{"timestamp":"2025-07-26 14:30:45","loglevel":"DEBUG","message":"Git repository detected successfully"}
{"timestamp":"2025-07-26 14:30:45","loglevel":"WARN","message":"Large repository detected, analysis may take time"}
{"timestamp":"2025-07-26 14:30:45","loglevel":"ERROR","message":"Failed to access git repository: Permission denied","detail":{}}
```

## Success Criteria

The logging implementation is complete when:
- [ ] Structured logging with timestamp format (YYYY-MM-DD HH:mm:ss) is working
- [ ] Both text and JSON log formats are supported
- [ ] JSON format includes required fields: timestamp, loglevel, message, detail (optional)
- [ ] CLI flags control log levels appropriately
- [ ] CLI flag controls log format (text vs JSON)
- [ ] File output with optional independent log level is working
- [ ] Console and file can have different log levels simultaneously
- [ ] All modules use logging instead of print statements
- [ ] Error conditions are properly logged with context
- [ ] File creation and permission errors are handled gracefully
- [ ] Comprehensive test coverage for all logging scenarios
- [ ] Foundation established for future logging enhancements

## Error Handling Scenarios

1. **Logger initialization failure:** Graceful fallback to stderr output
2. **Invalid log level specified:** Clear error message and default fallback
3. **Conflicting CLI flags:** Validation and user-friendly error
4. **Log output issues:** Proper error handling for output stream problems
5. **File creation failure:** Clear error message about file permissions/path issues
6. **File write failure:** Graceful degradation to console-only logging
7. **Invalid file log level:** Default to console log level with warning

## Future Enhancement Foundation

This implementation will prepare for:
- Additional structured JSON fields beyond the core set
- MDC (Mapped Diagnostic Context) support in the detail field
- Custom log formatters and output destinations
- Log file output and rotation
- Remote logging capabilities
- Performance monitoring and metrics
- Configuration file support for logging settings

## Next Phase Preparation

Upon completion of this phase:
1. Move GS-5 to "Queued" state
2. Update devdoc/README.md to reflect completion
3. Create next YouTrack issue for core git analysis functionality
4. Begin implementing actual git repository analysis features with proper logging

## Notes
- This phase establishes comprehensive logging for debugging and observability
- Replaces ad-hoc println! statements with structured logging
- Uses standard Rust logging practices with log and env_logger crates
- Implements configurable log levels for different use cases
- Follows TDD methodology throughout implementation
- **Status:** ✅ IMPLEMENTATION COMPLETE
