//! Contextual Help System for Progressive Help Discovery
//! 
//! Provides intelligent, context-aware help based on user actions and intent.

use std::collections::HashMap;

/// Help level for progressive disclosure
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HelpLevel {
    Basic,
    Intermediate,
    Advanced,
}

/// Context for help generation
#[derive(Debug, Clone)]
pub struct HelpContext {
    /// Command or plugin being requested
    pub command: Option<String>,
    /// Error that occurred (if any)
    pub error_context: Option<String>,
    /// User's apparent experience level
    pub user_level: HelpLevel,
    /// Recent commands used (for context)
    pub recent_commands: Vec<String>,
}

impl Default for HelpContext {
    fn default() -> Self {
        Self {
            command: None,
            error_context: None,
            user_level: HelpLevel::Basic,
            recent_commands: Vec::new(),
        }
    }
}

/// Help section with different levels of detail
#[derive(Debug, Clone)]
pub struct HelpSection {
    pub title: String,
    pub basic_description: String,
    pub intermediate_description: Option<String>,
    pub advanced_description: Option<String>,
    pub examples: Vec<HelpExample>,
    pub related_commands: Vec<String>,
    pub common_patterns: Vec<String>,
}

/// Help example with context
#[derive(Debug, Clone)]
pub struct HelpExample {
    pub description: String,
    pub command: String,
    pub use_case: String,
    pub level: HelpLevel,
}

/// Workflow guide for common tasks
#[derive(Debug, Clone)]
pub struct WorkflowGuide {
    pub name: String,
    pub description: String,
}


/// Contextual help system
pub struct ContextualHelp {
    help_sections: HashMap<String, HelpSection>,
    workflow_guides: HashMap<String, WorkflowGuide>,
}

impl ContextualHelp {
    /// Create a new contextual help system with built-in content
    pub fn new() -> Self {
        let mut help = Self {
            help_sections: HashMap::new(),
            workflow_guides: HashMap::new(),
        };
        
        help.initialize_builtin_help();
        help
    }
    
    /// Get contextual help based on context
    pub fn get_contextual_help(&self, context: &HelpContext) -> String {
        let mut help_text = String::new();
        
        if let Some(command) = &context.command {
            // Command-specific help
            if let Some(section) = self.help_sections.get(command) {
                help_text.push_str(&self.format_help_section(section, context.user_level));
                help_text.push_str("\n\n");
                
                // Add related commands
                if !section.related_commands.is_empty() {
                    help_text.push_str("RELATED COMMANDS:\n");
                    for related in &section.related_commands {
                        help_text.push_str(&format!("  gstats {} --help\n", related));
                    }
                    help_text.push_str("\n");
                }
            }
        }
        
        // Add workflow suggestions based on context
        let workflows = self.suggest_workflows(context);
        if !workflows.is_empty() {
            help_text.push_str("SUGGESTED WORKFLOWS:\n");
            for workflow in workflows {
                help_text.push_str(&format!("  {}: {}\n", workflow.name, workflow.description));
            }
            help_text.push_str("\n");
        }
        
        // Add troubleshooting if there's an error context
        if let Some(error) = &context.error_context {
            help_text.push_str(&self.get_troubleshooting_help(error, context));
        }
        
        help_text
    }
    
    
    /// Suggest workflows based on context
    pub fn suggest_workflows(&self, context: &HelpContext) -> Vec<&WorkflowGuide> {
        let mut suggestions = Vec::new();
        
        // Basic workflow suggestions
        if context.recent_commands.is_empty() {
            suggestions.push(self.workflow_guides.get("quick_start").unwrap());
        }
        
        // Context-based suggestions
        if let Some(command) = &context.command {
            match command.as_str() {
                "commits" => {
                    if let Some(guide) = self.workflow_guides.get("team_analysis") {
                        suggestions.push(guide);
                    }
                }
                "metrics" => {
                    if let Some(guide) = self.workflow_guides.get("code_quality") {
                        suggestions.push(guide);
                    }
                }
                "export" | _ => {
                    // For export or other commands, suggest team analysis as a general workflow
                    if let Some(guide) = self.workflow_guides.get("team_analysis") {
                        suggestions.push(guide);
                    }
                }
            }
        }
        
        suggestions
    }
    
    /// Get troubleshooting help for errors
    fn get_troubleshooting_help(&self, error: &str, context: &HelpContext) -> String {
        let mut troubleshooting = String::new();
        
        troubleshooting.push_str("TROUBLESHOOTING:\n");
        
        // Common error patterns
        if error.contains("git repository") {
            troubleshooting.push_str("• Make sure you're in a git repository directory\n");
            troubleshooting.push_str("• Use --repo to specify a different repository path\n");
            troubleshooting.push_str("• Try: cd /path/to/your/repo && gstats commits\n");
        }
        
        if error.contains("permission") || error.contains("access") {
            troubleshooting.push_str("• Check file permissions in the repository\n");
            troubleshooting.push_str("• Make sure you have read access to the git directory\n");
        }
        
        if error.contains("plugin") {
            troubleshooting.push_str("• Use 'gstats --plugins' to see available plugins\n");
            troubleshooting.push_str("• Check plugin name spelling with suggestions above\n");
            if let Some(command) = &context.command {
                troubleshooting.push_str(&format!("• Try: gstats --plugin-info {}\n", command));
            }
        }
        
        troubleshooting.push_str("\n");
        troubleshooting
    }
    
    /// Format help section based on user level
    fn format_help_section(&self, section: &HelpSection, level: HelpLevel) -> String {
        let mut formatted = String::new();
        
        formatted.push_str(&format!("# {}\n\n", section.title));
        
        // Description based on level
        match level {
            HelpLevel::Basic => {
                formatted.push_str(&section.basic_description);
            }
            HelpLevel::Intermediate => {
                formatted.push_str(&section.basic_description);
                if let Some(intermediate) = &section.intermediate_description {
                    formatted.push_str("\n\n");
                    formatted.push_str(intermediate);
                }
            }
            HelpLevel::Advanced => {
                formatted.push_str(&section.basic_description);
                if let Some(intermediate) = &section.intermediate_description {
                    formatted.push_str("\n\n");
                    formatted.push_str(intermediate);
                }
                if let Some(advanced) = &section.advanced_description {
                    formatted.push_str("\n\n");
                    formatted.push_str(advanced);
                }
            }
        }
        
        // Examples for this level
        let relevant_examples: Vec<_> = section.examples.iter()
            .filter(|ex| match level {
                HelpLevel::Basic => ex.level == HelpLevel::Basic,
                HelpLevel::Intermediate => ex.level != HelpLevel::Advanced,
                HelpLevel::Advanced => true,
            })
            .collect();
        
        if !relevant_examples.is_empty() {
            formatted.push_str("\n\nEXAMPLES:\n");
            for example in relevant_examples {
                formatted.push_str(&format!("  # {}\n", example.description));
                formatted.push_str(&format!("  {}\n", example.command));
                if level != HelpLevel::Basic {
                    formatted.push_str(&format!("  # Use case: {}\n", example.use_case));
                }
                formatted.push_str("\n");
            }
        }
        
        // Common patterns for intermediate/advanced
        if level != HelpLevel::Basic && !section.common_patterns.is_empty() {
            formatted.push_str("COMMON PATTERNS:\n");
            for pattern in &section.common_patterns {
                formatted.push_str(&format!("  {}\n", pattern));
            }
            formatted.push_str("\n");
        }
        
        formatted
    }
    
    /// Initialize built-in help content
    fn initialize_builtin_help(&mut self) {
        self.add_commits_help();
        self.add_metrics_help();
        self.add_export_help();
        self.add_workflow_guides();
    }
    
    /// Add help for commits command
    fn add_commits_help(&mut self) {
        let section = HelpSection {
            title: "Commit Analysis".to_string(),
            basic_description: "Analyze git commit history and contributor statistics.".to_string(),
            intermediate_description: Some("Provides detailed insights into code contributions, author activity, and project evolution over time.".to_string()),
            advanced_description: Some("Supports advanced filtering, statistical analysis, and can be combined with other commands for comprehensive repository analysis.".to_string()),
            examples: vec![
                HelpExample {
                    description: "Basic usage".to_string(),
                    command: "gstats commits".to_string(),
                    use_case: "Get overall repository statistics".to_string(),
                    level: HelpLevel::Basic,
                },
                HelpExample {
                    description: "Filter by time period".to_string(),
                    command: "gstats commits --since \"2 weeks ago\" --until \"1 week ago\"".to_string(),
                    use_case: "Analyze specific time periods for sprint reviews".to_string(),
                    level: HelpLevel::Intermediate,
                },
                HelpExample {
                    description: "Focus on specific author".to_string(),
                    command: "gstats commits --author \"jane@example.com\"".to_string(),
                    use_case: "Individual performance review or contribution tracking".to_string(),
                    level: HelpLevel::Intermediate,
                },
                HelpExample {
                    description: "Complex filtering".to_string(),
                    command: "gstats --limit 50 commits --include-path src/ --exclude-author \"bot@\"".to_string(),
                    use_case: "Detailed analysis excluding automated commits".to_string(),
                    level: HelpLevel::Advanced,
                },
            ],
            related_commands: vec!["metrics".to_string(), "export".to_string()],
            common_patterns: vec![
                "Weekly team report: gstats commits --since \"1 week ago\"".to_string(),
                "Code review prep: gstats commits --author $(git config user.email)".to_string(),
                "Release analysis: gstats commits --since v1.0.0 --until HEAD".to_string(),
                "Exclude bots: gstats commits --exclude-author \"*[bot]\"".to_string(),
            ],
        };
        
        self.help_sections.insert("commits".to_string(), section);
    }
    
    /// Add help for metrics command
    fn add_metrics_help(&mut self) {
        let section = HelpSection {
            title: "Code Metrics Analysis".to_string(),
            basic_description: "Generate code complexity and quality metrics for your repository.".to_string(),
            intermediate_description: Some("Analyzes code complexity, duplication, maintainability indices, and technical debt indicators.".to_string()),
            advanced_description: Some("Provides detailed statistical analysis with configurable thresholds, trend analysis, and integration with quality gates.".to_string()),
            examples: vec![
                HelpExample {
                    description: "Basic metrics".to_string(),
                    command: "gstats metrics".to_string(),
                    use_case: "Get overall code quality overview".to_string(),
                    level: HelpLevel::Basic,
                },
                HelpExample {
                    description: "Focus on specific directories".to_string(),
                    command: "gstats metrics --include-path src/ --include-path lib/".to_string(),
                    use_case: "Analyze only production code, excluding tests".to_string(),
                    level: HelpLevel::Intermediate,
                },
                HelpExample {
                    description: "File type filtering".to_string(),
                    command: "gstats metrics --include-file \"*.rs\" --exclude-file \"*test*\"".to_string(),
                    use_case: "Language-specific analysis excluding test files".to_string(),
                    level: HelpLevel::Advanced,
                },
            ],
            related_commands: vec!["commits".to_string(), "export".to_string()],
            common_patterns: vec![
                "Code review metrics: gstats metrics --include-path src/".to_string(),
                "Pre-release quality check: gstats metrics --include-file \"*.rs\"".to_string(),
                "Technical debt assessment: gstats metrics | gstats export --format json".to_string(),
            ],
        };
        
        self.help_sections.insert("metrics".to_string(), section);
    }
    
    /// Add help for export command
    fn add_export_help(&mut self) {
        let section = HelpSection {
            title: "Export Analysis Results".to_string(),
            basic_description: "Export analysis results in various formats for reporting and integration.".to_string(),
            intermediate_description: Some("Supports JSON, CSV, XML formats for integration with dashboards, reports, and external tools.".to_string()),
            advanced_description: Some("Provides configurable output schemas, data transformation options, and streaming for large datasets.".to_string()),
            examples: vec![
                HelpExample {
                    description: "Export to JSON".to_string(),
                    command: "gstats commits | gstats export --format json > report.json".to_string(),
                    use_case: "Create structured data for dashboards".to_string(),
                    level: HelpLevel::Basic,
                },
                HelpExample {
                    description: "Export to CSV".to_string(),
                    command: "gstats metrics | gstats export --format csv --output metrics.csv".to_string(),
                    use_case: "Import into spreadsheet applications".to_string(),
                    level: HelpLevel::Intermediate,
                },
                HelpExample {
                    description: "Custom export pipeline".to_string(),
                    command: "gstats commits --since \"1 month\" | gstats export --format json --compress".to_string(),
                    use_case: "Automated reporting with compression".to_string(),
                    level: HelpLevel::Advanced,
                },
            ],
            related_commands: vec!["commits".to_string(), "metrics".to_string()],
            common_patterns: vec![
                "Dashboard data: gstats commits | gstats export --format json".to_string(),
                "Excel reports: gstats metrics | gstats export --format csv".to_string(),
                "CI/CD integration: gstats metrics | gstats export --format json | curl -X POST ...".to_string(),
            ],
        };
        
        self.help_sections.insert("export".to_string(), section);
    }
    
    /// Add workflow guides
    fn add_workflow_guides(&mut self) {
        // Quick start workflow
        let quick_start = WorkflowGuide {
            name: "Quick Start".to_string(),
            description: "Get started with basic repository analysis".to_string(),
        };
        
        let team_analysis = WorkflowGuide {
            name: "Team Analysis".to_string(),
            description: "Analyze team contributions and collaboration patterns".to_string(),
        };
        
        let code_quality = WorkflowGuide {
            name: "Code Quality Assessment".to_string(),
            description: "Assess code quality and identify improvement areas".to_string(),
        };
        
        self.workflow_guides.insert("quick_start".to_string(), quick_start);
        self.workflow_guides.insert("team_analysis".to_string(), team_analysis);
        self.workflow_guides.insert("code_quality".to_string(), code_quality);
    }
}

impl Default for ContextualHelp {
    fn default() -> Self {
        Self::new()
    }
}


/// Helper function to create help context from command and error
pub fn create_help_context(command: Option<String>, error: Option<String>) -> HelpContext {
    let mut context = HelpContext::default();
    context.command = command;
    context.error_context = error;
    context.user_level = HelpLevel::Basic; // Default to basic
    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contextual_help_creation() {
        let help = ContextualHelp::new();
        assert!(help.help_sections.contains_key("commits"));
        assert!(help.help_sections.contains_key("metrics"));
        assert!(help.help_sections.contains_key("export"));
    }


    #[test]
    fn test_workflow_suggestions() {
        let help = ContextualHelp::new();
        let context = HelpContext {
            command: Some("commits".to_string()),
            ..HelpContext::default()
        };
        
        let workflows = help.suggest_workflows(&context);
        assert!(!workflows.is_empty());
        // Check if any workflow relates to commits/team analysis
        assert!(workflows.iter().any(|w| w.name.contains("Team") || w.name.contains("Analysis")));
    }

    #[test]
    fn test_troubleshooting_help() {
        let help = ContextualHelp::new();
        let context = HelpContext {
            error_context: Some("git repository not found".to_string()),
            ..HelpContext::default()
        };
        
        let help_text = help.get_contextual_help(&context);
        assert!(help_text.contains("TROUBLESHOOTING"));
        assert!(help_text.contains("repository"));
    }

}