# YouTrack CLI (`yt`) Documentation

## Overview
The `yt` command is a powerful CLI tool for managing JetBrains YouTrack issues, projects, users, time tracking, and more.

## Authentication
- `yt auth login` - Interactive login (recommended for first-time setup)
- `yt auth logout` - Clear authentication credentials
- `yt auth token` - Manage API tokens

## Common Command Shortcuts
- `yt i` = `yt issues`
- `yt p` = `yt projects`
- `yt t` = `yt time`
- `yt u` = `yt users`
- `yt a` = `yt articles`
- `yt b` = `yt boards`
- `yt ls` = `yt issues list` (shortcut)
- `yt new` = `yt issues create` (shortcut)

## Output Formats
- **Default**: `table` format (clean tabular display)
- **Alternative**: `json` format for automation
- **Note**: Only `--format table` and `--format json` are supported

## Issues Management

### Listing Issues
```bash
# List all your assigned issues
yt issues list --assignee me

# List issues with specific filters
yt issues list --project-id WEB --state Open
yt issues list --assignee me --page-size 10

# List with advanced query
yt issues list -q "priority:Critical state:Open"

# Options:
# -p, --project-id TEXT    Filter by project ID
# -s, --state TEXT         Filter by issue state
# -a, --assignee TEXT      Filter by assignee
# -f, --fields TEXT        Comma-separated list of fields
# --profile [minimal|standard|full]  Field selection profile
# --page-size INTEGER      Number of issues per page (default: 100)
# -q, --query TEXT         Advanced query filter
# --format [table|json]    Output format
```

### Viewing Issue Details
```bash
# Show detailed information about a specific issue
yt issues show CMS-33
```

### Searching Issues
```bash
# Advanced search with query
yt issues search "priority:Critical state:Open"

# Search within a project
yt issues search "project:CMS state:Open" --page-size 20
```

### Moving Issues (State Transitions)
```bash
# Change issue state
yt issues move ISSUE-ID --state "In Progress"
yt issues move ISSUE-ID --state Done

# Move issue to different project
yt issues move ISSUE-ID --project-id NEW-PROJECT
```

## Projects Management
```bash
# List all projects
yt projects list

# View project details (if available)
yt projects show PROJECT-ID
```

## Agile Boards
```bash
# List all agile boards
yt boards list
```

## Users Management
```bash
# List users
yt users list

# List only active users
yt users list --active-only

# Search users
yt users list -q "searchterm"
```

## Time Tracking
```bash
# List time entries
yt time list

# Filter time entries
yt time list --issue ISSUE-ID
yt time list --user-id USERNAME
yt time list --start-date 2025-07-01 --end-date 2025-07-25
```

## Query Language
When using `-q` or `--query` parameters, YouTrack uses its own query syntax. Common patterns:
- `assignee:me` - Issues assigned to current user
- `state:Open` - Issues in Open state
- `priority:Critical` - Critical priority issues
- `project:CMS` - Issues in CMS project
- Combined: `project:CMS state:Open priority:Critical`

For complex queries, refer to the comprehensive query language guide at `~/.claude/youtrack_query_language_guide.md`

## Common Workflows

### Daily Issue Review
```bash
# Check your open issues
yt issues list --assignee me --state Open

# Check in-progress work
yt issues list --assignee me --state "In Progress"
```

### Issue State Management
```bash
# View issue details
yt issues show ISSUE-ID

# Start working on an issue
yt issues move ISSUE-ID --state "In Progress"

# Complete an issue
yt issues move ISSUE-ID --state Done
```

### Project Overview
```bash
# List all projects
yt projects list

# View issues in a specific project
yt issues list --project-id PROJECT-ID
```

## Reports

### Burndown Reports
```bash
# Generate burndown report for a project
yt burndown DEMO

# Generate report for specific sprint
yt burndown WEB-PROJECT --sprint "Sprint 1"

# Generate report for date range
yt burndown API --start-date 2024-01-01 --end-date 2024-01-31
```

### Velocity Reports
```bash
# Generate velocity report for last 5 sprints (default)
yt velocity PROJECT-123

# Generate velocity report for last 10 sprints
yt velocity PROJECT-123 --sprints 10
```

## Configuration
```bash
# List all configuration values
yt config list

# Get a specific configuration value
yt config get SETTING_NAME

# Set a configuration value
yt config set SETTING_NAME VALUE
```

## Tips
- Use `--help` with any command for detailed options
- Use `--format json` for scripting and automation
- Default page size is 100 items; use `--page-size` to adjust
- The `--all` flag fetches all results using pagination
- Issue IDs shown in listings (e.g., DSE-21, CMS-32) can be used with `yt issues show`
- Query syntax is very powerful - see `YouTrack_Query_Language.md` for comprehensive documentation

## Limitations **IMPORTANT**
- Note that vscode's terminal/pty system does not deal well with long multiline strings in commands.
  Attempting to do so will crash/freeze the PTY and vscode will need to restart the terminal.
- As a result, use SHORT, CONCISE descriptions and comments when interacting with the yt.
  If a large amount of data needs to be added to an isse, create and attach it as a file.
