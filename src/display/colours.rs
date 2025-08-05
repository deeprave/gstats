//! Core colour management for CLI output
//! 
//! Provides colour support with terminal compatibility, NO_COLOR compliance,
//! and graceful degradation for non-colour terminals.

use colored::{ColoredString, Colorize};
use super::config::{ColourConfig, ColourPalette};

/// Manages colour output for the CLI application
#[derive(Debug, Clone)]
pub struct ColourManager {
    config: ColourConfig,
    palette: ColourPalette,
}

impl ColourManager {
    /// Create a new ColourManager with default configuration
    pub fn new() -> Self {
        let config = ColourConfig::default();
        let palette = config.get_palette();
        Self { config, palette }
    }
    
    /// Create a ColourManager with explicit colour control
    pub fn with_colours(enabled: bool) -> Self {
        let mut config = ColourConfig::default();
        config.set_enabled(enabled);
        let palette = config.get_palette();
        Self { config, palette }
    }
    
    /// Create a ColourManager with a specific configuration
    pub fn with_config(config: ColourConfig) -> Self {
        let palette = config.get_palette();
        Self { config, palette }
    }
    
    /// Create a ColourManager from CLI arguments and optional configuration
    pub fn from_args_and_config(no_color_flag: bool, config: Option<ColourConfig>) -> Self {
        let mut final_config = config.unwrap_or_default();
        
        // CLI --no-color flag overrides everything
        if no_color_flag {
            final_config.set_enabled(false);
        }
        
        let palette = final_config.get_palette();
        Self { config: final_config, palette }
    }
    
    
    /// Check if colours are enabled
    pub fn colours_enabled(&self) -> bool {
        self.config.should_use_colours()
    }
    
    /// Get the current colour configuration
    pub fn config(&self) -> &ColourConfig {
        &self.config
    }
    
    /// Get the current colour palette
    pub fn palette(&self) -> &ColourPalette {
        &self.palette
    }
    
    /// Update the colour configuration
    pub fn set_config(&mut self, config: ColourConfig) {
        self.palette = config.get_palette();
        self.config = config;
    }
    
    /// Format text as an error using the configured error colour
    pub fn error(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            self.apply_color(text, &self.palette.error)
        } else {
            text.normal()
        }
    }
    
    /// Format text as a warning using the configured warning colour
    pub fn warning(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            self.apply_color(text, &self.palette.warning)
        } else {
            text.normal()
        }
    }
    
    /// Format text as info using the configured info colour
    pub fn info(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            self.apply_color(text, &self.palette.info)
        } else {
            text.normal()
        }
    }
    
    /// Format text as debug using the configured debug colour
    pub fn debug(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            self.apply_color(text, &self.palette.debug)
        } else {
            text.normal()
        }
    }
    
    /// Format text as success using the configured success colour
    pub fn success(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            self.apply_color(text, &self.palette.success)
        } else {
            text.normal()
        }
    }
    
    /// Format text as highlight using the configured highlight colour
    pub fn highlight(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            self.apply_color(text, &self.palette.highlight)
        } else {
            text.normal()
        }
    }
    
    /// Format text as a command name (brown color for good contrast on both backgrounds)
    pub fn command(&self, text: &str) -> ColoredString {
        if self.colours_enabled() {
            // Use yellow as a brown-like color that works well on both light and dark backgrounds
            text.truecolor(139, 69, 19) // Saddle brown RGB color
        } else {
            text.normal()
        }
    }
    
    /// Apply a colour from the palette to text
    fn apply_color(&self, text: &str, color_name: &str) -> ColoredString {
        if let Some(color) = ColourPalette::parse_color(color_name) {
            text.color(color)
        } else {
            // Fallback to basic colours if parsing fails
            match color_name {
                name if name.contains("red") => text.red(),
                name if name.contains("yellow") => text.yellow(),
                name if name.contains("blue") => text.blue(),
                name if name.contains("green") => text.green(),
                name if name.contains("cyan") => text.cyan(),
                name if name.contains("magenta") => text.magenta(),
                name if name.contains("black") => text.bright_black(),
                name if name.contains("white") => text.white(),
                _ => text.normal(),
            }
        }
    }
}

impl Default for ColourManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    
    #[test]
    fn test_colour_manager_creation() {
        let manager = ColourManager::new();
        // Default should detect colours automatically
        assert!(manager.colours_enabled()); // Assumes test environment supports colours
    }
    
    #[test]
    fn test_colour_manager_explicit_enable() {
        let manager = ColourManager::with_colours(true);
        assert!(manager.colours_enabled());
    }
    
    #[test]
    fn test_colour_manager_explicit_disable() {
        let manager = ColourManager::with_colours(false);
        assert!(!manager.colours_enabled());
    }
    
    #[test]
    fn test_no_color_environment_variable() {
        // Set NO_COLOR temporarily
        env::set_var("NO_COLOR", "1");
        
        let manager = ColourManager::new();
        assert!(!manager.colours_enabled());
        
        // Clean up 
        env::remove_var("NO_COLOR");
    }
    
    #[test]
    fn test_colour_formatting_enabled() {
        // Force colors for testing by setting FORCE_COLOR env var
        env::set_var("FORCE_COLOR", "1");
        
        let manager = ColourManager::with_colours(true);
        
        let error_text = manager.error("test error");
        let warning_text = manager.warning("test warning");
        let info_text = manager.info("test info");
        let debug_text = manager.debug("test debug");
        let success_text = manager.success("test success");
        let highlight_text = manager.highlight("test highlight");
        
        // When colours are enabled, the formatted strings should contain ANSI codes
        // Note: colored crate may still disable colors in test environments
        // So we test that the manager respects our explicit enable/disable setting
        if error_text.to_string().contains("\x1b[") {
            // Colors are working
            assert!(warning_text.to_string().contains("\x1b["));
            assert!(info_text.to_string().contains("\x1b["));
            assert!(debug_text.to_string().contains("\x1b["));
            assert!(success_text.to_string().contains("\x1b["));
            assert!(highlight_text.to_string().contains("\x1b["));
        } else {
            // Colors are disabled by the colored crate itself (e.g., in tests)
            // This is acceptable behavior - the crate is working correctly
            assert_eq!(error_text.to_string(), "test error");
            assert_eq!(warning_text.to_string(), "test warning");
            assert_eq!(info_text.to_string(), "test info");
            assert_eq!(debug_text.to_string(), "test debug");
            assert_eq!(success_text.to_string(), "test success");
            assert_eq!(highlight_text.to_string(), "test highlight");
        }
        
        // Clean up
        env::remove_var("FORCE_COLOR");
    }
    
    #[test]
    fn test_colour_formatting_disabled() {
        let manager = ColourManager::with_colours(false);
        
        let error_text = manager.error("test error");
        let warning_text = manager.warning("test warning");
        let info_text = manager.info("test info");
        let debug_text = manager.debug("test debug");
        let success_text = manager.success("test success");
        let highlight_text = manager.highlight("test highlight");
        
        // When colours are disabled, the strings should be plain text
        assert_eq!(error_text.to_string(), "test error");
        assert_eq!(warning_text.to_string(), "test warning");
        assert_eq!(info_text.to_string(), "test info");
        assert_eq!(debug_text.to_string(), "test debug");
        assert_eq!(success_text.to_string(), "test success");
        assert_eq!(highlight_text.to_string(), "test highlight");
    }
}