# GS-31: CLI Filtering Flags Implementation Plan

**Status**: Ready to Start  
**Priority**: High  
**Dependencies**: GS-24 (Core Scanner Infrastructure) ✅ COMPLETED  
**YouTrack Issue**: GS-31 (Issue #3-228)

## Overview

Add comprehensive filtering flags to the CLI interface to enable users to specify date ranges, file path patterns, and author filters directly from the command line. This is essential for both testing the filtering system implemented in GS-24 and providing a complete user experience.

## Acceptance Criteria

- [ ] All filtering flags implemented and functional with comprehensive validation
- [ ] Date parsing supports both absolute (ISO 8601) and relative formats ("1 week ago")
- [ ] Path filtering supports glob-like patterns with include/exclude functionality
- [ ] Author filtering handles email addresses and names with include/exclude functionality
- [ ] Integration with existing GS-24 filtering system using QueryParams
- [ ] Comprehensive validation and error handling with clear error messages
- [ ] Updated CLI help documentation with examples
- [ ] Unit and integration tests achieving >95% coverage
- [ ] All existing tests continue to pass

## Implementation Steps

### Step 1: Extend CLI Args Structure with Basic Filtering Flags
**Goal**: Add new CLI arguments to Args struct using clap derive macros

#### 1.1: Add Date Filtering Arguments
- [ ] Add `since` field: `Option<String>` for start date
- [ ] Add `until` field: `Option<String>` for end date
- [ ] Use appropriate clap attributes with value names and help text
- [ ] Test: Verify args parsing accepts date arguments without validation

#### 1.2: Add Path Filtering Arguments  
- [ ] Add `include_path` field: `Vec<String>` with `ArgAction::Append`
- [ ] Add `exclude_path` field: `Vec<String>` with `ArgAction::Append`
- [ ] Use appropriate clap attributes for repeatable arguments
- [ ] Test: Verify multiple paths can be specified

#### 1.3: Add Author Filtering Arguments
- [ ] Add `author` field: `Vec<String>` with `ArgAction::Append`
- [ ] Add `exclude_author` field: `Vec<String>` with `ArgAction::Append`
- [ ] Use appropriate clap attributes for repeatable arguments
- [ ] Test: Verify multiple authors can be specified

#### 1.4: Add Result Control Arguments
- [ ] Add `limit` field: `Option<usize>` for maximum results
- [ ] Add `mode` field: `String` with default value "commits"
- [ ] Use appropriate clap attributes with validation
- [ ] Test: Verify limit and mode arguments work correctly

**Verification**: CLI accepts all new arguments, help text displays correctly, basic parsing works

### Step 2: Implement Date Parsing with Chrono
**Goal**: Create robust date parsing supporting both absolute and relative formats

#### 2.1: Create Date Parser Module
- [ ] Create `src/cli/date_parser.rs` module
- [ ] Add to `src/cli/mod.rs` if it doesn't exist
- [ ] Import chrono dependencies
- [ ] Test: Module structure compiles

#### 2.2: Implement Absolute Date Parsing
- [ ] Parse ISO 8601 formats: `YYYY-MM-DD`, `YYYY-MM-DDTHH:MM:SS`
- [ ] Support timezone aware and naive parsing
- [ ] Return `SystemTime` for compatibility with filtering system
- [ ] Test: Various ISO 8601 formats parse correctly

#### 2.3: Implement Relative Date Parsing
- [ ] Parse relative formats: "1 week ago", "3 months ago", "yesterday"
- [ ] Support units: seconds, minutes, hours, days, weeks, months, years
- [ ] Handle "yesterday", "today", "tomorrow" keywords
- [ ] Test: Relative date expressions parse to correct SystemTime

#### 2.4: Add Date Validation
- [ ] Validate date range logic (start <= end)
- [ ] Provide clear error messages for invalid formats
- [ ] Handle edge cases and malformed input gracefully
- [ ] Test: Invalid dates produce appropriate error messages

**Verification**: Date parsing handles all specified formats correctly with proper error handling

### Step 3: Implement CLI Argument Conversion
**Goal**: Convert parsed CLI arguments to QueryParams for filtering system integration

#### 3.1: Create CLI to QueryParams Converter
- [ ] Create `src/cli/converter.rs` module
- [ ] Implement `args_to_query_params(args: &Args) -> Result<QueryParams, CliError>`
- [ ] Define `CliError` enum for conversion errors
- [ ] Test: Basic conversion structure compiles

#### 3.2: Convert Date Arguments
- [ ] Parse `since` and `until` arguments using date parser
- [ ] Create `DateRange` from parsed dates
- [ ] Handle partial date ranges (only start or only end)
- [ ] Test: Date conversion produces correct QueryParams

#### 3.3: Convert Path Arguments
- [ ] Convert `include_path` and `exclude_path` to `PathBuf` vectors
- [ ] Create `FilePathFilter` from path arguments
- [ ] Validate path syntax and existence (optional)
- [ ] Test: Path conversion produces correct QueryParams

#### 3.4: Convert Author Arguments
- [ ] Convert `author` and `exclude_author` to `AuthorFilter`
- [ ] Validate author name format (non-empty)
- [ ] Handle email address formats appropriately
- [ ] Test: Author conversion produces correct QueryParams

#### 3.5: Apply Result Controls
- [ ] Set limit from CLI argument
- [ ] Validate mode against supported scanning modes
- [ ] Integrate with QueryParams structure
- [ ] Test: Control parameters applied correctly

**Verification**: CLI arguments convert correctly to QueryParams with validation

### Step 4: Implement Comprehensive Validation
**Goal**: Add robust validation with clear error messages

#### 4.1: Create CLI Validation Module
- [ ] Create `src/cli/validation.rs` module
- [ ] Define comprehensive `CliValidationError` enum
- [ ] Implement validation functions for each argument type
- [ ] Test: Validation module structure compiles

#### 4.2: Date Range Validation
- [ ] Validate start date <= end date when both provided
- [ ] Check for reasonable date ranges (not too far in past/future)
- [ ] Provide specific error messages for date issues
- [ ] Test: Date validation catches various error conditions

#### 4.3: Path Validation
- [ ] Validate path format (non-empty, valid characters)
- [ ] Check for conflicting include/exclude patterns
- [ ] Warn about non-existent paths (non-fatal)
- [ ] Test: Path validation handles edge cases

#### 4.4: Author Validation
- [ ] Validate author names are non-empty
- [ ] Check for reasonable email format when applicable
- [ ] Detect conflicts between include/exclude lists
- [ ] Test: Author validation provides helpful feedback

#### 4.5: Cross-Field Validation
- [ ] Validate argument combinations make sense
- [ ] Check for mutually exclusive options
- [ ] Provide suggestions for fixing validation errors
- [ ] Test: Complex validation scenarios work correctly

**Verification**: Validation provides clear, actionable error messages for all invalid inputs

### Step 5: Integration with Filtering System
**Goal**: Wire CLI arguments to existing GS-24 filtering infrastructure

#### 5.1: Update Main Application Flow
- [ ] Modify `src/main.rs` to use new CLI conversion functions
- [ ] Handle CLI validation errors appropriately
- [ ] Pass QueryParams to filtering system
- [ ] Test: Integration compiles and basic flow works

#### 5.2: Connect to Scanner Module
- [ ] Use `FilterExecutor::filter_from_query()` from GS-24
- [ ] Apply CLI-derived filters to scanning operations
- [ ] Ensure proper error propagation
- [ ] Test: Filtering integration works end-to-end

#### 5.3: Add Result Processing
- [ ] Apply limit constraints during result processing
- [ ] Handle different scanning modes appropriately
- [ ] Format output based on applied filters
- [ ] Test: Results respect CLI filtering parameters

#### 5.4: Performance Optimization
- [ ] Ensure early termination works with CLI limits
- [ ] Optimize filter application order
- [ ] Add progress reporting for large operations
- [ ] Test: Performance is acceptable with large repositories

**Verification**: CLI filtering integrates seamlessly with existing scanner infrastructure

### Step 6: Comprehensive Testing
**Goal**: Achieve >95% test coverage with comprehensive test scenarios

#### 6.1: Unit Tests for Date Parsing
- [ ] Test all supported date formats
- [ ] Test relative date calculations
- [ ] Test error conditions and edge cases
- [ ] Test timezone handling
- [ ] Achieve >95% coverage for date parsing module

#### 6.2: Unit Tests for CLI Conversion
- [ ] Test argument to QueryParams conversion
- [ ] Test validation error cases
- [ ] Test partial argument scenarios
- [ ] Test complex argument combinations
- [ ] Achieve >95% coverage for conversion module

#### 6.3: Integration Tests
- [ ] Test end-to-end CLI filtering with real git repositories
- [ ] Test performance with large datasets
- [ ] Test error handling in realistic scenarios
- [ ] Test help output and usage examples
- [ ] Verify no regression in existing functionality

#### 6.4: Property-Based Tests
- [ ] Generate random valid date ranges and verify parsing
- [ ] Generate random path patterns and verify filtering
- [ ] Generate random author combinations and verify results
- [ ] Use property-based testing for edge case discovery

**Verification**: All tests pass, coverage >95%, no regressions introduced

### Step 7: Documentation and Help Updates
**Goal**: Provide comprehensive CLI documentation and examples

#### 7.1: Update CLI Help Text
- [ ] Add detailed help text for all new arguments
- [ ] Include examples for common usage patterns
- [ ] Document date format specifications
- [ ] Add usage tips and best practices

#### 7.2: Update Application Documentation
- [ ] Update README.md with filtering examples
- [ ] Add comprehensive usage guide
- [ ] Document performance considerations
- [ ] Include troubleshooting section

#### 7.3: Add Code Documentation
- [ ] Document all public functions and modules
- [ ] Add examples in doc comments
- [ ] Ensure rustdoc generates clean documentation
- [ ] Include integration examples

**Verification**: Documentation is comprehensive and accurate, help output is useful

### Step 8: Final Integration and Testing
**Goal**: Ensure complete functionality and prepare for production use

#### 8.1: End-to-End Testing
- [ ] Test with real-world git repositories
- [ ] Verify filtering produces expected results
- [ ] Test edge cases and error conditions
- [ ] Benchmark performance with large repositories

#### 8.2: Code Quality Review
- [ ] Remove redundant comments per project guidelines
- [ ] Ensure consistent code style
- [ ] Verify error handling is comprehensive
- [ ] Check for potential security issues

#### 8.3: Final Validation
- [ ] Run complete test suite and verify 100% pass rate
- [ ] Test CLI help and usage scenarios
- [ ] Verify integration with GS-24 filtering system
- [ ] Confirm no breaking changes to existing functionality

**Verification**: Complete feature works as specified, ready for production use

## Testing Strategy

### Unit Test Coverage
- Date parsing: Test all formats, edge cases, error conditions
- CLI conversion: Test argument mapping, validation, error handling
- Integration: Test with scanner module, verify QueryParams usage

### Integration Test Coverage
- End-to-end filtering scenarios with real repositories
- Performance testing with large datasets
- Error handling in realistic failure scenarios
- CLI help and usage validation

### Test Data Requirements
- Sample git repositories with known commit patterns
- Edge case date ranges and formats
- Various path patterns and author combinations
- Large repository for performance testing

## Dependencies

### Required
- `clap` 4.x: Already available - CLI argument parsing with derive macros
- `chrono`: Already available - Date/time parsing and handling  
- GS-24 Filtering System: ✅ COMPLETED - Core filtering infrastructure

### Integration Points
- `src/scanner/query.rs`: QueryParams and DateRange structures
- `src/scanner/filters.rs`: FilterExecutor and filtering functionality
- `src/cli.rs`: Existing CLI argument structure

## Risk Assessment

### Technical Risks
- **Date parsing complexity**: Mitigated by using well-tested chrono library
- **CLI argument conflicts**: Mitigated by comprehensive validation
- **Performance with large repos**: Mitigated by existing early termination in GS-24

### Implementation Risks  
- **Integration complexity**: Mitigated by clear interfaces from GS-24
- **Test coverage**: Mitigated by TDD approach and property-based testing
- **User experience**: Mitigated by comprehensive help and validation

## Definition of Done

- [ ] All CLI filtering flags implemented with comprehensive validation
- [ ] Date parsing supports both absolute and relative formats
- [ ] Path filtering supports glob patterns with include/exclude
- [ ] Author filtering handles emails and names with include/exclude
- [ ] Integration with GS-24 filtering system completed
- [ ] CLI help updated with examples and comprehensive documentation
- [ ] Unit and integration tests achieving >95% coverage
- [ ] All existing tests continue to pass (100% pass rate)
- [ ] Code review completed and approved
- [ ] Performance tested with repositories >10k commits
- [ ] Documentation updated (README, help text, rustdoc)

## Future Enhancements (Out of Scope)

- Smart date parsing with natural language processing
- Path pattern completion suggestions  
- Author name auto-completion from git history
- Filter preset saving/loading functionality
- Interactive filter building mode
- Advanced regex pattern matching for paths
- Performance optimization for very large repositories (>100k commits)

## Notes

- This implementation directly supports testing the filtering system completed in GS-24
- CLI design follows existing patterns established in current Args structure
- Error messages should be clear and actionable to improve user experience
- Performance should leverage early termination mechanisms from GS-24
- All new code follows project TDD methodology and coding standards
