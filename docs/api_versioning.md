# API Versioning System

## Overview

The scanner module uses a **Cargo.toml-based API versioning system** that provides stable, reproducible versions across all developers and environments while allowing for manual increments when breaking changes occur.

## How It Works

### Source-Controlled Versioning
- API version is defined in `Cargo.toml` under `package.metadata.gstats.api_version`
- Build script reads version from Cargo.toml and generates constant at build time
- **Same source code always produces same API version** across all developers
- No dependency on build date or environment

### Current Version
```bash
# Check current API version in source
grep 'api_version' Cargo.toml

# Check generated constant  
grep 'BASE_API_VERSION' target/*/build/gstats-*/out/version_api.rs
```

### Version Increment Process
```bash
# Method 1: Use the provided script
./scripts/increment_api_version.sh

# Method 2: Manual edit
# Edit Cargo.toml: package.metadata.gstats.api_version = YYYYMMDD
# Commit the change and rebuild
```

## Benefits

### Reproducible Builds
- ✅ **Same source = same version**: All developers get identical API versions
- ✅ **Source controlled**: Version changes are tracked in git history  
- ✅ **CI/CD friendly**: Build servers produce same versions as development machines
- ✅ **Plugin stability**: Consistent API versioning for plugin compatibility

### Development Workflow
- 🔧 **Easy increment**: Edit single line in Cargo.toml
- 📝 **Clear audit trail**: Version changes visible in git commits
- 🚀 **Zero setup**: No environment variables or special build requirements

Whenever api_version version changes, regardless the method, `Cargo.toml` needs to be committed.

## Version Format

Uses **YYYYMMDD** format for human-readable dates:
- `20250727` = 27 July 2025
- `20250801` = 1 August 2025  
- `20251215` = 15 December 2025

## API Functions

```rust
// Get current API version (YYYYMMDD format)
let version = gstats::scanner::get_api_version(); // Returns 20250727

// Check if a version is compatible  
let compatible = gstats::scanner::is_compatible_version(version);

// Get version metadata as JSON
let info = gstats::scanner::get_version_info();
// Returns: {"api_version": 20250727, "release_date": "2025-07-27", ...}

// Check API compatibility
let api_ok = gstats::scanner::is_api_compatible(required_version);
```

## Implementation Details

### Cargo.toml Configuration
```toml
[package.metadata.gstats]
api_version = 20250727
```

### Build Script (`build.rs`)
- Reads `package.metadata.gstats.api_version` from Cargo.toml
- Generates `version_api.rs` with `BASE_API_VERSION` constant
- Triggers rebuild when Cargo.toml changes

### Version Module (`src/scanner/version.rs`)
- Includes generated constant via `include!()` macro
- Converts YYYYMMDD to human-readable dates
- Implements compatibility checking logic

## When to Increment

Increment the API version when making:
- ✋ **Breaking changes** to scanner traits or interfaces
- 📦 **Message format changes** affecting serialisation
- ⚙️ **Configuration structure** modifications
- 🔌 **Plugin compatibility** updates

## Developer Scenarios

## Testing

The version system includes comprehensive tests:
- Version stability across builds
- YYYYMMDD format validation  
- Date conversion functionality
- API integration tests

All 86 tests pass with the new versioning system.
