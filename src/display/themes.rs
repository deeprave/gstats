//! Predefined colour themes for different terminal environments
//! 
//! Provides a collection of carefully designed colour themes optimised
//! for different terminal backgrounds and accessibility requirements.

use super::config::{ColourPalette, ColourTheme};

/// Collection of predefined colour themes
pub struct ThemeCollection;

impl ThemeCollection {
    /// Get all available theme names
    pub fn available_themes() -> Vec<&'static str> {
        vec!["auto", "light", "dark", "high-contrast", "minimal"]
    }
    
    /// Get a theme by name
    pub fn get_theme(name: &str) -> Option<ColourTheme> {
        match name.to_lowercase().as_str() {
            "auto" => Some(ColourTheme::Auto),
            "light" => Some(ColourTheme::Light),
            "dark" => Some(ColourTheme::Dark),
            "high-contrast" => Some(ColourTheme::Custom(Self::high_contrast_palette())),
            "minimal" => Some(ColourTheme::Custom(Self::minimal_palette())),
            _ => None,
        }
    }
    
    /// High contrast theme for accessibility
    pub fn high_contrast_palette() -> ColourPalette {
        ColourPalette {
            error: "bright_red".to_string(),
            warning: "bright_yellow".to_string(),
            info: "bright_white".to_string(),
            debug: "bright_black".to_string(),
            success: "bright_green".to_string(),
            highlight: "bright_magenta".to_string(),
        }
    }
    
    /// Minimal theme with fewer colours
    pub fn minimal_palette() -> ColourPalette {
        ColourPalette {
            error: "red".to_string(),
            warning: "yellow".to_string(),
            info: "white".to_string(), // Same as normal text
            debug: "bright_black".to_string(),
            success: "green".to_string(),
            highlight: "white".to_string(), // No special highlight
        }
    }
    
    /// Professional theme suitable for corporate environments
    pub fn professional_palette() -> ColourPalette {
        ColourPalette {
            error: "red".to_string(),
            warning: "yellow".to_string(),
            info: "blue".to_string(),
            debug: "bright_black".to_string(),
            success: "green".to_string(),
            highlight: "cyan".to_string(),
        }
    }
    
    /// Vibrant theme with bright colours
    pub fn vibrant_palette() -> ColourPalette {
        ColourPalette {
            error: "bright_red".to_string(),
            warning: "bright_yellow".to_string(),
            info: "bright_blue".to_string(),
            debug: "bright_black".to_string(),
            success: "bright_green".to_string(),
            highlight: "bright_cyan".to_string(),
        }
    }
}

/// Terminal background detection (placeholder for future implementation)
pub struct BackgroundDetector;

impl BackgroundDetector {
    /// Detect if the terminal has a light or dark background
    /// 
    /// Currently returns a default assumption. In future phases, this could:
    /// - Query terminal for background colour (where supported)
    /// - Use heuristics based on terminal type/environment
    /// - Allow user configuration override
    pub fn detect_background() -> BackgroundType {
        // For now, assume dark background as it's more common in developer terminals
        // TODO: Implement actual background detection in future phases
        BackgroundType::Dark
    }
    
    /// Get the recommended theme for the detected background
    pub fn recommended_theme() -> ColourTheme {
        match Self::detect_background() {
            BackgroundType::Light => ColourTheme::Light,
            BackgroundType::Dark => ColourTheme::Dark,
            BackgroundType::Unknown => ColourTheme::Dark, // Default to dark
        }
    }
}

/// Terminal background types
#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundType {
    Light,
    Dark,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_available_themes() {
        let themes = ThemeCollection::available_themes();
        assert!(themes.contains(&"auto"));
        assert!(themes.contains(&"light"));
        assert!(themes.contains(&"dark"));
        assert!(themes.contains(&"high-contrast"));
        assert!(themes.contains(&"minimal"));
    }
    
    #[test]
    fn test_get_theme_by_name() {
        assert_eq!(ThemeCollection::get_theme("auto"), Some(ColourTheme::Auto));
        assert_eq!(ThemeCollection::get_theme("light"), Some(ColourTheme::Light));
        assert_eq!(ThemeCollection::get_theme("dark"), Some(ColourTheme::Dark));
        
        // Case insensitive
        assert_eq!(ThemeCollection::get_theme("AUTO"), Some(ColourTheme::Auto));
        assert_eq!(ThemeCollection::get_theme("Light"), Some(ColourTheme::Light));
        
        // Invalid theme
        assert_eq!(ThemeCollection::get_theme("invalid"), None);
    }
    
    #[test]
    fn test_custom_themes() {
        let high_contrast = ThemeCollection::get_theme("high-contrast");
        assert!(high_contrast.is_some());
        
        if let Some(ColourTheme::Custom(palette)) = high_contrast {
            assert_eq!(palette.error, "bright_red");
            assert_eq!(palette.info, "bright_white");
        }
        
        let minimal = ThemeCollection::get_theme("minimal");
        assert!(minimal.is_some());
        
        if let Some(ColourTheme::Custom(palette)) = minimal {
            assert_eq!(palette.info, "white");
            assert_eq!(palette.highlight, "white");
        }
    }
    
    #[test]
    fn test_predefined_palettes() {
        let high_contrast = ThemeCollection::high_contrast_palette();
        assert_eq!(high_contrast.error, "bright_red");
        assert_eq!(high_contrast.info, "bright_white");
        
        let minimal = ThemeCollection::minimal_palette();
        assert_eq!(minimal.info, "white");
        assert_eq!(minimal.highlight, "white");
        
        let professional = ThemeCollection::professional_palette();
        assert_eq!(professional.error, "red");
        assert_eq!(professional.highlight, "cyan");
        
        let vibrant = ThemeCollection::vibrant_palette();
        assert_eq!(vibrant.error, "bright_red");
        assert_eq!(vibrant.success, "bright_green");
    }
    
    #[test]
    fn test_background_detection() {
        // Test that background detection returns a valid type
        let background = BackgroundDetector::detect_background();
        assert!(matches!(background, BackgroundType::Light | BackgroundType::Dark | BackgroundType::Unknown));
        
        // Test recommended theme
        let theme = BackgroundDetector::recommended_theme();
        assert!(matches!(theme, ColourTheme::Light | ColourTheme::Dark));
    }
}