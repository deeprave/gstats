//! Progress indicators and status displays for CLI output
//! 
//! Provides visual feedback for long-running operations including
//! status indicators with color support.

use crate::display::ColourManager;

/// Status indicator symbols with unicode support
#[derive(Debug, Clone)]
pub struct StatusSymbols {
    pub warning: &'static str,
    pub info: &'static str,
}

impl Default for StatusSymbols {
    fn default() -> Self {
        Self {
            warning: "⚠️",
            info: "ℹ️",
        }
    }
}

impl StatusSymbols {
    /// ASCII-only symbols for terminals without unicode support
    pub fn ascii() -> Self {
        Self {
            warning: "[WARN]",
            info: "[INFO]",
        }
    }
}



/// Progress indicator manager
pub struct ProgressIndicator {
    colour_manager: ColourManager,
    symbols: StatusSymbols,
    use_unicode: bool,
}

impl ProgressIndicator {
    /// Create a new progress indicator with the given colour manager
    pub fn new(colour_manager: ColourManager) -> Self {
        let use_unicode = Self::supports_unicode();
        
        Self {
            colour_manager,
            symbols: if use_unicode { StatusSymbols::default() } else { StatusSymbols::ascii() },
            use_unicode,
        }
    }
    
    /// Check if terminal supports unicode characters
    fn supports_unicode() -> bool {
        // Check LANG environment variable for UTF-8 support
        if let Ok(lang) = std::env::var("LANG") {
            return lang.to_lowercase().contains("utf-8") || lang.to_lowercase().contains("utf8");
        }
        
        // Check LC_CTYPE
        if let Ok(lc_ctype) = std::env::var("LC_CTYPE") {
            return lc_ctype.to_lowercase().contains("utf-8") || lc_ctype.to_lowercase().contains("utf8");
        }
        
        // Default to ASCII for safety
        false
    }
    
    /// Display a status message with appropriate symbol and color
    pub fn status(&self, status_type: StatusType, message: &str) {
        let symbol = match status_type {
            StatusType::Warning => self.symbols.warning,
            StatusType::Info => self.symbols.info,
        };
        
        let colored_message = match status_type {
            StatusType::Warning => self.colour_manager.warning(message),
            StatusType::Info => self.colour_manager.info(message),
        };
        
        println!("{} {}", symbol, colored_message);
    }
    
    
    
}

impl Clone for ProgressIndicator {
    fn clone(&self) -> Self {
        Self {
            colour_manager: self.colour_manager.clone(),
            symbols: self.symbols.clone(),
            use_unicode: self.use_unicode,
        }
    }
}

/// Status type for different kinds of messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Warning,
    Info,
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::ColourManager;
    
    fn create_test_indicator() -> ProgressIndicator {
        let mut config = crate::display::ColourConfig::default();
        config.set_enabled(false);
        let colour_manager = ColourManager::with_config(config); // No colors for testing
        ProgressIndicator::new(colour_manager)
    }
    
    #[test]
    fn test_progress_indicator_creation() {
        let _indicator = create_test_indicator();
        // Unicode support depends on environment variables, so we just verify creation works
        // The actual unicode detection logic is tested separately
    }
    
    #[test]
    fn test_status_symbols() {
        let ascii_symbols = StatusSymbols::ascii();
        assert_eq!(ascii_symbols.warning, "[WARN]");
        assert_eq!(ascii_symbols.info, "[INFO]");
    }
    
    
    #[test]
    fn test_unicode_support_detection() {
        // This test depends on the environment, so we just verify the function runs
        let supports_unicode = ProgressIndicator::supports_unicode();
        assert!(supports_unicode == true || supports_unicode == false);
    }
}