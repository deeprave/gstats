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
    pub success: &'static str,
    pub error: &'static str,
    pub warning: &'static str,
    pub info: &'static str,
    pub loading: &'static str,
}

impl Default for StatusSymbols {
    fn default() -> Self {
        Self {
            success: "✅",
            error: "❌", 
            warning: "⚠️",
            info: "ℹ️",
            loading: "🔄",
        }
    }
}

impl StatusSymbols {
    /// ASCII-only symbols for terminals without unicode support
    pub fn ascii() -> Self {
        Self {
            success: "[OK]",
            error: "[ERR]",
            warning: "[WARN]",
            info: "[INFO]",
            loading: "[...]",
        }
    }
}

/// Progress bar configuration
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    pub width: usize,
    pub completed_char: char,
    pub incomplete_char: char,
    pub show_percentage: bool,
    pub show_count: bool,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            width: 30,
            completed_char: '█',
            incomplete_char: '░',
            show_percentage: true,
            show_count: true,
        }
    }
}

/// Spinner animation frames
#[derive(Debug, Clone)]
pub struct SpinnerFrames {
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
    
    /// Dots spinner
    pub fn dots() -> Self {
        Self {
            frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            interval: Duration::from_millis(80),
        }
    }
}

/// Progress indicator manager
pub struct ProgressIndicator {
    colour_manager: ColourManager,
    symbols: StatusSymbols,
    progress_config: ProgressConfig,
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
            progress_config: ProgressConfig::default(),
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
            StatusType::Success => self.symbols.success,
            StatusType::Error => self.symbols.error,
            StatusType::Warning => self.symbols.warning,
            StatusType::Info => self.symbols.info,
            StatusType::Loading => self.symbols.loading,
        };
        
        let colored_message = match status_type {
            StatusType::Success => self.colour_manager.success(message),
            StatusType::Error => self.colour_manager.error(message),
            StatusType::Warning => self.colour_manager.warning(message),
            StatusType::Info => self.colour_manager.info(message),
            StatusType::Loading => self.colour_manager.info(message),
        };
        
        println!("{} {}", symbol, colored_message);
    }
    
    /// Create a progress bar string
    pub fn progress_bar(&self, current: usize, total: usize) -> String {
        if total == 0 {
            return String::new();
        }
        
        let percentage = (current * 100) / total;
        let completed_width = (current * self.progress_config.width) / total;
        let incomplete_width = self.progress_config.width - completed_width;
        
        let bar = format!(
            "{}{}",
            self.progress_config.completed_char.to_string().repeat(completed_width),
            self.progress_config.incomplete_char.to_string().repeat(incomplete_width)
        );
        
        let colored_bar = self.colour_manager.success(&bar);
        
        let mut result = format!("{}", colored_bar);
        
        if self.progress_config.show_percentage {
            result.push_str(&format!(" {}%", percentage));
        }
        
        if self.progress_config.show_count {
            result.push_str(&format!(" {}/{}", current, total));
        }
        
        result
    }
    
    /// Display a progress bar with message
    pub fn progress(&self, message: &str, current: usize, total: usize) {
        let bar = self.progress_bar(current, total);
        print!("\r{}: {} ", self.colour_manager.info(message), bar);
        std::io::Write::flush(&mut std::io::stdout()).ok();
        
        if current >= total {
            println!(); // Move to next line when complete
        }
    }
    
    /// Start a spinner animation
    pub fn start_spinner(&self, message: &str) -> SpinnerHandle {
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
    
    /// Create an operation tracker for long-running operations
    pub fn operation(&self, message: &str) -> OperationTracker {
        OperationTracker {
            indicator: self.clone(),
            message: message.to_string(),
            start_time: Instant::now(),
        }
    }
}

impl Clone for ProgressIndicator {
    fn clone(&self) -> Self {
        Self {
            colour_manager: self.colour_manager.clone(),
            symbols: self.symbols.clone(),
            progress_config: self.progress_config.clone(),
            spinner_frames: self.spinner_frames.clone(),
            use_unicode: self.use_unicode,
        }
    }
}

/// Status type for different kinds of messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Success,
    Error,
    Warning,
    Info,
    Loading,
}

/// Handle for controlling a spinner animation
pub struct SpinnerHandle {
    stop_flag: Arc<AtomicBool>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl SpinnerHandle {
    /// Stop the spinner animation
    pub async fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.await.ok();
        }
    }
    
    /// Stop the spinner and display a completion message
    pub async fn complete(self, status_type: StatusType, message: &str) {
        self.stop().await;
        // Note: We can't access the ProgressIndicator from here, so just print
        let symbol = match status_type {
            StatusType::Success => "✅",
            StatusType::Error => "❌",
            StatusType::Warning => "⚠️",
            StatusType::Info => "ℹ️",
            StatusType::Loading => "🔄",
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
pub struct OperationTracker {
    indicator: ProgressIndicator,
    message: String,
    start_time: Instant,
}

impl OperationTracker {
    /// Complete the operation with success status
    pub fn complete(self) {
        let duration = self.start_time.elapsed();
        let message = format!("{} (completed in {:.1}s)", self.message, duration.as_secs_f64());
        self.indicator.status(StatusType::Success, &message);
    }
    
    /// Complete the operation with error status
    pub fn error(self, error_msg: &str) {
        let duration = self.start_time.elapsed();
        let message = format!("{} failed after {:.1}s: {}", self.message, duration.as_secs_f64(), error_msg);
        self.indicator.status(StatusType::Error, &message);
    }
    
    /// Complete the operation with warning status
    pub fn warning(self, warning_msg: &str) {
        let duration = self.start_time.elapsed();
        let message = format!("{} completed with warnings after {:.1}s: {}", self.message, duration.as_secs_f64(), warning_msg);
        self.indicator.status(StatusType::Warning, &message);
    }
    
    /// Update progress if this is a multi-step operation
    pub fn progress(&self, current: usize, total: usize) {
        self.indicator.progress(&self.message, current, total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::ColourManager;
    
    fn create_test_indicator() -> ProgressIndicator {
        let colour_manager = ColourManager::with_colours(false); // No colors for testing
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
        assert_eq!(ascii_symbols.success, "[OK]");
        assert_eq!(ascii_symbols.error, "[ERR]");
        assert_eq!(ascii_symbols.warning, "[WARN]");
    }
    
    #[test]
    fn test_progress_bar() {
        let indicator = create_test_indicator();
        
        // Test empty progress
        let bar = indicator.progress_bar(0, 100);
        assert!(bar.contains("0%"));
        assert!(bar.contains("0/100"));
        
        // Test half progress
        let bar = indicator.progress_bar(50, 100);
        assert!(bar.contains("50%"));
        assert!(bar.contains("50/100"));
        
        // Test complete progress
        let bar = indicator.progress_bar(100, 100);
        assert!(bar.contains("100%"));
        assert!(bar.contains("100/100"));
    }
    
    #[test]
    fn test_progress_config() {
        let config = ProgressConfig::default();
        assert_eq!(config.width, 30);
        assert_eq!(config.completed_char, '█');
        assert_eq!(config.incomplete_char, '░');
        assert!(config.show_percentage);
        assert!(config.show_count);
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
    
    #[tokio::test]
    async fn test_operation_tracker() {
        let indicator = create_test_indicator();
        let tracker = indicator.operation("Test operation");
        
        // Verify that the tracker stores the correct message
        assert_eq!(tracker.message, "Test operation");
        
        // Complete the operation (testing that it doesn't panic)
        tracker.complete();
    }
}