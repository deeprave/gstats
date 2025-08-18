//! Colour configuration and theme management
//! 
//! Provides configuration structures and theme system for customising
//! colour output based on user preferences and terminal capabilities.

#![allow(dead_code)]

use colored::Color;
use serde::{Deserialize, Serialize};

/// Colour configuration for the display system
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ColourConfig {
    /// Whether colours are enabled globally
    pub enabled: bool,
    /// The colour theme to use
    pub theme: ColourTheme,
    /// Whether to respect NO_COLOR environment variable
    pub respect_no_color: bool,
    /// Force colours even when not in a TTY (--color flag)
    #[serde(skip, default)]
    pub color_forced: bool,
}

impl Default for ColourConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            theme: ColourTheme::Auto,
            respect_no_color: true,
            color_forced: false,
        }
    }
}

/// Available colour themes
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum ColourTheme {
    /// Automatically detect light/dark terminal background
    Auto,
    /// Optimised for light backgrounds
    Light,
    /// Optimised for dark backgrounds
    Dark,
    /// Custom colour palette
    Custom(ColourPalette),
}

/// Custom colour palette definition
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ColourPalette {
    /// Colour for error messages
    pub error: String,
    /// Colour for warning messages
    pub warning: String,
    /// Colour for info messages
    pub info: String,
    /// Colour for debug messages
    pub debug: String,
    /// Colour for success messages
    pub success: String,
    /// Colour for highlighted text
    pub highlight: String,
}

impl Default for ColourPalette {
    fn default() -> Self {
        Self {
            error: "red".to_string(),
            warning: "yellow".to_string(),
            info: "blue".to_string(),
            debug: "bright_black".to_string(),
            success: "green".to_string(),
            highlight: "cyan".to_string(),
        }
    }
}

impl ColourPalette {
    /// Get the light theme palette (optimised for light backgrounds)
    pub fn light() -> Self {
        Self {
            error: "red".to_string(),
            warning: "yellow".to_string(),
            info: "blue".to_string(),
            debug: "black".to_string(), // Darker for light backgrounds
            success: "green".to_string(),
            highlight: "magenta".to_string(), // Better contrast on light
        }
    }
    
    /// Get the dark theme palette (optimised for dark backgrounds)
    pub fn dark() -> Self {
        Self {
            error: "bright_red".to_string(),
            warning: "bright_yellow".to_string(),
            info: "bright_blue".to_string(),
            debug: "bright_black".to_string(), // Lighter for dark backgrounds
            success: "bright_green".to_string(),
            highlight: "bright_cyan".to_string(),
        }
    }
    
    /// Parse a colour string into a Color enum
    pub fn parse_color(color_str: &str) -> Option<Color> {
        match color_str.to_lowercase().as_str() {
            "black" => Some(Color::Black),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "blue" => Some(Color::Blue),
            "magenta" => Some(Color::Magenta),
            "cyan" => Some(Color::Cyan),
            "white" => Some(Color::White),
            "bright_black" => Some(Color::BrightBlack),
            "bright_red" => Some(Color::BrightRed),
            "bright_green" => Some(Color::BrightGreen),
            "bright_yellow" => Some(Color::BrightYellow),
            "bright_blue" => Some(Color::BrightBlue),
            "bright_magenta" => Some(Color::BrightMagenta),
            "bright_cyan" => Some(Color::BrightCyan),
            "bright_white" => Some(Color::BrightWhite),
            _ => None,
        }
    }
}

impl ColourTheme {
    /// Get the appropriate colour palette for this theme
    pub fn get_palette(&self) -> ColourPalette {
        match self {
            ColourTheme::Auto => {
                // For now, default to dark theme for auto
                // TODO: Add terminal background detection in future
                ColourPalette::dark()
            }
            ColourTheme::Light => ColourPalette::light(),
            ColourTheme::Dark => ColourPalette::dark(),
            ColourTheme::Custom(palette) => palette.clone(),
        }
    }
}

impl ColourConfig {
    /// Create a new colour configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a colour configuration with colours disabled
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            theme: ColourTheme::Auto,
            respect_no_color: true,
            color_forced: false,
        }
    }
    
    /// Create a colour configuration with a specific theme
    pub fn with_theme(theme: ColourTheme) -> Self {
        Self {
            enabled: true,
            theme,
            respect_no_color: true,
            color_forced: false,
        }
    }
    
    /// Set whether colours are enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Set the colour theme
    pub fn set_theme(&mut self, theme: ColourTheme) {
        self.theme = theme;
    }
    
    /// Set whether to respect NO_COLOR environment variable
    pub fn set_respect_no_color(&mut self, respect: bool) {
        self.respect_no_color = respect;
    }
    
    /// Force colours to be enabled (--color flag)
    pub fn set_color_forced(&mut self, forced: bool) {
        self.color_forced = forced;
    }
    
    /// Check if colours should be enabled based on configuration and environment
    pub fn should_use_colours(&self) -> bool {
        if !self.enabled {
            return false;
        }
        
        // If color_forced is true (--color flag), ignore TTY and NO_COLOR
        if self.color_forced {
            return true;
        }
        
        // Check NO_COLOR environment variable
        if self.respect_no_color && std::env::var("NO_COLOR").is_ok() {
            return false;
        }
        
        // If respect_no_color is false, user wants colors regardless of environment
        if !self.respect_no_color {
            return true;
        }
        
        // Check if we're in a TTY (default behavior)
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
    }
    
    /// Get the colour palette for the current theme
    pub fn get_palette(&self) -> ColourPalette {
        self.theme.get_palette()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    
    #[test]
    fn test_default_colour_config() {
        let config = ColourConfig::default();
        assert!(config.enabled);
        assert_eq!(config.theme, ColourTheme::Auto);
        assert!(config.respect_no_color);
    }
    
    #[test]
    fn test_disabled_colour_config() {
        let config = ColourConfig::disabled();
        assert!(!config.enabled);
        assert!(!config.should_use_colours());
    }
    
    #[test]
    fn test_colour_config_with_theme() {
        let config = ColourConfig::with_theme(ColourTheme::Dark);
        assert!(config.enabled);
        assert_eq!(config.theme, ColourTheme::Dark);
    }
    
    #[test]
    fn test_should_use_colours_with_no_color() {
        env::set_var("NO_COLOR", "1");
        
        let config = ColourConfig::default();
        assert!(!config.should_use_colours());
        
        let mut config_no_respect = ColourConfig::default();
        config_no_respect.set_respect_no_color(false);
        assert!(config_no_respect.should_use_colours());
        
        env::remove_var("NO_COLOR");
    }
    
    #[test]
    fn test_colour_palettes() {
        let light_palette = ColourPalette::light();
        let dark_palette = ColourPalette::dark();
        let default_palette = ColourPalette::default();
        
        // Light theme should have darker debug colour
        assert_eq!(light_palette.debug, "black");
        assert_eq!(dark_palette.debug, "bright_black");
        
        // Default should match standard colours
        assert_eq!(default_palette.error, "red");
        assert_eq!(default_palette.success, "green");
    }
    
    #[test]
    fn test_colour_theme_palettes() {
        let auto_theme = ColourTheme::Auto;
        let light_theme = ColourTheme::Light;
        let dark_theme = ColourTheme::Dark;
        let custom_theme = ColourTheme::Custom(ColourPalette::default());
        
        // Auto should return dark palette for now
        assert_eq!(auto_theme.get_palette().debug, "bright_black");
        
        // Light and Dark should return appropriate palettes
        assert_eq!(light_theme.get_palette().debug, "black");
        assert_eq!(dark_theme.get_palette().debug, "bright_black");
        
        // Custom should return the custom palette
        assert_eq!(custom_theme.get_palette().error, "red");
    }
    
    #[test]
    fn test_parse_color() {
        assert_eq!(ColourPalette::parse_color("red"), Some(Color::Red));
        assert_eq!(ColourPalette::parse_color("bright_blue"), Some(Color::BrightBlue));
        assert_eq!(ColourPalette::parse_color("invalid"), None);
        
        // Test case insensitivity
        assert_eq!(ColourPalette::parse_color("RED"), Some(Color::Red));
        assert_eq!(ColourPalette::parse_color("Bright_Green"), Some(Color::BrightGreen));
    }
    
    #[test]
    fn test_colour_config_modification() {
        let mut config = ColourConfig::new();
        
        config.set_enabled(false);
        assert!(!config.enabled);
        
        config.set_theme(ColourTheme::Light);
        assert_eq!(config.theme, ColourTheme::Light);
        
        config.set_respect_no_color(false);
        assert!(!config.respect_no_color);
    }
}