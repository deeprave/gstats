//! Scanning Modes
//! 
//! Bitflag-based scanning modes for combinable scan configurations.

use bitflags::bitflags;
use serde::{Serialize, Deserialize};

bitflags! {
    /// Scanning modes that can be combined using bitwise operations
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct ScanMode: u32 {
        /// No scanning mode selected
        const NONE = 0x0;
        /// Scan file system structure and content
        const FILES = 0x01;
        /// Scan git history and commits
        const HISTORY = 0x02;
        /// Scan for code metrics and statistics
        const METRICS = 0x04;
        /// Scan for dependencies and imports
        const DEPENDENCIES = 0x08;
        /// Scan for security vulnerabilities
        const SECURITY = 0x10;
        /// Scan for performance bottlenecks
        const PERFORMANCE = 0x20;
    }
}

/// Mode information for discovery API
#[derive(Debug, Clone, PartialEq)]
pub struct ModeInfo {
    pub name: String,
    pub description: String,
    pub flag_value: u32,
}

/// Get supported scanning modes with metadata
/// 
/// # Returns
/// * `Vec<ModeInfo>` - Information about all supported modes
pub fn get_supported_modes() -> Vec<ModeInfo> {
    vec![
        ModeInfo {
            name: "files".to_string(),
            description: "Scan file system structure and content".to_string(),
            flag_value: ScanMode::FILES.bits(),
        },
        ModeInfo {
            name: "history".to_string(),
            description: "Scan git history and commit information".to_string(),
            flag_value: ScanMode::HISTORY.bits(),
        },
        ModeInfo {
            name: "metrics".to_string(),
            description: "Scan code metrics and complexity".to_string(),
            flag_value: ScanMode::METRICS.bits(),
        },
        ModeInfo {
            name: "dependencies".to_string(),
            description: "Scan project dependencies".to_string(),
            flag_value: ScanMode::DEPENDENCIES.bits(),
        },
        ModeInfo {
            name: "security".to_string(),
            description: "Scan for security vulnerabilities".to_string(),
            flag_value: ScanMode::SECURITY.bits(),
        },
        ModeInfo {
            name: "performance".to_string(),
            description: "Scan performance characteristics".to_string(),
            flag_value: ScanMode::PERFORMANCE.bits(),
        },
    ]
}

/// Get all supported scanning modes as bitflags
/// 
/// # Returns
/// * `ScanMode` - Bitflags containing all supported modes
pub fn get_all_modes() -> ScanMode {
    ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS | 
    ScanMode::DEPENDENCIES | ScanMode::SECURITY | ScanMode::PERFORMANCE
}

/// Check if a scanning mode is valid (not empty)
/// 
/// # Arguments
/// * `mode` - The scanning mode to validate
/// 
/// # Returns
/// * `bool` - True if mode is valid (not empty)
pub fn is_valid_mode(mode: ScanMode) -> bool {
    !mode.is_empty()
}

/// Get human-readable description of a scanning mode
/// 
/// # Arguments
/// * `mode` - The scanning mode to describe
/// 
/// # Returns
/// * `String` - Description of the scanning mode
pub fn get_mode_description(mode: ScanMode) -> String {
    match mode {
        ScanMode::FILES => "Scan file system structure and content".to_string(),
        ScanMode::HISTORY => "Scan git history and commit information".to_string(),
        ScanMode::METRICS => "Scan for code metrics and statistics".to_string(),
        ScanMode::DEPENDENCIES => "Scan for dependencies and imports".to_string(),
        ScanMode::SECURITY => "Scan for security vulnerabilities".to_string(),
        ScanMode::PERFORMANCE => "Scan for performance bottlenecks".to_string(),
        ScanMode::NONE => "No scanning mode selected".to_string(),
        _ => {
            // Handle combined modes
            let mut descriptions = Vec::new();
            if mode.contains(ScanMode::FILES) {
                descriptions.push("Files");
            }
            if mode.contains(ScanMode::HISTORY) {
                descriptions.push("History");
            }
            if mode.contains(ScanMode::METRICS) {
                descriptions.push("Metrics");
            }
            if mode.contains(ScanMode::DEPENDENCIES) {
                descriptions.push("Dependencies");
            }
            if mode.contains(ScanMode::SECURITY) {
                descriptions.push("Security");
            }
            if mode.contains(ScanMode::PERFORMANCE) {
                descriptions.push("Performance");
            }
            format!("Combined scanning: {}", descriptions.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitflag_operations() {
        let files = ScanMode::FILES;
        let history = ScanMode::HISTORY;
        let combined = files | history;

        assert!(combined.contains(files));
        assert!(combined.contains(history));
        assert!(combined.intersects(files));
        assert!(combined.intersects(history));
    }

    #[test]
    fn test_mode_validation() {
        assert!(is_valid_mode(ScanMode::FILES));
        assert!(!is_valid_mode(ScanMode::NONE));
        assert!(!is_valid_mode(ScanMode::empty()));
    }

    #[test]
    fn test_supported_modes() {
        let supported = get_supported_modes();
        assert!(!supported.is_empty());
        
        // Check that all modes are present
        let mode_names: Vec<&str> = supported.iter().map(|m| m.name.as_str()).collect();
        assert!(mode_names.contains(&"files"));
        assert!(mode_names.contains(&"history"));
        assert!(mode_names.contains(&"metrics"));
        
        // Test get_all_modes as well
        let all_modes = get_all_modes();
        assert!(all_modes.contains(ScanMode::FILES));
        assert!(all_modes.contains(ScanMode::HISTORY));
        assert!(!all_modes.is_empty());
    }

    #[test]
    fn test_mode_descriptions() {
        let desc = get_mode_description(ScanMode::FILES);
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("file"));

        let combined = ScanMode::FILES | ScanMode::HISTORY;
        let combined_desc = get_mode_description(combined);
        assert!(combined_desc.contains("Combined"));
    }
}
