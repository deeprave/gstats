# YouTrack Query Language - Comprehensive Guide

YouTrack's query language is a powerful, natural-language-like search syntax that allows you to find, filter, and organize issues efficiently. This guide provides comprehensive documentation of the query language based on the latest YouTrack documentation.

## Table of Contents

- [Introduction](#introduction)
- [Basic Syntax](#basic-syntax)
- [Attribute-Based Search](#attribute-based-search)
- [Text Search](#text-search)
- [Keywords](#keywords)
- [Operators and Logic](#operators-and-logic)
- [Symbols and Special Characters](#symbols-and-special-characters)
- [Date and Time Queries](#date-and-time-queries)
- [Issue Links](#issue-links)
- [Custom Fields](#custom-fields)
- [Time Tracking](#time-tracking)
- [Sorting](#sorting)
- [Advanced Features](#advanced-features)
- [Common Examples](#common-examples)
- [Query Grammar Reference](#query-grammar-reference)
- [Best Practices](#best-practices)

## Introduction

YouTrack's query language is designed to be intuitive and similar to natural language. Instead of complex filter forms, you type what you're looking for in a manner similar to how you would express it in plain English. The language supports auto-completion to help you build queries faster and more accurately.

### Key Features

- **Natural language syntax** - Queries read like English sentences
- **Auto-completion** - Press Ctrl+Space or Alt+Down for suggestions
- **Case-insensitive** - Grammar is not case-sensitive
- **Flexible syntax** - Multiple ways to express the same query
- **Rich attribute support** - Search across all issue attributes and custom fields

## Basic Syntax

### Attribute-Value Pairs

The fundamental building block of YouTrack queries is the attribute-value pair:

```
attribute: value
```

Examples:
```
project: YouTrack
priority: Critical
assignee: john.doe
state: {In Progress}
```

### Multiple Values

Specify multiple values for an attribute using commas (OR logic):

```
priority: Critical, Major
state: Open, {In Progress}
```

### Exclusion

Exclude values using the minus operator:

```
priority: -Minor
state: -{Won't fix}
```

### Single Values (Shortcuts)

Reference values directly without specifying attributes:

```
#Critical        # Same as priority: Critical
#Unresolved      # Issues with unresolved state
#me              # Issues related to current user
```

## Attribute-Based Search

### Core Issue Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `project` | Project name or ID | `project: DEMO` |
| `issue ID` | Specific issue ID | `issue ID: ABC-123` |
| `state` | Issue state | `state: Open` |
| `priority` | Priority level | `priority: Critical` |
| `assignee` | Assigned user | `assignee: john.doe` |
| `reporter` | Issue creator | `reporter: jane.smith` |
| `type` | Issue type | `type: Bug` |
| `created` | Creation date | `created: Today` |
| `updated` | Last update date | `updated: {This week}` |
| `resolved date` | Resolution date | `resolved date: Yesterday` |

### Aliases

Many attributes have convenient aliases:

```
project: DEMO        = in: DEMO
assignee: john.doe   = for: john.doe = assigned to: john.doe
reporter: jane.smith = by: jane.smith = created by: jane.smith
```

### Text Attributes

For attributes that store text (summary, description, comments):

```
summary: performance issue
description: {memory leak}
comments: "bug fix"
```

## Text Search

### Standard Text Search

Any input not parsed as attribute-based search becomes text search:

```
memory leak                    # Search all text fields
project: DEMO memory leak      # Combine with attributes
```

### Phrase Search

Use quotation marks for exact phrase matching:

```
"performance improvement"      # Exact phrase
description: "memory leak"     # Phrase in specific field
```

### Exact Match Search

Use single quotes for exact character matching (case-sensitive):

```
'NPE'                         # Exact match, case-sensitive
summary: 'IOException'        # Exact match in summary
```

### Wildcards in Text Search

| Wildcard | Description | Example |
|----------|-------------|---------|
| `*` | Multiple characters | `test*` matches "test", "testing", "tester" |
| `?` | Single character | `t?st` matches "test", "tost" |

## Keywords

Keywords are special values that don't require attributes:

### User Keywords

- `#me` / `#my` - References current user
- Can be used with any user attribute

### State Keywords

- `#Resolved` - Issues with resolved state
- `#Unresolved` - Issues with unresolved state

### Version Keywords

- `#Released` - Released versions (must be used with version attributes)
- `#Archived` - Archived versions

### Usage Examples

```
#me #Unresolved              # Unresolved issues related to me
for: #me state: #Unresolved  # Same as above
fixed in: #Released          # Issues fixed in released versions
```

## Operators and Logic

### Default Logic

- **Multiple attributes**: Joined with AND
- **Multiple values for same attribute**: Joined with OR
- **Multiple text terms**: Joined with AND

### Explicit Operators

#### AND Operator

```
state: Open and priority: Critical
tag: urgent and tag: {customer reported}
```

#### OR Operator

```
assignee: john.doe or assignee: jane.smith
state: Open or state: {In Progress}
```

#### Parentheses

Group operations and change precedence:

```
(state: Open or state: {In Progress}) and priority: Critical
(in: Project1 and for: me) or (in: Project2 and type: Bug)
```

**Important**: When using parentheses, you must provide explicit operators to join the parenthetical statement with neighboring arguments.

## Symbols and Special Characters

| Symbol | Purpose | Example |
|--------|---------|---------|
| `-` | Exclusion | `priority: -Minor` |
| `#` | Single value indicator | `#Critical` |
| `,` | Value separator (OR) | `state: Open, {In Progress}` |
| `..` | Range operator | `created: 2024-01-01 .. 2024-12-31` |
| `*` | Wildcard/unbounded | `created: 2024-01-01 .. *` |
| `?` | Single character wildcard | `summary: t?st` |
| `{ }` | Enclose values with spaces | `state: {In Progress}` |
| `" "` | Phrase search | `"exact phrase"` |
| `' '` | Exact match search | `'CaseSensitive'` |

## Date and Time Queries

### Date Formats

- `YYYY-MM-DD` (e.g., `2024-03-15`)
- `YYYY-MM` (e.g., `2024-03`)
- `MM-DD` (e.g., `03-15`)
- `YYYY-MM-DDTHH:MM:SS` (e.g., `2024-03-15T14:30:00`)

### Predefined Relative Dates

| Parameter | Description |
|-----------|-------------|
| `Now` | Current instant |
| `Today` | Current day |
| `Yesterday` | Previous day |
| `Tomorrow` | Next day |
| `{This week}` | Monday 00:00 to Sunday 23:59 current week |
| `{Last week}` | Previous week |
| `{Next week}` | Next week |
| `{This month}` | Current month |
| `{Last month}` | Previous month |
| `{Last working day}` | Most recent working day |

### Custom Date Parameters

Create custom relative dates:

```
created: {minus 7d} .. Today              # Last 7 days
updated: {minus 2h} .. *                  # Last 2 hours
Due Date: {plus 5d}                       # Due in 5 days
created: * .. {minus 1y 6M}               # Older than 1.5 years
```

#### Time Units

- `y` - years
- `M` - months  
- `w` - weeks
- `d` - days
- `h` - hours

Example: `{minus 2y 3M 1w 2d 12h}` = 2 years, 3 months, 1 week, 2 days, 12 hours ago

### Date Range Examples

```
created: 2024-01-01 .. 2024-12-31         # Specific year
updated: {This week}                      # This week
resolved date: Yesterday .. Today         # Yesterday and today
commented: {minus 7d} .. *                # Comments in last 7 days
```

## Issue Links

Search based on issue relationships:

### Generic Links

```
links: ABC-123                            # Issues linked to ABC-123
has: links                                # Issues with any links
has: -links                               # Issues without links
```

### Specific Link Types

| Link Type | Outward | Inward | Example |
|-----------|---------|---------|---------|
| Dependency | `Depends on` | `Is required for` | `Depends on: ABC-123` |
| Hierarchy | `Subtask of` | `Parent for` | `Subtask of: ABC-123` |
| Duplication | `Duplicates` | `Is duplicated by` | `Duplicates: ABC-123` |
| Relation | `Relates to` | `Relates to` | `Relates to: ABC-123` |

### Link Queries

#### With Specific Issues

```
Subtask of: PROJ-123                      # Direct subtasks
Parent for: PROJ-456                      # Direct parent issues
Depends on: PROJ-789                      # Dependencies
```

#### With Sub-queries

```
Subtask of: (state: Unresolved)           # Subtasks of unresolved issues
Depends on: (assignee: john.doe)          # Depends on John's issues
Parent for: (priority: Critical)         # Parent of critical issues
```

#### Aggregation Links

```
aggregate Subtask of: PROJ-123            # All levels of subtasks
```

## Custom Fields

Search custom fields using the field name as attribute:

### Basic Custom Field Search

```
{Custom Field Name}: value
{Story Points}: 5
{Fix Version}: 2.1.0
Priority: Critical                        # Default field
```

### Empty Values

```
Assignee: Unassigned                      # Predefined empty value
{Custom Field}: {No Custom Field}        # Generic empty value
has: -{Custom Field}                      # Field has no value
```

### Multi-value Fields

```
{Affected Components}: Frontend, Backend  # Any of these values
has: {Affected Components}                # Field has any value
```

## Time Tracking

Search issues based on work items and time tracking:

### Work Item Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `work` | Text in work items | `work: testing` |
| `work author` | Work item author | `work author: john.doe` |
| `work type` | Work item type | `work type: Development` |
| `work date` | Work item date | `work date: Today` |

### Custom Work Item Attributes

```
{work item custom field}: value
Expenses: Non-billable
```

### Time Tracking Examples

```
work author: me work date: {This week}    # My work this week
work type: Development                    # Development work items
work: code review                         # Work items mentioning code review
has: work                                 # Issues with any work items
```

## Sorting

Control the order of search results:

### Sort Syntax

```
sort by: <attribute> <direction>
order by: <attribute> <direction>
```

### Sort Attributes

- Basic: `updated`, `created`, `{issue id}`, `summary`
- Custom: Any custom field name
- Special: `star`, `votes`, `comments`, `{attachment size}`

### Sort Directions

- `asc` - Ascending order
- `desc` - Descending order

### Sort Examples

```
priority: Critical sort by: updated desc  # Latest critical issues first
#Unresolved sort by: created asc          # Oldest unresolved first
type: Bug sort by: votes desc             # Most voted bugs first
in: PROJ sort by: {Story Points} asc      # Sort by story points
```

## Advanced Features

### The `has` Keyword

Check for presence or absence of values:

```
has: assignee                             # Issues with any assignee
has: -assignee                            # Unassigned issues
has: attachments                          # Issues with attachments
has: comments                             # Issues with comments
has: {Custom Field}                       # Custom field has value
has: duplicates                           # Issues with duplicates
has: star                                 # Starred issues
has: votes                                # Issues with votes
```

### Visibility and Permissions

```
visible to: {Team Name}                   # Visible to specific team
visible to: me                            # Visible to current user
visible to: {All Users}                   # Public issues
```

### Advanced Text Search

#### Code Search

```
code: function                            # Code formatted text
code: {hello world}                       # Multiple words in code
```

#### Attachment Search

```
attachments: screenshot                   # Attachment filename
{attachment text}: error                  # Text in image attachments
```

### Range Searches

```
priority: Critical .. Normal              # Priority range
created: 2024-01-01 .. 2024-03-31        # Date range
{Story Points}: 3 .. 8                   # Numeric range
votes: 5 .. *                            # 5 or more votes
```

## Common Examples

### Finding Your Work

```
#me #Unresolved                           # My unresolved issues
for: me state: {In Progress}              # My in-progress work
assignee: me updated: Today               # My issues updated today
```

### Project Management

```
in: PROJ #Unresolved sort by: priority desc  # Project priorities
state: Open created: {This week}              # New issues this week
type: Bug priority: Critical, Major           # Critical/major bugs
```

### Team Queries

```
assignee: {Development Team} #Unresolved      # Team's open work
updated by: {QA Team} updated: {Last week}    # QA activity last week
reporter: {Customer Support} type: Bug        # Customer-reported bugs
```

### Sprint Planning

```
Board: {Sprint 23}                            # Issues in specific sprint
has: {Story Points} state: {Ready for Dev}   # Estimated ready issues
fix for: 2.1.0 state: -{Won't fix}          # Issues for release 2.1.0
```

### Historical Analysis

```
resolved date: {Last month}                   # Resolved last month
created: {minus 3M} .. {minus 1M}            # Created 1-3 months ago
work date: 2024-Q1 work author: me           # My Q1 work
```

## Query Grammar Reference

YouTrack uses a formal BNF (Backus-Naur Form) grammar for its query language. The grammar is case-insensitive and follows these key patterns:

### Basic Structure

```
<Query> ::= <OrExpression>
<OrExpression> ::= <AndExpression> ("or" <AndExpression>)*
<AndExpression> ::= <SignExpression> ("and" <SignExpression>)*
<SignExpression> ::= ("-")? <Item>
<Item> ::= <Field> | <SingleValue> | <Parentheses>
```

### Field Syntax

```
<Field> ::= <Attribute> ":" <Value>
<Value> ::= <SingleValue> | <ValueList> | <ValueRange>
<ValueList> ::= <SingleValue> ("," <SingleValue>)*
<ValueRange> ::= <SingleValue> ".." <SingleValue>
```

### Special Constructs

```
<SingleValue> ::= ("#" | "-")? <ValueToken>
<Parentheses> ::= "(" <OrExpression> ")"
<SortClause> ::= ("sort by" | "order by") <SortAttribute> <SortOrder>
```

## Best Practices

### Query Construction

1. **Use auto-completion** - Press Ctrl+Space for suggestions
2. **Start simple** - Begin with basic attributes and add complexity
3. **Use aliases** - Shorter aliases like `in:` instead of `project:`
4. **Enclose spaces** - Use braces `{}` for multi-word values
5. **Test incrementally** - Build complex queries step by step

### Performance Tips

1. **Specify projects** - Limit scope with `in: PROJECT`
2. **Use date ranges** - Avoid open-ended date queries when possible
3. **Limit text search** - Be specific with text search terms
4. **Sort efficiently** - Only sort when necessary

### Common Patterns

```
# Daily standup queries
assignee: me state: {In Progress}, {Code Review}

# Release planning
fix for: 2.1.0 state: -{Won't fix}, -Duplicate

# Bug triage
type: Bug state: Submitted sort by: priority desc, created asc

# Sprint retrospective
Board: {Sprint 23} resolved date: {Last week}
```

### Saved Searches

Create saved searches for frequently used queries:

1. Complex project filters
2. Personal work views
3. Team dashboards
4. Release tracking
5. Regular reports

## References

This guide is based on the official YouTrack documentation:

- [YouTrack Cloud Search Query Reference](https://www.jetbrains.com/help/youtrack/cloud/search-and-command-attributes.html)
- [YouTrack Server Search Query Reference](https://www.jetbrains.com/help/youtrack/server/search-and-command-attributes.html)
- [Attribute-based Search Documentation](https://www.jetbrains.com/help/youtrack/cloud/attribute-based-search.html)
- [Text Search Documentation](https://www.jetbrains.com/help/youtrack/server/full-text-search.html)
- [Sample Search Queries](https://www.jetbrains.com/help/youtrack/server/sample-search-queries.html)

For the most up-to-date information, always refer to the official JetBrains YouTrack documentation at [jetbrains.com/help/youtrack](https://www.jetbrains.com/help/youtrack/).
