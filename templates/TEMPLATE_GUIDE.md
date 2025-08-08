# Template Usage Guide

This guide explains which templates to use with different gstats commands and provides usage examples.

## Available Templates

### 1. `commit-summary.j2` - General Repository Analysis (Markdown)
**Best for:** `commits` command
**Output:** Clean markdown report with commit history, file activity, and contributors

```bash
# Default report (10 files, 5 commits)
gstats commits --template templates/commit-summary.j2 --output summary.md

# Full report with all data
gstats commits --all --template templates/commit-summary.j2 --output full-summary.md

# Limited report
gstats commits --output-limit 20 --template templates/commit-summary.j2 --output limited-summary.md
```

### 2. `authors-report.j2` - Author-Focused Analysis (Markdown)
**Best for:** `authors` command
**Output:** Detailed breakdown of author contributions

```bash
# Author contribution report
gstats authors --template templates/authors-report.j2 --output authors.md

# All contributors (useful for large teams)
gstats authors --all --template templates/authors-report.j2 --output all-authors.md
```

### 3. `team-report.html.j2` - Visual Team Dashboard (HTML)
**Best for:** `commits` command when you want a visual report
**Output:** Styled HTML report with charts and statistics
**Note:** Currently has syntax issues - being fixed

```bash
# When fixed, use like this:
# gstats commits --template templates/team-report.html.j2 --output team-dashboard.html
```

### 4. `data-export.json.j2` - Raw Data Export (JSON)
**Best for:** Any command when you need structured data
**Output:** JSON format for further processing

```bash
# Export raw data as JSON
gstats commits --template templates/data-export.json.j2 --output data.json
gstats authors --template templates/data-export.json.j2 --output authors-data.json
```

### 5. `security-audit.j2` - Security-Focused Analysis (Markdown)
**Best for:** `commits` command for security reviews
**Output:** Security-focused analysis of commits and file changes

```bash
# Security audit report
gstats commits --template templates/security-audit.j2 --output security-audit.md
```

## Command-Template Compatibility

| Command | Recommended Templates | Notes |
|---------|----------------------|-------|
| `commits` | `commit-summary.j2`, `team-report.html.j2`, `security-audit.j2` | General repository analysis |
| `authors` | `authors-report.j2`, `data-export.json.j2` | Author-focused reports |
| `metrics` | `data-export.json.j2` | Raw metrics data |
| `export` | Any template | Direct template processing |

## Template Features

All templates support:
- `--all` flag (shows all data instead of defaults)
- `--output-limit N` flag (limits output to N items)
- Custom variables via `--template-var key=value`

## Custom Filters Available

- `number_format` - Formats numbers with commas (e.g., 1000 → 1,000)
- `percentage` - Formats percentages (e.g., 0.15 → 15%)
- `slice(end=N)` - Limits arrays to first N items
- `first_line` - Extracts first line from multi-line strings
- `round(precision=N)` - Rounds numbers to N decimal places
- `date` - Formats ISO timestamps to readable dates

## Getting Help

```bash
# Get template help and available data
gstats commits --template-help

# List available template variables
gstats authors --template-help
```

## Creating Custom Templates

Templates use Jinja2 syntax with the following main data sections:
- `repository` - Repository information
- `scan_config` - Scan configuration (output_all, output_limit)
- `statistics` - Overall statistics
- `authors` - Author contribution data
- `files` - File modification data  
- `commits` - Commit history data

Example minimal template:
```jinja2
# {{ repository.name }} Report

Total commits: {{ statistics.total_commits }}
Authors: {{ authors.total_authors }}

{% for author in authors.list %}
- {{ author.name }}: {{ author.commits }} commits
{% endfor %}
```