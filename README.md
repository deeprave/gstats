![Project Status](https://img.shields.io/badge/Status-Under Development-red) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE.md)
# Git Repository Analytics Tool
A fast, local-first git analytics tool for analyzing Git repositories with comprehensive logging and CLI interface.

## Current Features
- **Git Repository Detection** - Automatic validation of Git repositories
- **Comprehensive CLI Interface** - Command-line argument parsing with validation
- **Advanced Logging System** - Structured logging with JSON/text formats, configurable levels, and file output
- **Timestamp Formatting** - Standardized YYYY-MM-DD HH:MM:SS timestamp format
- **Multiple Log Destinations** - Console and file logging with independent log levels
- **Configurable Output** - Support for verbose, quiet, and JSON logging modes

## Planned Features
- Code complexity trends over time
- Contributor statistics and visualizations
- Performance metrics for large repositories
- Export to various formats (JSON, CSV, etc.)
- Repository URL support for remote analysis

## Usage

### Basic Usage
```bash
# Analyze current directory (must be a Git repository)
gstats .

# Analyze specific Git repository path
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

### CLI Help
```bash
gstats --help
```
