![Project Status](https://img.shields.io/badge/Status-Under Development-red)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE.md)
# Git Repository Analytics Tool
A fast, local-first git analytics tool for analysing Git repositories with comprehensive logging and CLI interface.

## Current Features
- **Git Repository Detection** - Automatic validation of Git repositories
- **Comprehensive CLI Interface** - Command-line argument parsing with validation
- **Advanced Logging System** - Structured logging with JSON/text formats, configurable levels, and file output
- **Configuration File Support** - TOML-based configuration with discovery hierarchy and CLI overrides
- **Timestamp Formatting** - Standardised YYYY-MM-DD HH:MM:SS timestamp format
- **Multiple Log Destinations** - Console and file logging with independent log levels
- **Configurable Output** - Support for verbose, quiet, and JSON logging modes
- **Section-based Configuration** - Module-specific settings with inheritance and override capabilities

## Planned Features
- Code complexity trends over time
- Contributor statistics and visualisations
- Performance metrics for large repositories
- Export to various formats (JSON, CSV, etc.)
- Repository URL support for remote analysis

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
Create a TOML configuration file with section-based organisation:

```toml
# Global settings
[base]
quiet = true
log-format = "json"
log-file = "/tmp/gstats.log"
log-file-level = "info"

# Module-specific settings (for future features)
[module.commits]
since = 30d
per-day = true
format = json

[module.contributors]
top = 10
normalize-emails = true
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
- `--verbose, -v` - Enable verbose output (debug level logging)
- `--quiet, -q` - Enable quiet mode (error level logging only)
- `--debug` - Enable debug output (trace level logging)
- `--log-format <FORMAT>` - Set log format: text or json (default: text)
- `--log-file <FILE>` - Log file path for file output
- `--log-file-level <LEVEL>` - Log level for file output (independent of console)
- `--config-file <FILE>` - Configuration file path
- `--config-name <SECTION>` - Configuration section name for environment-specific settings

All logging options can be configured via configuration file, with CLI arguments taking precedence.
