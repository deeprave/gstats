# GS-6 Implementation Plan: Basic Git Statistics Collection

## Project: Git Repository Analytics Tool (gstats)
**YouTrack Issue:** GS-6 - "Basic Git Statistics Collection"  
**Date:** 26 July 2025  
**Phase:** Core Analysis - Git Statistics Foundation  

## Overview
This implementation plan establishes the core git analysis functionality for gstats. Building on the logging and CLI infrastructure from GS-1 through GS-5, this phase implements fundamental git repository analysis including commit analysis, contributor statistics, file tracking, branch analysis, and repository metrics. This forms the foundation for all advanced analytics features.

## Requirements Analysis
Based on the YouTrack issue description and core git analytics needs:

### Command-Based Architecture
Instead of flags like `--stats` and `--detailed`, implement a command-based interface where each analysis type is a subcommand:

- **`gstats stats`**: Overall repository statistics and summary
- **`gstats commits`**: Commit analysis (count, timeline, frequency patterns)
- **`gstats contributors`**: Contributor statistics (authors, activity patterns)
- **`gstats files`**: File change tracking (modifications, type distribution)
- **`gstats branches`**: Branch analysis (listing, merge patterns, activity)
- **`gstats activity`**: Activity analysis (trends, patterns, velocity)
- **`gstats loc`**: Lines of code analysis (language distribution, metrics)

### Common Command Arguments
All commands share common time-filtering and output options:
- **Time Filters**: `--since`, `--until`, `--between`, `--days`, `--month`, `--year`
- **Output Options**: `--format` (json/text), global logging flags
- **Multiple Commands**: Allow `gstats commits contributors --since=30d` for combined reports

### Module-to-Command Mapping
- Each command maps 1:1 with an analysis module
- Commands can be combined for comprehensive reports
- Shared functionality in common modules (time filtering, output formatting)
- Command-specific flags for specialized analysis options

## Acceptance Criteria
- [ ] **Commit Analysis Module**
  - [ ] Total commit count with date range filtering
  - [ ] Commit timeline analysis (commits per day/week/month)
  - [ ] Recent activity patterns and trends
  - [ ] Commit message analysis and categorization

- [ ] **Contributor Analysis Module**
  - [ ] Unique contributor identification and counting
  - [ ] Individual contributor commit statistics
  - [ ] Contributor activity timeline and patterns
  - [ ] Email and name normalization for contributors

- [ ] **File Change Analysis Module**
  - [ ] File modification tracking (added, modified, deleted)
  - [ ] File type distribution and statistics
  - [ ] Large file identification and tracking
  - [ ] Directory structure analysis

- [ ] **Branch Analysis Module**
  - [ ] Active branch listing and counting
  - [ ] Branch creation and merge pattern analysis
  - [ ] Main branch identification and statistics
  - [ ] Branch age and activity tracking

- [ ] **Repository Metrics Module**
  - [ ] Repository size calculation (files, lines of code)
  - [ ] File type distribution and language detection
  - [ ] Repository age and creation date
  - [ ] Overall activity and health metrics

- [ ] **Data Models and Structures**
  - [ ] Structured data models for all statistics
  - [ ] Serializable data structures for export
  - [ ] Efficient in-memory data representation
  - [ ] Clear data validation and error handling

- [ ] **Integration and Output**
  - [ ] CLI integration with new analysis commands
  - [ ] JSON and text output format support
  - [ ] Progress reporting for large repositories
  - [ ] Comprehensive logging throughout analysis

- [ ] **Testing and Quality**
  - [ ] Comprehensive unit tests for all analysis modules
  - [ ] Integration tests with real git repositories
  - [ ] Performance tests for large repository handling
  - [ ] Error handling tests for edge cases

## Implementation Strategy: Sub-Task Breakdown

**PREREQUISITE: GS-12 (Configuration File Support) must be completed first**

Given the scope of GS-6, this should be broken into focused sub-tasks, each implementing one command/module. However, before starting any GS-6 sub-tasks, GS-12 (Configuration File Support) must be completed to establish the configuration foundation that all analysis commands will use.

### Implementation Order:
1. **Complete GS-12**: Configuration File Support (REQUIRED FIRST)
2. **Begin GS-6 Sub-tasks**: After configuration foundation is established

### Proposed Sub-Tasks (YouTrack Issues)
These will be created as regular tasks with "is subtask of" relationship to GS-6:

1. **Command Infrastructure and Common Framework**
   - Command architecture and trait definitions  
   - Common argument parsing integration with configuration system
   - Output formatting framework
   - Command registry and dispatch

2. **Repository Overview Command**
   - Basic repository statistics and summary metrics
   - Health indicators and foundation for detailed analyses
   - Integration with configuration system

3. **Commit Analysis Command**  
   - Commit counting and timeline analysis
   - Frequency patterns, trends, and message analysis
   - Time filtering integration

4. **Contributor Analysis Command**
   - Contributor identification and statistics
   - Activity patterns, timeline analysis, email/name normalization
   - Configuration-based filtering options

5. **File Analysis Command**
   - File modification tracking and type distribution
   - Large file detection and directory structure analysis
   - Configurable file filtering and analysis options

6. **Branch Analysis Command**
   - Branch enumeration, activity tracking, merge pattern analysis
   - Branch health and lifecycle metrics
   - Remote branch configuration options

7. **Advanced Metrics Commands** 
   - Activity analysis, velocity metrics, lines of code analysis
   - Code complexity indicators and language detection
   - Configurable analysis parameters

8. **Integration Testing and Performance**
   - Cross-command integration testing and performance optimization
   - Large repository testing, benchmarking, and validation
   - Configuration system validation across all commands

Each sub-task would follow the TDD methodology and be independently deliverable, building on the common framework established in GS-6.1.

## Implementation Steps

### 1. Data Models and Core Structures (RED Phase)
**Objective:** Define data structures and create failing tests

#### 1.1 Create Statistics Data Models
- [ ] Create `src/stats.rs` module for core data structures
- [ ] Define `RepositoryStats` struct with all core metrics
- [ ] Define `CommitStats`, `ContributorStats`, `FileStats`, `BranchStats` structs
- [ ] Implement serialization support (serde) for all structures
- [ ] Add comprehensive validation for data structures

**Data Structures:**
```rust
// Core repository statistics container
pub struct RepositoryStats {
    pub repository_path: PathBuf,
    pub analysis_date: DateTime<Utc>,
    pub commit_stats: CommitStats,
    pub contributor_stats: ContributorStats,
    pub file_stats: FileStats,
    pub branch_stats: BranchStats,
    pub repository_metrics: RepositoryMetrics,
}

// Commit-related statistics
pub struct CommitStats {
    pub total_commits: u64,
    pub first_commit_date: Option<DateTime<Utc>>,
    pub last_commit_date: Option<DateTime<Utc>>,
    pub commits_by_day: Vec<(NaiveDate, u32)>,
    pub commits_by_author: HashMap<String, u32>,
    pub average_commits_per_day: f64,
}

// Contributor analysis
pub struct ContributorStats {
    pub total_contributors: u32,
    pub active_contributors_30d: u32,
    pub contributors: Vec<ContributorInfo>,
    pub top_contributors: Vec<(String, u32)>,
}

// File and content analysis
pub struct FileStats {
    pub total_files: u32,
    pub files_by_extension: HashMap<String, u32>,
    pub largest_files: Vec<(PathBuf, u64)>,
    pub most_changed_files: Vec<(PathBuf, u32)>,
    pub total_lines_of_code: Option<u64>,
}

// Branch analysis
pub struct BranchStats {
    pub total_branches: u32,
    pub active_branches: u32,
    pub main_branch: Option<String>,
    pub branch_list: Vec<BranchInfo>,
    pub recent_branches: Vec<(String, DateTime<Utc>)>,
}

// Repository-level metrics
pub struct RepositoryMetrics {
    pub repository_age_days: u32,
    pub repository_size_bytes: u64,
    pub primary_language: Option<String>,
    pub language_distribution: HashMap<String, f32>,
    pub activity_score: f32,
}
```

#### 1.2 Create Failing Tests for Data Models
- [ ] Test data structure creation and validation
- [ ] Test serialization/deserialization (JSON)
- [ ] Test data structure invariants and constraints
- [ ] Test error handling for invalid data

**Verification Test:** Data models compile and basic tests fail appropriately ✅

#### 1.3 Command Coordinator Architecture
- [ ] Create `src/commands/mod.rs` module for command coordination
- [ ] Define `AnalysisCommand` trait for extensible command implementation
- [ ] Create command registry and dispatch system
- [ ] Design shared argument parsing and validation
- [ ] Implement command result aggregation for multiple commands

**Command Architecture:**
```rust
// Common command interface
pub trait AnalysisCommand {
    fn name() -> &'static str;
    fn run(&self, repo: &Repository, args: &CommonArgs) -> Result<CommandOutput>;
    fn supports_time_filtering() -> bool { true }
    fn supports_output_formats() -> Vec<OutputFormat> { vec![OutputFormat::Text, OutputFormat::Json] }
}

// Command output for aggregation
pub struct CommandOutput {
    pub command_name: String,
    pub data: serde_json::Value,
    pub execution_time: Duration,
    pub error: Option<String>,
}

// Common arguments across all commands
pub struct CommonArgs {
    pub time_filter: Option<TimeFilter>,
    pub output_format: OutputFormat,
    pub repository_path: PathBuf,
    pub parallel: bool,
    pub progress: bool,
}

// Time filtering options
pub enum TimeFilter {
    Since(DateTime<Utc>),
    Until(DateTime<Utc>),
    Between(DateTime<Utc>, DateTime<Utc>),
    Days(u32),
    Month(u32, i32),  // month, year
    Year(i32),
}
```

### 2. Commit Analysis Implementation (GREEN Phase)
**Objective:** Implement commit analysis functionality

#### 2.1 Commit Data Collection
- [ ] Implement commit iteration using git2 crate
- [ ] Extract commit metadata (author, date, message, hash)
- [ ] Handle commit traversal performance for large repositories
- [ ] Implement date range filtering for commit analysis

#### 2.2 Commit Statistics Calculation
- [ ] Calculate total commit count with efficient iteration
- [ ] Build commit timeline data (daily/weekly/monthly aggregation)
- [ ] Analyze commit frequency patterns and trends
- [ ] Extract and categorize commit message patterns

#### 2.3 Commit Analysis Integration
- [ ] Integrate commit analysis with main application flow
- [ ] Add comprehensive logging for commit analysis progress
- [ ] Implement progress reporting for long-running analysis
- [ ] Add error handling for corrupted or inaccessible commits

**Verification Test:** Commit analysis produces accurate statistics ✅

### 3. Contributor Analysis Implementation (GREEN Phase)
**Objective:** Implement contributor identification and statistics

#### 3.1 Contributor Data Extraction
- [ ] Extract unique contributors from commit history
- [ ] Normalize contributor names and email addresses
- [ ] Handle multiple identities for same contributor
- [ ] Track contributor activity patterns over time

#### 3.2 Contributor Statistics Calculation
- [ ] Calculate per-contributor commit counts and patterns
- [ ] Identify most active contributors and timeframes
- [ ] Analyze contributor retention and activity trends
- [ ] Generate contributor activity heatmaps data

#### 3.3 Contributor Analysis Integration
- [ ] Integrate with commit analysis for unified data
- [ ] Add contributor-specific progress reporting
- [ ] Implement efficient contributor data structures
- [ ] Add validation for contributor data quality

**Verification Test:** Contributor analysis identifies and tracks contributors accurately ✅

### 4. File Change Analysis Implementation (GREEN Phase)
**Objective:** Implement file modification tracking and analysis

#### 4.1 File Change Detection
- [ ] Track file additions, modifications, and deletions
- [ ] Implement rename and move detection
- [ ] Calculate file change frequency and patterns
- [ ] Identify most frequently modified files

#### 4.2 File Statistics and Metrics
- [ ] Analyze file type distribution across repository
- [ ] Calculate repository structure and organization metrics
- [ ] Identify large files and potential repository bloat
- [ ] Estimate lines of code and content metrics

#### 4.3 File Analysis Integration
- [ ] Integrate file analysis with existing git operations
- [ ] Add file-specific progress tracking and logging
- [ ] Implement efficient file change data structures
- [ ] Handle binary files and special file types appropriately

**Verification Test:** File analysis tracks changes and calculates metrics accurately ✅

### 5. Branch Analysis Implementation (GREEN Phase)
**Objective:** Implement branch analysis and tracking

#### 5.1 Branch Data Collection
- [ ] Enumerate all repository branches (local and remote)
- [ ] Track branch creation dates and last activity
- [ ] Identify main/default branch automatically
- [ ] Analyze branch merge patterns and relationships

#### 5.2 Branch Statistics Calculation
- [ ] Calculate branch activity and health metrics
- [ ] Identify stale and active branches
- [ ] Analyze branching strategies and patterns
- [ ] Track branch lifecycle and merge frequency

#### 5.3 Branch Analysis Integration
- [ ] Integrate branch analysis with commit analysis
- [ ] Add branch-specific logging and progress tracking
- [ ] Implement efficient branch data representation
- [ ] Handle edge cases (orphaned branches, etc.)

**Verification Test:** Branch analysis identifies and tracks branches accurately ✅

### 6. Repository Metrics Implementation (GREEN Phase)
**Objective:** Implement overall repository health and metrics

#### 6.1 Repository Size and Structure
- [ ] Calculate total repository size and content metrics
- [ ] Analyze directory structure and organization
- [ ] Identify repository growth patterns over time
- [ ] Calculate storage efficiency and optimization opportunities

#### 6.2 Language and Content Analysis
- [ ] Detect primary programming languages in repository
- [ ] Calculate language distribution and percentages
- [ ] Identify configuration, documentation, and code ratios
- [ ] Analyze code complexity indicators

#### 6.3 Activity and Health Metrics
- [ ] Calculate overall repository activity scores
- [ ] Analyze development velocity and consistency
- [ ] Identify potential maintenance and health issues
- [ ] Generate repository quality indicators

**Verification Test:** Repository metrics provide accurate overall statistics ✅

### 7. CLI Integration and Commands (GREEN Phase)
**Objective:** Implement command-based CLI interface for analysis

#### 7.1 Command Structure Design
- [ ] Implement subcommand architecture using clap's `Command` derive
- [ ] Create base command structure with shared arguments
- [ ] Design command-to-module mapping architecture
- [ ] Implement multiple command execution in single run

**Command Examples:**
```bash
# Individual analysis commands
gstats stats                          # Overall repository summary
gstats commits --since=30d            # Recent commit analysis
gstats contributors --month=2025-07   # Monthly contributor stats
gstats files --format=json            # File analysis as JSON
gstats branches --detailed             # Detailed branch analysis
gstats activity --between=2025-01-01,2025-06-30  # Activity in date range

# Combined commands
gstats commits contributors --since=7d  # Multiple analyses
gstats stats files branches             # Combined overview
```

#### 7.2 Common Command Arguments
- [ ] Implement shared time filtering: `--since`, `--until`, `--between`, `--days`, `--month`, `--year`
- [ ] Global output formatting: `--format` (json/text), inherited logging flags
- [ ] Repository targeting: positional path argument (inherits from global)
- [ ] Performance options: `--parallel`, `--cache`, `--progress`

#### 7.3 Command-Specific Arguments
- [ ] **stats**: `--summary`, `--detailed`
- [ ] **commits**: `--per-day`, `--per-week`, `--per-month`, `--messages`
- [ ] **contributors**: `--top=N`, `--active-only`, `--normalize-emails`
- [ ] **files**: `--extensions`, `--largest=N`, `--most-changed=N`
- [ ] **branches**: `--active-only`, `--include-remote`, `--merge-analysis`
- [ ] **activity**: `--velocity`, `--patterns`, `--heatmap`
- [ ] **loc**: `--by-language`, `--by-file`, `--exclude-patterns`

**Verification Test:** CLI integration provides expected output formats ✅

### 8. Testing and Quality Assurance (REFACTOR Phase)
**Objective:** Comprehensive testing and code quality

#### 8.1 Unit Testing
- [ ] Test all analysis modules individually
- [ ] Test data structure validation and serialization
- [ ] Test error handling and edge cases
- [ ] Test performance with synthetic data

#### 8.2 Integration Testing
- [ ] Test with real git repositories of various sizes
- [ ] Test CLI integration with different options
- [ ] Test output format consistency and correctness
- [ ] Test memory usage and performance characteristics

#### 8.3 Quality and Documentation
- [ ] Code review and refactoring for clarity
- [ ] Documentation for all public APIs
- [ ] Performance benchmarking and optimization
- [ ] User documentation and examples

**Verification Test:** All tests pass and code quality meets standards ✅

## Technical Implementation Details

### Dependencies and Crates
```toml
# Core git operations (already included)
git2 = "0.18"

# Date and time handling (already included)
chrono = { version = "0.4", features = ["serde"] }

# Data serialization (already included)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Performance and parallel processing
rayon = "1.7"  # For parallel iterator processing

# Statistical analysis (optional)
# statistical = "1.0"  # For advanced statistical calculations
```

### Performance Considerations
- **Lazy Loading**: Load git data on-demand to minimize memory usage
- **Parallel Processing**: Use rayon for CPU-intensive analysis tasks
- **Caching**: Cache analysis results to avoid repeated computation
- **Streaming**: Process large datasets in chunks to manage memory
- **Progress Reporting**: Provide feedback for long-running operations

### Error Handling Strategy
- **Graceful Degradation**: Continue analysis even when some data is unavailable
- **Detailed Error Context**: Provide specific error information for debugging
- **Recovery Mechanisms**: Attempt to recover from non-fatal errors
- **User-Friendly Messages**: Convert technical errors to user-understandable messages

### Memory Management
- **Efficient Data Structures**: Use appropriate collections for data volume
- **Resource Cleanup**: Ensure proper cleanup of git resources
- **Memory Monitoring**: Track memory usage during analysis
- **Chunk Processing**: Process large datasets in manageable chunks

## Integration with Existing Infrastructure

### Logging Integration
- Use existing logging infrastructure from GS-5
- Add analysis-specific log levels and categories
- Include performance metrics in log output
- Provide detailed debugging information for analysis

### CLI Integration
- Extend existing CLI argument parsing from GS-4 with subcommand support
- Maintain consistent global argument naming and behavior (logging, format)
- Add command-specific argument parsing and validation
- Support command combination for comprehensive analysis
- Provide backward compatibility with existing repository path handling

### Git Integration
- Build on git repository validation from GS-2
- Extend git operations with analysis-specific functionality
- Maintain consistent error handling patterns
- Support all git repository types and configurations

## Future Extensibility

This implementation will prepare for:
- Advanced complexity analysis (GS-7)
- Data export and visualization features (GS-8)
- Performance optimization for very large repositories
- Plugin architecture for custom analysis modules
- Real-time analysis and monitoring capabilities
- Integration with external tools and services

## Next Phase Preparation

Upon completion of this phase:
1. Move GS-6 to "Queued" state
2. Update project documentation with new capabilities
3. Create benchmarks for analysis performance
4. Begin planning for GS-7 (Advanced Code Complexity Analysis)
5. Gather user feedback on analysis output and usability

## Notes
- This phase establishes the core foundation for all git analytics
- Follows TDD methodology with comprehensive test coverage
- Focuses on accuracy, performance, and extensibility
- Provides both programmatic (JSON) and human-readable output
- Designed for integration with future advanced analytics features
- **Status:** 🔄 READY FOR IMPLEMENTATION
