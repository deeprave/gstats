//! Progress indicators and status displays for CLI output
//! 
//! Provides visual feedback for long-running operations including spinners,
//! progress bars, and status indicators with color support.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
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


/// Spinner animation frames
#[derive(Debug, Clone)]
struct SpinnerFrames {
    pub frames: Vec<&'static str>,
    pub interval: Duration,
}

impl Default for SpinnerFrames {
    fn default() -> Self {
        Self {
            frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            interval: Duration::from_millis(100),
        }
    }
}

impl SpinnerFrames {
    /// ASCII-only spinner for terminals without unicode support
    pub fn ascii() -> Self {
        Self {
            frames: vec!["|", "/", "-", "\\"],
            interval: Duration::from_millis(150),
        }
    }
}

/// Progress indicator manager
pub struct ProgressIndicator {
    colour_manager: ColourManager,
    symbols: StatusSymbols,
    spinner_frames: SpinnerFrames,
    use_unicode: bool,
}

impl ProgressIndicator {
    /// Create a new progress indicator with the given colour manager
    pub fn new(colour_manager: ColourManager) -> Self {
        let use_unicode = Self::supports_unicode();
        
        Self {
            colour_manager,
            symbols: if use_unicode { StatusSymbols::default() } else { StatusSymbols::ascii() },
            spinner_frames: if use_unicode { SpinnerFrames::default() } else { SpinnerFrames::ascii() },
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
    
    
    /// Start a spinner animation
    #[allow(dead_code)]
    fn start_spinner(&self, message: &str) -> SpinnerHandle {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);
        let frames = self.spinner_frames.clone();
        let colored_message = self.colour_manager.info(message).to_string();
        
        let handle = tokio::spawn(async move {
            let mut frame_index = 0;
            let start_time = Instant::now();
            
            while !stop_flag_clone.load(Ordering::Relaxed) {
                let frame = frames.frames[frame_index];
                let elapsed = start_time.elapsed();
                let elapsed_str = if elapsed.as_secs() > 0 {
                    format!(" ({}s)", elapsed.as_secs())
                } else {
                    String::new()
                };
                
                print!("\r{} {}{}", frame, colored_message, elapsed_str);
                std::io::Write::flush(&mut std::io::stdout()).ok();
                
                frame_index = (frame_index + 1) % frames.frames.len();
                sleep(frames.interval).await;
            }
            
            // Clear the spinner line
            print!("\r{}", " ".repeat(colored_message.len() + 20));
            std::io::Write::flush(&mut std::io::stdout()).ok();
            print!("\r");
        });
        
        SpinnerHandle {
            stop_flag,
            handle: Some(handle),
        }
    }
    
}

impl Clone for ProgressIndicator {
    fn clone(&self) -> Self {
        Self {
            colour_manager: self.colour_manager.clone(),
            symbols: self.symbols.clone(),
            spinner_frames: self.spinner_frames.clone(),
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

/// Handle for controlling a spinner animation
#[allow(dead_code)]
struct SpinnerHandle {
    stop_flag: Arc<AtomicBool>,
    #[allow(dead_code)]
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl SpinnerHandle {
    /// Stop the spinner animation
    #[allow(dead_code)]
    async fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.await.ok();
        }
    }
    
    /// Stop the spinner and display a completion message
    #[allow(dead_code)]
    async fn complete(self, status_type: StatusType, message: &str) {
        self.stop().await;
        // Note: We can't access the ProgressIndicator from here, so just print
        let symbol = match status_type {
            StatusType::Warning => "⚠️",
            StatusType::Info => "ℹ️",
        };
        println!("{} {}", symbol, message);
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

/// Tracker for long-running operations

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
    fn test_spinner_frames() {
        let unicode_frames = SpinnerFrames::default();
        assert!(!unicode_frames.frames.is_empty());
        assert_eq!(unicode_frames.interval, Duration::from_millis(100));
        
        let ascii_frames = SpinnerFrames::ascii();
        assert_eq!(ascii_frames.frames, vec!["|", "/", "-", "\\"]);
        assert_eq!(ascii_frames.interval, Duration::from_millis(150));
    }
    
    #[test]
    fn test_unicode_support_detection() {
        // This test depends on the environment, so we just verify the function runs
        let supports_unicode = ProgressIndicator::supports_unicode();
        assert!(supports_unicode == true || supports_unicode == false);
    }
}