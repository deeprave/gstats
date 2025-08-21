//! Plugin Settings and Configuration
//! 
//! Provides configuration settings that are passed to plugins during
//! discovery and instantiation, replacing the fragile environment 
//! variable approach.

use clap::ColorChoice;

/// Configuration settings passed to plugins
#[derive(Debug, Clone)]
pub struct PluginSettings {
    /// Color choice for plugin output (using clap's ColorChoice)
    pub color_choice: ColorChoice,
    /// Whether plugins should show verbose output
    pub verbose: bool,
    /// Whether plugins should show debug information
    pub debug: bool,
}

impl PluginSettings {
    /// Create new plugin settings from initial args
    pub fn from_initial_args(initial_args: &crate::cli::initial_args::InitialArgs) -> Self {
        let color_choice = if initial_args.no_color {
            ColorChoice::Never
        } else if initial_args.color {
            ColorChoice::Always
        } else {
            ColorChoice::Auto
        };
        
        Self {
            color_choice,
            verbose: false, // Will be set later from full args parsing
            debug: false,   // Will be set later from full args parsing
        }
    }
    
    /// Update settings with full CLI args after parsing
    pub fn update_with_args(&mut self, args: &crate::cli::Args) {
        self.verbose = args.verbose;
        self.debug = args.debug;
    }
    
    /// Get color choice as boolean flags for backward compatibility
    pub fn get_color_flags(&self) -> (bool, bool) {
        match self.color_choice {
            ColorChoice::Always => (true, false),  // (color, no_color)
            ColorChoice::Never => (false, true),
            ColorChoice::Auto => (false, false),
        }
    }
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            color_choice: ColorChoice::Auto,
            verbose: false,
            debug: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ColorChoice;
    
    #[test]
    fn test_color_choice_from_flags() {
        let settings1 = PluginSettings::from_initial_args(&crate::cli::initial_args::InitialArgs {
            color: false,
            no_color: false,
            ..Default::default()
        });
        assert!(matches!(settings1.color_choice, ColorChoice::Auto));
        
        let settings2 = PluginSettings::from_initial_args(&crate::cli::initial_args::InitialArgs {
            color: true,
            no_color: false,
            ..Default::default()
        });
        assert!(matches!(settings2.color_choice, ColorChoice::Always));
        
        let settings3 = PluginSettings::from_initial_args(&crate::cli::initial_args::InitialArgs {
            color: false,
            no_color: true,
            ..Default::default()
        });
        assert!(matches!(settings3.color_choice, ColorChoice::Never));
        
        // no_color takes precedence
        let settings4 = PluginSettings::from_initial_args(&crate::cli::initial_args::InitialArgs {
            color: true,
            no_color: true,
            ..Default::default()
        });
        assert!(matches!(settings4.color_choice, ColorChoice::Never));
    }
    
    #[test]
    fn test_get_color_flags() {
        let settings_auto = PluginSettings { color_choice: ColorChoice::Auto, ..Default::default() };
        assert_eq!(settings_auto.get_color_flags(), (false, false));
        
        let settings_always = PluginSettings { color_choice: ColorChoice::Always, ..Default::default() };
        assert_eq!(settings_always.get_color_flags(), (true, false));
        
        let settings_never = PluginSettings { color_choice: ColorChoice::Never, ..Default::default() };
        assert_eq!(settings_never.get_color_flags(), (false, true));
    }
}