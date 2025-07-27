# GS-24 Implementation Plan: Core Scanner Infrastructure & Versioning

## Overview

Establish the foundational scanner module with API-based versioning system, basic structures, and bitflag scanning modes. This creates the core infrastructure that all other scanner components will build upon.

## Acceptance Criteria

- [ ] Create scanner module infrastructure with proper Rust module organisation, observing SOLID principles
- [ ] Implement API-based versioning system using date-based i64 (days since epoch)
- [ ] Define base scanner traits and configuration structures
- [ ] Add scanning modes bitflags enum (initially having a single variant none 0x0)
- [ ] Create compact message structures with fixed header + variable length data
- [ ] Implement API for plugins to query supported scan modes
- [ ] Add version compatibility checking functionality
- [ ] Design filtering system with hybrid approach (built-in filters + functional callbacks)
- [ ] Create query parameter structures for common filters (date ranges, file paths, limits)
- [ ] Implement early termination mechanism for efficient scanning
- [ ] Provide comprehensive unit test coverage

## Technical Design

### API Versioning Strategy
- **Format**: Days since Unix epoch (i64) - low resolution, manually incremented (create a cargo target that sets it)
- **Purpose**: Compatibility checking for plugins without parsing logic
- **Increment Policy**: Only when scanner API becomes incompatible with existing plugins
- **Usage**: Plugins query minimum required API version

### Suggested Module Architecture
```
src/scanner/
├── mod.rs          # Public API and core exports
├── version.rs      # API versioning system
├── modes.rs        # ScanMode bitflags and mode discovery API
├── messages.rs     # Compact message structures
├── config.rs       # Scanner configuration
├── traits.rs       # Core scanner traits
├── filters.rs      # Query filters and functional callbacks
└── query.rs        # Query parameter structures and builders
```

### Scanning Modes Design
- **Bitflags**: Combinable scan modes for performance
- **Discovery API**: Plugins can query supported modes at runtime
- **Extensibility**: Single none variant initially, can be extended without breaking compatibility

### Filtering System Design
- **Hybrid Approach**: Built-in filters for common cases + functional callbacks for complex logic
- **Built-in Filters**: Date ranges, file paths, record limits, author filters
- **Functional Callbacks**: Zero-cost abstractions using `Fn` traits and closures
- **Early Termination**: Iterator-style `ControlFlow::Break`/`ControlFlow::Continue`
- **Context-Aware**: Filter functions receive different data based on active scanning modes
- **Rust Idioms**: Leverage iterators, `Option`/`Result` monads, and zero-cost abstractions

### Query Parameters
- **Date Ranges**: Start/end dates for commit filtering
- **File Paths**: Specific files or directories to include/exclude
- **Limits**: Maximum number of records to process
- **Authors**: Filter by commit authors or contributors
- **Custom Predicates**: Functional filters for complex conditions

### Message Structure
- **Fixed Header**: Common repository statistics (commit count, file count, etc.)
- **Variable Data**: Compact byte array specific to scanning modes
- **Encoding**: Efficient serialisation for memory-conscious queues

## Implementation Steps

### Step 1: Create Scanner Module Infrastructure ⭕ TODO
**Goal**: Establish basic module structure and exports

**TDD Approach**:
1. **RED**: Write test for scanner module existence and basic imports
2. **GREEN**: Create module structure with placeholder implementations
3. **REFACTOR**: Organise module exports and documentation

**Implementation**:
- Create `src/scanner/mod.rs` with public API exports
- Add scanner module to `src/lib.rs`
- Create placeholder submodules (version, modes, messages, config, traits)
- Define module-level documentation

**Tests**:
- Test module imports work correctly
- Test module structure is accessible
- Test documentation generation

**Files to Create/Modify**:
- `src/scanner/mod.rs` (new)
- `src/scanner/version.rs` (new, placeholder)
- `src/scanner/modes.rs` (new, placeholder)
- `src/scanner/messages.rs` (new, placeholder)
- `src/scanner/config.rs` (new, placeholder)
- `src/scanner/traits.rs` (new, placeholder)
- `src/scanner/filters.rs` (new, placeholder)
- `src/scanner/query.rs` (new, placeholder)
- `src/lib.rs` (add scanner module)

### Step 2: Implement API Versioning System ⭕ TODO
**Goal**: Create date-based API versioning for plugin compatibility

**TDD Approach**:
1. **RED**: Write test for API version generation and comparison
2. **GREEN**: Implement days-since-epoch versioning system
3. **REFACTOR**: Ensure clean version API and compatibility checking

**Implementation**:
- Implement `get_api_version()` returning current API version (i64)
- Add `is_api_compatible(required_version: i64)` for compatibility checking
- Create version constants for current API level
- Add version increment utilities for development

**Tests**:
- Test API version generation returns correct day number
- Test compatibility checking with different version requirements
- Test version comparison logic
- Test version utilities and constants

**Files to Create/Modify**:
- `src/scanner/version.rs` (implement)
- `src/scanner/mod.rs` (export version functions)

### Step 3: Define Scanning Modes and Discovery API ⭕ TODO
**Goal**: Create bitflag scanning modes with plugin discovery API

**TDD Approach**:
1. **RED**: Write test for scanning mode operations and discovery
2. **GREEN**: Implement bitflags enum and discovery API
3. **REFACTOR**: Optimise mode checking and API usability

**Implementation**:
- Create `ScanMode` bitflags enum (initially empty but extensible)
- Implement `get_supported_modes()` API for plugin discovery
- Add mode validation and combination utilities
- Create mode description and metadata system

**Tests**:
- Test bitflag operations (combination, checking, etc.)
- Test mode discovery API returns correct information
- Test mode validation for invalid combinations
- Test extensibility by adding mock modes

**Files to Create/Modify**:
- `src/scanner/modes.rs` (implement)
- Add `bitflags` dependency to `Cargo.toml`

### Step 4: Create Compact Message Structures ⭕ TODO
**Goal**: Design memory-efficient message format for queue system

**TDD Approach**:
1. **RED**: Write test for message serialisation and size efficiency
2. **GREEN**: Implement compact message structures
3. **REFACTOR**: Optimise memory usage and serialisation performance

**Implementation**:
- Define `MessageHeader` with common repository statistics
- Create `ScanMessage` with header + variable data (`Box<[u8]>`)
- Implement message encoding/decoding traits
- Add message size calculation and memory tracking

**Tests**:
- Test message serialisation/deserialisation
- Test message compactness and memory usage
- Test variable data encoding for different scan modes
- Test message header consistency

**Files to Create/Modify**:
- `src/scanner/messages.rs` (implement)
- Add serialisation dependencies if needed

### Step 5: Define Core Scanner Traits ⭕ TODO
**Goal**: Create trait-based interfaces for scanner components

**TDD Approach**:
1. **RED**: Write test for trait implementations and polymorphism
2. **GREEN**: Implement core scanner traits
3. **REFACTOR**: Ensure clean trait boundaries and usability

**Implementation**:
- Define `Scanner` trait for main scanning interface
- Create `MessageProducer` trait for queue integration
- Add `VersionCompatible` trait for plugin compatibility
- Implement trait default implementations where appropriate

**Tests**:
- Test trait implementations with mock structures
- Test polymorphic usage of traits
- Test trait default implementations
- Test trait compatibility across different implementations

**Files to Create/Modify**:
- `src/scanner/traits.rs` (implement)

### Step 6: Implement Scanner Configuration ⭕ TODO
**Goal**: Create configuration structures for scanner parameters

**TDD Approach**:
1. **RED**: Write test for configuration validation and defaults
2. **GREEN**: Implement scanner configuration system
3. **REFACTOR**: Ensure configuration integrates with existing config system

**Implementation**:
- Define `ScannerConfig` structure with relevant parameters
- Add configuration validation and default value handling
- Integrate with existing configuration system pattern
- Add configuration builder pattern for ease of use

**Tests**:
- Test configuration validation with valid/invalid parameters
- Test default configuration values
- Test configuration builder pattern
- Test integration with existing config system

**Files to Create/Modify**:
- `src/scanner/config.rs` (implement)
- `src/config.rs` (extend for scanner integration)

### Step 7: Design Query Parameter System ⭕ TODO
**Goal**: Create query parameter structures for common filtering scenarios

**TDD Approach**:
1. **RED**: Write test for query parameter validation and builder pattern
2. **GREEN**: Implement query parameter structures and builders
3. **REFACTOR**: Optimise query parameter usability and validation

**Implementation**:
- Define `QueryParams` structure with date ranges, file paths, limits
- Create query builder pattern for ease of construction
- Add parameter validation and default handling
- Implement parameter serialisation for message passing

**Tests**:
- Test query parameter validation with various inputs
- Test query builder pattern and method chaining
- Test parameter serialisation/deserialisation
- Test default parameter handling

**Files to Create/Modify**:
- `src/scanner/query.rs` (new)
- `src/scanner/mod.rs` (export query structures)

### Step 8: Implement Filtering System ⭕ TODO
**Goal**: Create hybrid filtering system with Rust-idiomatic patterns for optimal performance

**TDD Approach**:
1. **RED**: Write test for filtering operations and early termination
2. **GREEN**: Implement filtering traits using zero-cost abstractions
3. **REFACTOR**: Optimise filter performance with iterator combinators

**Implementation**:
- Define filter traits using `Fn` closures and `ControlFlow` for early termination
- Implement built-in filters using iterator combinators (`filter`, `take_while`, `find`)
- Create functional callback system with zero-cost closure abstractions
- Use `std::ops::ControlFlow` for early termination signals
- Implement filter composition using iterator chaining and `flat_map`
- Leverage `Option` and `Result` monads for error handling

**Rust Idioms**:
- Use `impl Fn(T) -> ControlFlow<R, ()>` for filter functions
- Leverage iterator adaptors for composition: `filter().take_while().collect()`
- Use `?` operator for early returns in filter chains
- Employ `match` expressions for pattern-based filtering
- Utilise `std::cmp::Ordering` for range-based filters

**Tests**:
- Test built-in filters with iterator combinators
- Test zero-cost closure compilation (benchmark tests)
- Test early termination with `ControlFlow`
- Test filter composition performance

**Files to Create/Modify**:
- `src/scanner/filters.rs` (new)
- `src/scanner/traits.rs` (extend with filter traits)

### Step 9: Integration and API Finalisation ⭕ TODO
**Goal**: Complete integration and finalise public API

**TDD Approach**:
1. **RED**: Write integration tests for complete scanner core API
2. **GREEN**: Implement final API integration and exports
3. **REFACTOR**: Optimise API usability and documentation

**Implementation**:
- Finalise public API exports from `scanner::mod`
- Add comprehensive API documentation
- Implement convenience functions for common operations
- Add API examples and usage patterns

**Tests**:
- Integration test using complete scanner core API
- Test API convenience functions
- Test documentation examples
- Test API backwards compatibility considerations

**Files to Create/Modify**:
- `src/scanner/mod.rs` (finalise exports and documentation)
- Integration tests demonstrating API usage

## Testing Strategy

### Unit Tests
- API version generation and compatibility checking
- Scanning mode bitflag operations and discovery
- Message structure serialisation/deserialisation
- Configuration validation and defaults
- Trait implementations and polymorphism
- Query parameter validation and builder patterns
- Filtering system operations and early termination
- Functional callback integration and performance

### Integration Tests
- Complete scanner core API usage
- Cross-module interaction testing
- Memory usage validation
- Performance benchmarking of core operations
- End-to-end filtering with query parameters
- Functional callback performance and termination
- Filter composition and chaining efficiency

### Property-Based Tests
- Version compatibility edge cases
- Message size optimisation validation
- Configuration parameter boundary testing

## Technical Notes

### Dependencies
- `bitflags` for scanning mode flags
- Existing project dependencies (anyhow, log, etc.)
- Consider `serde` for message serialisation if needed

### API Design Principles
- **Minimal Surface**: Only expose necessary public API
- **Backwards Compatibility**: Design for future extension
- **Performance**: Optimise for memory and CPU efficiency using zero-cost abstractions
- **Documentation**: Comprehensive API documentation
- **Rust Idioms**: Leverage iterators, closures, and type system for safety and performance

### Version Management
- Start with API version representing today's date (days since epoch)
- Only increment when making breaking changes
- Document version history and compatibility requirements

## Success Criteria

1. ⭕ All unit tests pass with comprehensive coverage
2. ⭕ API versioning enables plugin compatibility checking
3. ⭕ Scanning modes system supports future extensibility
4. ⭕ Message structures demonstrate memory efficiency
5. ⭕ Core traits enable polymorphic scanner usage
6. ⭕ Configuration system integrates cleanly with existing patterns
7. ⭕ Query parameter system provides flexible filtering options
8. ⭕ Filtering system supports both built-in and functional approaches
9. ⭕ Early termination mechanism enables efficient scanning
10. ⭕ Public API is well-documented and intuitive

## Dependencies

- **GS-14**: Repository path flag support (✅ COMPLETED)
- **Future**: This provides foundation for GS-25 (Git Integration)

## Status: ⭕ TODO

Ready to begin implementation following TDD workflow.

## Future Considerations

- Plugin registration system will build on version compatibility
- Message queue system will use these message structures
- Async scanner engine will implement the core traits
- Additional scanning modes can be added to bitflags enum
- Advanced filtering predicates and query optimisation
- Distributed filtering across multiple repositories
- Caching of filter results for repeated queries

## Notes

This implementation establishes the foundational infrastructure that all subsequent scanner components will depend upon. The API versioning system ensures plugins can safely check compatibility, while the bitflag scanning modes provide efficient, combinable scan configurations. The compact message format enables memory-conscious queue operations essential for the overall scanner architecture.

The hybrid filtering system provides both performance (built-in filters) and flexibility (functional callbacks), allowing plugins to efficiently process large repositories while maintaining the ability to implement complex custom filtering logic. The early termination mechanism ensures that scanning can be stopped as soon as sufficient data is collected, optimising resource usage.

**Rust Performance Idioms**: The filtering system leverages zero-cost abstractions through closures and iterator combinators, ensuring that functional-style filtering compiles to optimal machine code. The use of `ControlFlow` provides early termination without allocation overhead, while iterator chaining enables efficient composition without intermediate collections.
