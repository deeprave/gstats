# GS-12 Implementation Plan: Configuration File Support

## Project: Git Repository Analytics Tool (gstats)
**YouTrack Issue:** GS-12 - "Configuration File Support"  
**Date:** 26 July 2025  
**Phase:** Infrastructure Enhancement - Configuration Management  
**Priority:** HIGH - Must be completed before GS-6 implementation

## Overview
This implementation establishes a comprehensive configuration file system to manage the growing complexity of CLI flags and options. As we expand into multiple analysis commands (GS-6 and beyond), the number of configuration options will grow significantly. A robust configuration system prevents CLI bloat and provides better user experience through persistent settings.

## Requirements Analysis
Based on the need to manage complex CLI configurations:

### Configuration File Features
- **File Specification**: `--config-file <filename>` with optional default location
- **Section Support**: `--config-name <section>` for command-specific configurations
- **Format Choice**: TOML preferred (flexible, structured), INI as fallback option
- **Override Hierarchy**: Command-line args override config file values
- **Global + Section Structure**: Common settings at root, command-specific in sections
- **Default Locations**: Standard config directories with automatic detection

### Configuration Structure Example
```toml
# Global settings (apply to all commands)
quiet = true
verbose = false
log-format = "text"
log-file = "/tmp/gstats.log"

# Command-specific sections
[commits]
since = "30d"
format = "json"
per-day = true

[contributors]
top = 10
normalize-emails = true
active-only = false

[files]
extensions = ["rs", "toml", "md"]
largest = 5
```

## Acceptance Criteria
- [ ] **Configuration File Loading**
  - [ ] Support `--config-file <path>` argument
  - [ ] Default configuration file location discovery
  - [ ] TOML format parsing with comprehensive error handling
  - [ ] Optional INI format support for simpler use cases

- [ ] **Section-Based Configuration**
  - [ ] Support `--config-name <section>` for specific sections
  - [ ] Global settings at root level
  - [ ] Command-specific settings in named sections
  - [ ] Section inheritance and override mechanisms

- [ ] **CLI Integration**
  - [ ] Seamless integration with existing CLI argument parsing
  - [ ] Command-line arguments override config file values
  - [ ] Preserved argument validation and error handling
  - [ ] Backward compatibility with existing CLI usage

- [ ] **Time Filter Configuration**
  - [ ] Flexible time filter parsing from config (`since`, `until`, `between`)
  - [ ] Support for aliases (`since` vs `from`, etc.)
  - [ ] Shared time filter parser across all commands
  - [ ] Human-readable time formats (30d, 2025-07, last-month)

- [ ] **Configuration Validation**
  - [ ] Comprehensive validation of config file structure
  - [ ] Type checking for all configuration values
  - [ ] Clear error messages for invalid configurations
  - [ ] Configuration schema documentation

- [ ] **Default and Discovery**
  - [ ] Standard configuration file locations (XDG, home directory)
  - [ ] Automatic config file discovery without explicit `--config-file`
  - [ ] Configuration file creation assistance and templates
  - [ ] Environment variable support for config paths

## Implementation Steps

### 1. Configuration Infrastructure (RED Phase)
**Objective:** Establish configuration parsing and loading foundation

#### 1.1 Create Configuration Module
- [ ] Create `src/config.rs` module for configuration management
- [ ] Define `Configuration` struct with all settings
- [ ] Implement TOML parsing using `toml` crate
- [ ] Add comprehensive error handling for parsing failures

#### 1.2 Configuration Data Structures
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Configuration {
    // Global settings
    pub quiet: Option<bool>,
    pub verbose: Option<bool>,
    pub debug: Option<bool>,
    pub log_format: Option<String>,
    pub log_file: Option<PathBuf>,
    pub log_file_level: Option<String>,
    
    // Command-specific sections
    pub commits: Option<CommitsConfig>,
    pub contributors: Option<ContributorsConfig>,
    pub files: Option<FilesConfig>,
    pub branches: Option<BranchesConfig>,
    pub activity: Option<ActivityConfig>,
    pub loc: Option<LocConfig>,
    
    // Custom sections for future extensibility
    #[serde(flatten)]
    pub custom_sections: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CommitsConfig {
    pub since: Option<String>,
    pub until: Option<String>,
    pub between: Option<String>,
    pub per_day: Option<bool>,
    pub per_week: Option<bool>,
    pub per_month: Option<bool>,
    pub messages: Option<bool>,
}

// Similar structs for other commands...
```

#### 1.3 Configuration Loading and Discovery
- [ ] Implement config file discovery in standard locations
- [ ] Support explicit `--config-file` specification
- [ ] Handle missing config files gracefully
- [ ] Implement config file validation and error reporting

**Verification Test:** Configuration parsing and loading works correctly ✅

### 2. CLI Integration (GREEN Phase)
**Objective:** Integrate configuration with existing CLI argument parsing

#### 2.1 Enhanced CLI Argument Structure
- [ ] Add `--config-file` and `--config-name` to main CLI args
- [ ] Modify existing `Args` struct to support configuration
- [ ] Implement configuration merging with CLI arguments
- [ ] Ensure CLI args override config file values

#### 2.2 Configuration Resolution
- [ ] Load configuration file before CLI argument processing
- [ ] Merge config values with CLI arguments (CLI takes precedence)
- [ ] Handle section-specific configurations
- [ ] Validate final merged configuration

#### 2.3 Backward Compatibility
- [ ] Ensure existing CLI usage continues to work
- [ ] Maintain all existing argument validation
- [ ] Preserve error messages and help text
- [ ] No breaking changes to current functionality

**Verification Test:** CLI integration maintains existing behavior while adding config support ✅

### 3. Time Filter System (GREEN Phase)
**Objective:** Implement flexible time filtering shared across commands

#### 3.1 Time Filter Parser
- [ ] Create shared time filter parsing module
- [ ] Support multiple time formats (ISO dates, relative times)
- [ ] Implement aliases (`since`/`from`, `until`/`to`, etc.)
- [ ] Handle human-readable formats (30d, last-month, 2025-07)

#### 3.2 Time Filter Configuration
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TimeFilter {
    Since(DateTime<Utc>),
    Until(DateTime<Utc>),
    Between(DateTime<Utc>, DateTime<Utc>),
    Days(u32),
    Weeks(u32),
    Months(u32),
    Years(u32),
}

impl TimeFilter {
    pub fn parse(input: &str) -> Result<Self> {
        // Parse various time formats:
        // - "30d", "7w", "3m", "1y"
        // - "2025-07-26", "2025-07", "2025"
        // - "last-week", "last-month", "yesterday"
        // - "2025-01-01,2025-06-30" (between)
    }
}
```

#### 3.3 Time Filter Integration
- [ ] Integrate time filters with configuration system
- [ ] Support time filters in all command sections
- [ ] Provide validation and error handling
- [ ] Create comprehensive time filter documentation

**Verification Test:** Time filtering works from both CLI and config sources ✅

### 4. Configuration Validation and Defaults (GREEN Phase)
**Objective:** Robust configuration validation and sensible defaults

#### 4.1 Configuration Schema
- [ ] Define complete configuration schema
- [ ] Implement validation for all configuration sections
- [ ] Provide clear error messages for invalid values
- [ ] Support configuration documentation generation

#### 4.2 Default Configuration
- [ ] Establish sensible default values for all settings
- [ ] Create default configuration file template
- [ ] Implement configuration file generation command
- [ ] Support different configuration profiles (dev, prod, etc.)

#### 4.3 Environment Integration
- [ ] Support environment variable overrides
- [ ] Standard environment variable naming convention
- [ ] Integration with XDG configuration directories
- [ ] Support for multiple configuration files

**Verification Test:** Configuration validation catches errors and provides helpful feedback ✅

### 5. Testing and Documentation (REFACTOR Phase)
**Objective:** Comprehensive testing and user documentation

#### 5.1 Testing Strategy
- [ ] Unit tests for configuration parsing and validation
- [ ] Integration tests with CLI argument processing
- [ ] Test configuration override behavior
- [ ] Test error handling and edge cases

#### 5.2 Documentation and Examples
- [ ] Complete configuration file documentation
- [ ] Example configuration files for different use cases
- [ ] Migration guide for existing users
- [ ] Configuration best practices and recommendations

#### 5.3 Performance and Optimization
- [ ] Optimize configuration loading performance
- [ ] Cache configuration parsing results
- [ ] Minimize memory usage for large configurations
- [ ] Profile configuration loading overhead

**Verification Test:** All tests pass and documentation is complete ✅

## Technical Implementation Details

### Dependencies
```toml
# Configuration parsing
toml = "0.8"           # TOML parsing
serde = { version = "1.0", features = ["derive"] }

# Optional INI support
ini = "1.3"            # INI format parsing (optional)

# Path and environment handling
dirs = "5.0"           # Standard directory locations
```

### Configuration File Locations
1. **Explicit**: `--config-file /path/to/config.toml`
2. **User Config**: `~/.config/gstats/config.toml` (XDG)
3. **User Home**: `~/.gstats.toml`
4. **Project Local**: `./.gstats.toml` (repository-specific)
5. **Environment**: `$GSTATS_CONFIG_FILE`

### Override Hierarchy (highest to lowest priority)
1. Command-line arguments
2. Environment variables  
3. Explicit config file (`--config-file`)
4. User config directory
5. User home directory
6. Project local config
7. Built-in defaults

## Integration with Existing Infrastructure

### CLI Integration
- Extends existing `src/cli.rs` without breaking changes
- Maintains all current argument parsing and validation
- Adds configuration loading before argument processing
- Preserves help text and error messages

### Logging Integration
- Configuration for logging level, format, and file output
- Inherits from GS-5 logging infrastructure
- Supports per-command logging configuration
- Environment variable integration

### Future Command Integration
- Provides foundation for GS-6 command-specific configurations
- Extensible section system for new commands
- Shared time filtering and common arguments
- Plugin-ready architecture for custom commands

## Next Phase Preparation

Upon completion of GS-12:
1. Update GS-6 plan to use configuration system
2. Begin GS-6 implementation with configuration foundation
3. Create example configuration files for documentation
4. Gather user feedback on configuration experience
5. Prepare configuration templates for common use cases

## Notes
- This system provides the foundation for all future CLI complexity
- Follows Rust configuration best practices and standards
- Designed for extensibility as new commands are added
- Maintains backward compatibility with existing CLI usage
- **Status:** 🔄 READY FOR IMPLEMENTATION
