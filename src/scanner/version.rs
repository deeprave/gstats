//! API Version Management for Scanner Module
//! 
//! Provides build-time API version reading from Cargo.toml metadata.
//! The version is defined in Cargo.toml under package.metadata.gstats.api_version
//! and ensures reproducible builds across all developers and environments.

// Include the build-generated API version constant
include!(concat!(env!("OUT_DIR"), "/version_api.rs"));

/// Get the current API version 
/// 
/// This uses a build-generated constant that reads the API version from Cargo.toml.
/// The version is defined in package.metadata.gstats.api_version and provides
/// reproducible builds - same source code always produces same API version.
/// 
/// To increment the API version:
/// 1. Edit Cargo.toml: package.metadata.gstats.api_version = NEW_VERSION
/// 2. Commit the change to source control
/// 3. Build - new version will be used
/// 
/// Version format: YYYYMMDD (e.g., 20250727 = 27 July 2025)
/// 
/// # Returns
/// * `i64` - The current API version from Cargo.toml metadata
pub fn get_api_version() -> i64 {
    BASE_API_VERSION
}

/// Convert YYYYMMDD version to a human-readable date string
/// 
/// # Arguments
/// * `version` - The version in YYYYMMDD format
/// 
/// # Returns
/// * `String` - Date in YYYY-MM-DD format
pub fn days_to_date_string(version: i64) -> String {
    if (10000000..=99999999).contains(&version) {
        // Extract YYYY, MM, DD from YYYYMMDD format
        let year = version / 10000;
        let month = (version % 10000) / 100;
        let day = version % 100;
        
        format!("{year:04}-{month:02}-{day:02}")
    } else {
        // Fallback for legacy days-since-epoch format
        let epoch_year = 1970;
        let days_per_year = 365; // Simplified, ignoring leap years
        
        let years_since_epoch = version / days_per_year;
        let remaining_days = version % days_per_year;
        
        let year = epoch_year + years_since_epoch;
        let month = (remaining_days / 30) + 1;
        let day = (remaining_days % 30) + 1;
        
        format!("{:04}-{:02}-{:02}", year, month.min(12), day.min(31))
    }
}

/// Get version information as a structured format
/// 
/// # Returns
/// * `String` - JSON-formatted version information
pub fn get_version_info() -> String {
    let version = get_api_version();
    
    format!(
        r#"{{"api_version": {}, "release_date": "{}", "compatibility": "manual increment only", "version_format": "YYYYMMDD"}}"#,
        version,
        days_to_date_string(version)
    )
}

/// Check if a version is within the supported compatibility range
/// 
/// # Arguments  
/// * `version` - The version to check (days since epoch)
/// * `current` - The current API version (days since epoch)
/// * `compatibility_days` - Number of days backward compatibility is maintained
/// 
/// # Returns
/// * `bool` - True if version is compatible
pub fn is_version_compatible(version: i64, current: i64, compatibility_days: i64) -> bool {
    version <= current && version >= (current - compatibility_days)
}

/// Check if a required API version is compatible with current version
pub fn is_api_compatible(required_version: i64) -> bool {
    get_api_version() >= required_version
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version_positive() {
        let version = get_api_version();
        assert!(version > 0, "API version should be positive");
        
        // Test that version is reasonable (after 2020)
        let days_2020 = 18262; // Approximate days for 2020-01-01
        assert!(version >= days_2020, "API version should be after 2020");
    }

    #[test]
    fn test_api_version_stability() {
        // Test that API version is stable across multiple calls
        let version1 = get_api_version();
        let version2 = get_api_version();
        assert_eq!(version1, version2, "API version should be stable");
        
        // Test that version is consistent with build-generated constant
        assert_eq!(version1, BASE_API_VERSION, "API version should match build constant");
    }

    #[test]
    fn test_version_compatibility() {
        let current = get_api_version();
        
        // Self-compatibility
        assert!(is_version_compatible(current, current, 30));
        
        // Backward compatibility
        assert!(is_version_compatible(current - 10, current, 30));
        
        // Outside compatibility range
        assert!(!is_version_compatible(current - 50, current, 30));
        
        // Future versions not compatible
        assert!(!is_version_compatible(current + 10, current, 30));
    }

    #[test]
    fn test_date_string_conversion() {
        // Test a known date
        let days_2020 = 18262; // Approximate days for 2020-01-01
        let date_str = days_to_date_string(days_2020);
        
        // Should start with "20" (for 2020s)
        assert!(date_str.starts_with("20"), "Date should be in 2020s: {}", date_str);
        assert_eq!(date_str.len(), 10, "Date should be YYYY-MM-DD format");
    }

    #[test]
    fn test_version_info() {
        let info = get_version_info();
        
        // Check required fields exist in JSON string
        assert!(info.contains("api_version"), "Should contain api_version");
        assert!(info.contains("release_date"), "Should contain release_date");
        assert!(info.contains("compatibility"), "Should contain compatibility");
        assert!(info.contains("version_format"), "Should contain version_format");
        
        // Should be valid JSON-like format
        assert!(info.starts_with("{"), "Should be JSON format");
        assert!(info.ends_with("}"), "Should be JSON format");
    }

    #[test]
    fn test_api_compatibility() {
        let current = get_api_version();
        
        // Current version should be compatible
        assert!(is_api_compatible(current), "Current version should be compatible");
        
        // Older versions should be compatible
        assert!(is_api_compatible(current - 100), "Older versions should be compatible");
        
        // Future versions should not be compatible
        assert!(!is_api_compatible(current + 1), "Future versions should not be compatible");
    }
}
