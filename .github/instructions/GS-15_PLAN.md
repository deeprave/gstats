# GS-15 Implementation Plan: Repository Scanner MVP with Scanning Modes

## Overview

Implement a foundational repository scanner system that provides efficient, memory-conscious scanning of git repositories with configurable scanning modes, message queuing, and plugin communication interfaces. This MVP establishes the core infrastructure for the plugin architecture system.

## Acceptance Criteria

- [ ] Implement bitflag-based scanning modes enum (initially empty for extensibility)
- [ ] Create compact, memory-efficient message structures with variable-length data
- [ ] Implement in-memory message queue system with memory monitoring and backoff
- [ ] Add datetime-based scanner versioning for plugin compatibility
- [ ] Refactor git validation to return Repository handle for scanner use
- [ ] Create streaming message interface with async/await notification system
- [ ] Implement trait-based scanner and plugin interfaces
- [ ] Add configuration/CLI options for memory limits
- [ ] Provide base scanning statistics common to all scan types
- [ ] Support multi-producer/multi-consumer queue architecture

## Technical Design

### Scanner Version System
- **Format**: Unix timestamp (i64) representing build date
- **Purpose**: Plugin compatibility checking without parsing logic
- **Usage**: Plugins specify minimum required scanner version

### Message Structure
- **Base Header**: Fixed-size struct with common repository statistics
- **Variable Data**: Compact byte array specific to scanning modes
- **Decoding**: Scanner provides decode methods for known scan types

### Memory Management
- **Queue Monitoring**: Track memory usage per queue
- **Backoff Algorithm**: Stall scanner when memory threshold approached
- **Configuration**: CLI/config options for memory limits

### Concurrency Model
- **Scanner Tasks**: Each scanning mode runs in separate async task
- **Queue System**: Multi-producer/multi-consumer with notifications
- **Plugin Communication**: Async notification-based consumption

## Implementation Steps

### Step 1: Core Scanner Infrastructure ⭕ TODO
**Goal**: Establish scanner module with version system and basic structures

**TDD Approach**:
1. **RED**: Write test for scanner version generation
2. **GREEN**: Implement datetime-based version system
3. **REFACTOR**: Ensure clean version interface

**Implementation**:
- Create `src/scanner/mod.rs` with core scanner structures
- Implement datetime-based version system (Unix timestamp)
- Define base scanner traits and version checking
- Add scanner configuration structure

**Tests**:
- Test version generation and comparison
- Test scanner configuration validation
- Test basic scanner initialization

**Files to Create/Modify**:
- `src/scanner/mod.rs` (new)
- `src/scanner/version.rs` (new)
- `src/lib.rs` (add scanner module)

### Step 2: Scanning Modes and Message Structures ⭕ TODO
**Goal**: Define bitflag scanning modes and compact message format

**TDD Approach**:
1. **RED**: Write test for scanning mode combination and message creation
2. **GREEN**: Implement bitflag modes and message structures
3. **REFACTOR**: Optimize message compactness

**Implementation**:
- Create `ScanMode` bitflags enum (initially empty but extensible)
- Design compact message structures with fixed header + variable data
- Implement message encoding/decoding interface
- Create base repository statistics structure

**Tests**:
- Test scanning mode bitflag operations
- Test message serialization/deserialization
- Test variable data encoding/decoding
- Test message compactness and memory usage

**Files to Create/Modify**:
- `src/scanner/modes.rs` (new)
- `src/scanner/messages.rs` (new)
- `src/scanner/mod.rs` (extend)

### Step 3: Git Integration and Repository Handle Management ⭕ TODO
**Goal**: Refactor git validation to provide Repository handle for scanner

**TDD Approach**:
1. **RED**: Write test for git validation returning Repository handle
2. **GREEN**: Refactor `validate_git_repository` to return Repository
3. **REFACTOR**: Update all callers to use new interface

**Implementation**:
- Refactor `git::validate_git_repository` to return `git2::Repository`
- Update `git::resolve_repository_path` to work with Repository handles
- Integrate Repository handle with scanner initialization
- Maintain backward compatibility where needed

**Tests**:
- Test git validation returns valid Repository handle
- Test Repository handle integration with existing code
- Test error handling with invalid repositories
- Update existing git tests for new interface

**Files to Create/Modify**:
- `src/git.rs` (refactor)
- `src/scanner/repository.rs` (new)
- Update all existing callers

### Step 4: Memory-Conscious Message Queue System ⭕ TODO
**Goal**: Implement in-memory queue with memory monitoring and backoff

**TDD Approach**:
1. **RED**: Write test for queue memory tracking and limits
2. **GREEN**: Implement memory-monitored queue system
3. **REFACTOR**: Optimize queue performance and memory usage

**Implementation**:
- Create memory-monitored message queue structure
- Implement backoff algorithm when approaching memory limits
- Add queue statistics and monitoring interfaces
- Support multi-producer/multi-consumer operations

**Tests**:
- Test queue memory tracking accuracy
- Test backoff algorithm behavior
- Test multi-producer/multi-consumer operations
- Test queue overflow protection

**Files to Create/Modify**:
- `src/scanner/queue.rs` (new)
- `src/scanner/memory.rs` (new)

### Step 5: Async Scanner Engine ⭕ TODO
**Goal**: Implement async scanning engine with streaming results

**TDD Approach**:
1. **RED**: Write test for async scanning with message streaming
2. **GREEN**: Implement async scanner with task coordination
3. **REFACTOR**: Optimize async performance and resource usage

**Implementation**:
- Create async scanner engine coordinating multiple scan modes
- Implement streaming message production
- Add task management for concurrent scanning modes
- Integrate with Repository handle from git validation

**Tests**:
- Test async scanner initialization and execution
- Test streaming message production
- Test concurrent scanning mode coordination
- Test integration with Repository handles

**Files to Create/Modify**:
- `src/scanner/engine.rs` (new)
- `src/scanner/tasks.rs` (new)

### Step 6: Plugin Communication Interface ⭕ TODO
**Goal**: Create trait-based interface for plugin communication

**TDD Approach**:
1. **RED**: Write test for plugin notification and message consumption
2. **GREEN**: Implement plugin traits and notification system
3. **REFACTOR**: Ensure clean plugin API design

**Implementation**:
- Define plugin traits for scanner interaction
- Implement async notification system for queue updates
- Create plugin registration and discovery mechanisms
- Add version compatibility checking for plugins

**Tests**:
- Test plugin trait implementations
- Test async notification delivery
- Test plugin version compatibility checking
- Test plugin registration and discovery

**Files to Create/Modify**:
- `src/scanner/plugins.rs` (new)
- `src/scanner/traits.rs` (new)

### Step 7: Configuration and CLI Integration ⭕ TODO
**Goal**: Add CLI options and configuration for scanner memory limits

**TDD Approach**:
1. **RED**: Write test for CLI parsing of scanner options
2. **GREEN**: Implement CLI integration with scanner configuration
3. **REFACTOR**: Ensure consistent configuration management

**Implementation**:
- Add scanner-related CLI arguments (memory limits, queue sizes)
- Integrate scanner configuration with existing config system
- Add validation for scanner configuration parameters
- Update help text and documentation

**Tests**:
- Test CLI argument parsing for scanner options
- Test configuration validation and defaults
- Test integration with existing configuration system

**Files to Create/Modify**:
- `src/cli.rs` (extend)
- `src/config.rs` (extend)
- `src/scanner/config.rs` (new)

### Step 8: Base Statistics Implementation ⭕ TODO
**Goal**: Implement common repository statistics collection

**TDD Approach**:
1. **RED**: Write test for base repository statistics collection
2. **GREEN**: Implement basic statistics gathering
3. **REFACTOR**: Optimize statistics collection performance

**Implementation**:
- Implement basic repository statistics (commit count, file count, etc.)
- Create statistics message format for base data
- Integrate statistics collection with scanner engine
- Ensure efficient statistics gathering

**Tests**:
- Test basic statistics collection accuracy
- Test statistics message format
- Test integration with scanner engine
- Test performance of statistics gathering

**Files to Create/Modify**:
- `src/scanner/stats.rs` (new)
- `src/scanner/messages.rs` (extend)

### Step 9: Integration Testing and Performance Validation ⭕ TODO
**Goal**: Comprehensive testing of complete scanner system

**TDD Approach**:
1. **RED**: Write integration tests for complete scanner workflow
2. **GREEN**: Ensure all components work together correctly
3. **REFACTOR**: Optimize overall system performance

**Implementation**:
- Create comprehensive integration tests
- Test memory usage under various loads
- Validate async performance and responsiveness
- Test error handling and recovery scenarios

**Tests**:
- Integration test with real repository scanning
- Memory usage validation tests
- Performance benchmarking tests
- Error handling and recovery tests

**Files to Create/Modify**:
- `tests/scanner_integration_test.rs` (new)
- `benches/scanner_bench.rs` (new)

## Testing Strategy

### Unit Tests
- Scanner version system functionality
- Message structure serialization/deserialization
- Queue memory monitoring and backoff
- Git integration with Repository handles
- Plugin trait implementations

### Integration Tests
- End-to-end scanning workflow
- Memory management under load
- Async task coordination
- Plugin communication interface

### Performance Tests
- Memory usage benchmarking
- Message throughput testing
- Queue performance under concurrent load
- Scanner responsiveness testing

## Technical Notes

### Dependencies
- `bitflags` for scanning mode flags
- `tokio` for async runtime
- `git2` for repository access (existing)
- `serde` for message serialization (if needed)

### Memory Optimization
- Use `Box<[u8]>` for variable-length message data
- Implement custom memory tracking for queue monitoring
- Use memory pools for frequent allocations if needed

### Version Strategy
- Scanner version: Unix timestamp (i64)
- Plugin compatibility: minimum required version check
- Future extensibility: major/minor version separation if needed

### Error Handling
- Use `anyhow::Result` for error propagation (consistent with existing code)
- Implement scanner-specific error types for better diagnostics
- Graceful degradation when memory limits approached

## Success Criteria

1. ⭕ All unit tests pass with 100% coverage
2. ⭕ Integration tests demonstrate complete scanning workflow
3. ⭕ Memory usage stays within configured limits
4. ⭕ Scanner version system enables plugin compatibility
5. ⭕ Async performance meets responsiveness requirements
6. ⭕ Plugin interface supports future extensibility

## Dependencies

- **GS-14**: Repository path flag support (✅ COMPLETED)
- **Future**: Plugin implementation will build on this foundation

## Status: ⭕ TODO

Ready to begin implementation following TDD workflow.

## Future Enhancements

- Multiple repository scanning support
- Persistent queue storage options
- Advanced memory management strategies
- Plugin hot-reloading capabilities
- Distributed scanning coordination

## Notes

This scanner implementation serves as the foundational layer for the entire plugin architecture. The design prioritizes memory efficiency, async performance, and extensibility while maintaining simplicity for the MVP scope. The empty scanning modes enum allows for future extension without breaking compatibility.
