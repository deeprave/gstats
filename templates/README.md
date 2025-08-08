# gstats Template Examples

This directory contains example templates demonstrating the gstats template system capabilities. Templates use [Tera](https://tera.netlify.app/) (Jinja2-like) syntax for flexible output formatting.

**💡 QUICK START:** Each template file has a header comment showing exactly which command to use it with and example usage!

## Available Templates

### 1. commit-summary.j2
**Purpose:** Simple Markdown summary of repository activity  
**Use case:** Quick project overviews, README generation  
**Output format:** Markdown

```bash
gstats commits --template templates/commit-summary.j2 --output summary.md
```

### 2. team-report.html.j2
**Purpose:** Comprehensive HTML report with styling and charts  
**Use case:** Team retrospectives, management reports  
**Output format:** HTML with embedded CSS

```bash
gstats commits --template templates/team-report.html.j2 \
    --template-var project="My Project" \
    --template-var date="Q4 2025" \
    --output team-report.html
```

### 3. security-audit.j2
**Purpose:** Security-focused analysis highlighting risk patterns  
**Use case:** Security reviews, compliance audits  
**Output format:** Markdown

```bash
gstats commits --template templates/security-audit.j2 \
    --template-var auditor="Security Team" \
    --output security-audit.md
```

### 4. data-export.json.j2
**Purpose:** Structured JSON export for programmatic consumption  
**Use case:** API integration, data pipeline feeds  
**Output format:** JSON

```bash
gstats commits --template templates/data-export.json.j2 \
    --template-var project="MyApp" \
    --template-var version="v1.2.3" \
    --output data.json
```

## Template Variables

All templates have access to the following data:

### Repository Information
- `repository.name` - Repository name
- `repository.path` - Full path to repository  
- `repository.scan_timestamp` - ISO 8601 timestamp

### Statistics
- `statistics.total_commits` - Total number of commits
- `statistics.total_files` - Number of files analyzed
- `statistics.total_metrics` - Metrics data points

### Authors Data
- `authors.total_authors` - Number of unique contributors
- `authors.list` - Array of author objects:
  - `name` - Author name
  - `commits` - Number of commits
  - `percentage` - Percentage of total commits

### Files Data
- `files.total_files` - Total files analyzed
- `files.top_by_commits` - Files sorted by commit frequency
- `files.top_by_changes` - Files sorted by lines changed
  - Each file has: `path`, `commits`, `lines_added`, `lines_removed`, `net_change`

### Commits Data
- `commits.total_commits` - Total commits
- `commits.list` - Array of commit objects:
  - `hash` - Commit hash
  - `author` - Commit author
  - `message` - Commit message
  - `timestamp` - Commit timestamp
  - `files_changed` - Number of files in commit

### Custom Variables
Any variables passed via `--template-var key=value` are available as `{{ key }}`

## Available Filters

### number_format
Formats numbers with thousands separators:
```jinja2
{{ 1234567 | number_format }} → "1,234,567"
```

### percentage
Formats numbers as percentages:
```jinja2
{{ 0.234 | percentage }} → "23.4%"
{{ 0.234 | percentage(precision=0) }} → "23%"
```

### Built-in Tera Filters
- `slice(start=0, end=10)` - Array slicing
- `length` - Get length of arrays/strings  
- `first` - Get first element
- `last` - Get last element
- `round(precision=2)` - Round numbers
- `date(format="%Y-%m-%d")` - Format dates
- `tojson` - Convert to JSON

## Template Syntax Reference

### Variables
```jinja2
{{ variable_name }}
{{ object.property }}
{{ array[0] }}
```

### Loops
```jinja2
{% for item in array %}
    {{ loop.index }}. {{ item.name }}
{% endfor %}
```

### Conditionals
```jinja2
{% if condition %}
    Content when true
{% elif other_condition %}
    Alternative content
{% else %}
    Default content
{% endif %}
```

### Filters
```jinja2
{{ value | filter_name }}
{{ value | filter_name(param=value) }}
```

### Comments
```jinja2
{# This is a comment #}
```

## Creating Custom Templates

1. Use any of the example templates as a starting point
2. Access any data from the template variables listed above
3. Use Tera syntax for logic and formatting
4. Save with `.j2` extension (or any extension you prefer)
5. Test with: `gstats commits --template your-template.j2 --output result.html`

## Template Help

For interactive help with available template variables:

```bash
gstats export --template-help
```

This shows current template syntax documentation and all available data variables.

## Advanced Features

### Array Filtering
```jinja2
{# Show only authors with >10% contribution #}
{% for author in authors.list | selectattr("percentage", "gt", 10.0) %}
    {{ author.name }}: {{ author.percentage }}%
{% endfor %}
```

### Conditional Styling
```jinja2
<td style="color: {% if file.net_change > 0 %}green{% else %}red{% endif %};">
    {{ file.net_change }}
</td>
```

### Complex Data Access
```jinja2
{# Top contributor's commit count #}
{{ (authors.list | first).commits }}

{# Files with >5 commits #}
{% set active_files = files.top_by_commits | selectattr("commits", "gt", 5) | list %}
Found {{ active_files | length }} active files
```

## Tips

1. Use `--template-var` to make templates configurable
2. Test templates with different repositories to ensure robustness
3. Include fallback values for optional variables: `{{ project | default("Unknown Project") }}`
4. Use the `slice` filter to limit output length for large datasets
5. Validate JSON templates with a JSON validator after generation