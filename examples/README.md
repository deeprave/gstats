# Configuration Examples

This directory contains example configuration files for gstats.

## gstats.toml

An example configuration file showing all available options with their default values and descriptions.

You can use this file as a starting point for your own configuration:

```bash
# Copy to your desired location
cp examples/gstats.toml ~/.config/gstats/gstats.toml

# Or use it directly
gstats --config-file examples/gstats.toml
```

The configuration system supports:
- **TOML format** with nested sections
- **Automatic discovery** in standard locations
- **CLI overrides** for all configuration options
- **Environment-specific** configurations using `--config-name`

See the main README.md for complete configuration documentation.
