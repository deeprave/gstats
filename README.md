![Project Status](https://img.shields.io/badge/Status-Alpha-orange)
<!-- noinspection MarkdownUnresolvedFileReference -->
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE.md)
# Git Repository Analytics Tool
A fast, local-first git analytics tool for analysing Git repositories with comprehensive logging, CLI interface, and extensible plugin architecture.

## Current Features
- **Git Repository Detection** - Automatic validation of Git repositories
- **Comprehensive CLI Interface** - Command-line argument parsing with validation
- **Advanced Logging System** - Structured logging with JSON/text formats, configurable levels, and file output
- **Configuration File Support** - TOML-based configuration with discovery hierarchy and CLI overrides
- **Async Scanner Engine** - High-performance async repository scanning with streaming data processing
- **Memory-Conscious Queue System** - Efficient message queue with memory pressure handling and backoff algorithms
- **Plugin Architecture** - Extensible plugin system with trait-based design and async communication
- **Built-in Plugins** - Commits analysis, code metrics, and data export plugins
- **Plugin Management** - CLI-based plugin discovery, validation, and execution
- **Real-time Processing** - Streaming plugin processing with backpressure handling
- **Multiple Log Destinations** - Console and file logging with independent log levels
- **Section-based Configuration** - Module-specific settings with inheritance and override capabilities
- **Color-coded Output** - Enhanced visual feedback with color-coded logging, plugin results, and progress indicators (console output only)
- **Progress Indicators** - Visual feedback with spinner animations for long-running operations
- **Terminal Compatibility** - Automatic color detection with graceful fallback to plain text for non-color terminals
- **Accessibility Support** - NO_COLOR environment variable and --no-color flag compliance
- **Configurable Themes** - Auto-detection, light, dark, and custom color themes via configuration files

## Planned Features
- External plugin loading and dynamic discovery
- Advanced visualization and reporting
- Performance metrics for large repositories
- Extended export formats and destinations
- Repository URL support for remote analysis
- Web interface for interactive analytics

## Usage

### Basic Usage
```bash
# Analyse current directory (must be a Git repository)
gstats .

# Analyse specific Git repository path
gstats /path/to/repository
```

### Logging Options
```bash
# Verbose output with debug information
gstats --verbose .

# Quiet mode (errors only)
gstats --quiet .

# JSON formatted logs
gstats --log-format json .

# Log to file with custom level
gstats --log-file output.log --log-file-level debug .

# Combine options
gstats --verbose --log-format json --log-file debug.log .
```

### Color and Visual Options
```bash
# Force colors (default for console output)
gstats --color commits

# Disable colors (automatic for non-tty output)
gstats --no-color commits

# Disable colors via environment variable
NO_COLOR=1 gstats commits

# Export complete configuration file
gstats --export-config gstats-config.toml commits
```

### Plugin Management
```bash
# List available plugins
gstats --list-plugins

# Get plugin information
gstats --plugin-info commits

# List plugins by type
gstats --list-by-type scanner

# Use specific plugins (default: commits)
gstats --plugins commits,metrics,export .
```

### Configuration File Support
```bash
# Use explicit configuration file
gstats --config-file /path/to/config.toml .

# Use configuration section for environment-specific settings
gstats --config-name dev .

# Combine configuration with CLI overrides (CLI takes precedence)
gstats --config-file config.toml --verbose .
```

#### Configuration File Format
Create a TOML configuration file with root-level settings and module-specific sections:

```toml
# Root-level global settings (console output only)
quiet = false
log-format = "text"
log-file = "/tmp/gstats.log"
color = true                    # Enable colors (default: auto-detect)
theme = "auto"                  # Options: auto, light, dark, custom
colors = { error = "red", warning = "yellow", info = "blue", debug = "bright_black", success = "green", highlight = "cyan" }

# Scanner configuration
[scanner]
max-memory = "64MB"
queue-size = 1000

# Module-specific settings
[module.commits]
since = "30d"
per-day = true
format = "json"

[module.contributors]
top = 10
normalise-emails = true
```

#### Configuration Discovery
Configuration files are automatically discovered in this order:
1. `--config-file <path>` (explicit CLI override)
2. `$GSTATS_CONFIG` environment variable
3. `~/.config/gstats/config.toml` (XDG standard)
4. `~/.gstats.toml` (home directory)
5. `./.gstats.toml` (project local)

See `examples/gstats.toml` for a complete configuration example.

### CLI Help
```bash
gstats --help
```

#### Available CLI Options

**Logging Options:**
- `--verbose, -v` - Enable verbose output (debug level logging)
- `--quiet, -q` - Enable quiet mode (error level logging only)
- `--debug` - Enable debug output (trace level logging)
- `--log-format <FORMAT>` - Set log format: text or json (default: text)
- `--log-file <FILE>` - Log file path for file output
- `--log-file-level <LEVEL>` - Log level for file output (independent of console)

**Configuration Options:**
- `--config-file <FILE>` - Configuration file path
- `--config-name <SECTION>` - Configuration section name for environment-specific settings

**Plugin Options:**
- `--plugins <LIST>` - Comma-separated list of plugins to use
- `--list-plugins` - List all available plugins
- `--plugin-info <NAME>` - Get detailed information about a plugin
- `--list-by-type <TYPE>` - List plugins by type (scanner, processing, output, notification)

All options can be configured via configuration file, with CLI arguments taking precedence.

## Scan Modes

gstats supports multiple scan modes that can be combined to provide comprehensive repository analysis:

### Available Scan Modes
- **FILES** - Scan file system structure and content
- **HISTORY** - Scan git history and commits
- **METRICS** - Scan for code metrics and statistics
- **DEPENDENCIES** - Scan for dependencies and imports
- **SECURITY** - Scan for security vulnerabilities
- **PERFORMANCE** - Scan for performance bottlenecks
- **CHANGE_FREQUENCY** - Analyse file change frequency patterns

### Mode Combinations
Scan modes can be combined using bitwise operations to enable multiple analysis types:
- `FILES | HISTORY` - Analyse both file structure and git history
- `HISTORY | CHANGE_FREQUENCY` - Combine commit analysis with change frequency patterns
- `FILES | METRICS | SECURITY` - Comprehensive code analysis

### Plugin Support
Each plugin declares which scan modes it supports:
- **Commits Plugin** - Supports `HISTORY` mode
- **Metrics Plugin** - Supports `FILES | SECURITY` modes
- **Export Plugin** - Supports all modes for data export

The scanner engine automatically provides the appropriate data streams based on the requested modes, enabling plugins to focus on analysis rather than data collection.

## Architecture Overview

gstats is built on a modern, async architecture designed for performance and extensibility:

### Core Components
- **Async Scanner Engine** - High-performance repository scanning with streaming data processing
- **Plugin System** - Trait-based plugin architecture with async communication interfaces
- **Memory-Conscious Queue** - Efficient message handling with backpressure and memory management
- **Configuration System** - Hierarchical TOML-based configuration with CLI overrides

### Plugin Architecture
The plugin system provides a flexible, extensible foundation for repository analysis:

```
CLI Args → Plugin Registry → Scanner Engine → Plugin Scanners → 
Plugin Executor → Message Queue → Consumer → Plugin Processing
```

**Built-in Plugins:**
- **Commits Plugin** - Analyses commit history and patterns
- **Metrics Plugin** - Calculates code metrics and statistics
- **Export Plugin** - Handles data export to various formats

**Plugin Types:**
- **Scanner** - Process repository data during scanning
- **Processing** - Transform and analyse scan results
- **Output** - Handle result formatting and export
- **Notification** - Respond to system events and updates

For detailed information about the plugin system, see [PLUGIN_GUIDE.md](PLUGIN_GUIDE.md).
For complete architecture documentation, see [ARCHITECTURE.md](ARCHITECTURE.md).
