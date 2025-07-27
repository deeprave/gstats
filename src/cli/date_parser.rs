//! Date parsing module for CLI arguments
//! 
//! Supports both absolute and relative date formats:
//! - Absolute: ISO 8601 formats like "2023-01-01", "2023-01-01T10:30:00"
//! - Relative: Human-readable formats like "1 week ago", "yesterday", "last month"

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use std::time::SystemTime;

/// Error types for date parsing
#[derive(Debug, thiserror::Error)]
pub enum DateParseError {
    #[error("Invalid date format: {input}. Expected ISO 8601 (YYYY-MM-DD) or relative format (e.g., '1 week ago')")]
    InvalidFormat { input: String },
    
    #[error("Invalid relative date: {input}. Expected format like '1 week ago', 'yesterday', 'last month'")]
    InvalidRelativeFormat { input: String },
    
    #[error("Unsupported time unit: {unit}. Supported units: seconds, minutes, hours, days, weeks, months, years")]
    UnsupportedUnit { unit: String },
    
    #[error("Invalid number in relative date: {input}")]
    InvalidNumber { input: String },
    
    #[error("Date range validation failed: start date {start} is after end date {end}")]
    InvalidRange { start: String, end: String },
}

/// Parse a date string into SystemTime
/// 
/// Supports both absolute and relative formats:
/// - Absolute: "2023-01-01", "2023-01-01T10:30:00", "2023-01-01T10:30:00Z"
/// - Relative: "1 week ago", "3 months ago", "yesterday", "today", "tomorrow"
/// - Named periods: "last week", "last month", "last year"
/// 
/// # Examples
/// 
/// ```
/// use gstats::cli::date_parser::parse_date;
/// 
/// // Absolute dates
/// let date1 = parse_date("2023-01-01").unwrap();
/// let date2 = parse_date("2023-01-01T10:30:00").unwrap();
/// 
/// // Relative dates
/// let date3 = parse_date("1 week ago").unwrap();
/// let date4 = parse_date("yesterday").unwrap();
/// ```
pub fn parse_date(input: &str) -> Result<SystemTime, DateParseError> {
    let trimmed = input.trim();
    
    // Try absolute date formats first
    if let Ok(system_time) = parse_absolute_date(trimmed) {
        return Ok(system_time);
    }
    
    // Try relative date formats
    parse_relative_date(trimmed)
}

/// Parse absolute date formats (ISO 8601)
fn parse_absolute_date(input: &str) -> Result<SystemTime, DateParseError> {
    // Try various ISO 8601 formats
    
    // Full datetime with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(dt.into());
    }
    
    // Try datetime without timezone (assume UTC)
    if let Ok(naive_dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S") {
        let dt = Utc.from_utc_datetime(&naive_dt);
        return Ok(dt.into());
    }
    
    // Try datetime with different format
    if let Ok(naive_dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        let dt = Utc.from_utc_datetime(&naive_dt);
        return Ok(dt.into());
    }
    
    // Try date only (assume start of day in local timezone)
    if let Ok(naive_date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap();
        let dt = Local.from_local_datetime(&naive_dt).single()
            .ok_or_else(|| DateParseError::InvalidFormat { input: input.to_string() })?;
        return Ok(dt.into());
    }
    
    Err(DateParseError::InvalidFormat { input: input.to_string() })
}

/// Parse relative date formats
fn parse_relative_date(input: &str) -> Result<SystemTime, DateParseError> {
    let input_lower = input.to_lowercase();
    let now = Local::now();
    
    // Handle named dates first
    match input_lower.as_str() {
        "today" => return Ok(now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).single().unwrap().into()),
        "yesterday" => return Ok((now - chrono::Duration::days(1)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).single().unwrap().into()),
        "tomorrow" => return Ok((now + chrono::Duration::days(1)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).single().unwrap().into()),
        "last week" => return Ok((now - chrono::Duration::weeks(1)).into()),
        "last month" => return Ok((now - chrono::Duration::days(30)).into()),
        "last year" => return Ok((now - chrono::Duration::days(365)).into()),
        _ => {}
    }
    
    // Parse "X unit ago" format
    let parts: Vec<&str> = input_lower.split_whitespace().collect();
    if parts.len() >= 3 && parts[parts.len() - 1] == "ago" {
        let number_str = parts[0];
        let unit = parts[1];
        
        let number = number_str.parse::<i64>()
            .map_err(|_| DateParseError::InvalidNumber { input: input.to_string() })?;
        
        let duration = match unit {
            "second" | "seconds" => chrono::Duration::seconds(number),
            "minute" | "minutes" => chrono::Duration::minutes(number),
            "hour" | "hours" => chrono::Duration::hours(number),
            "day" | "days" => chrono::Duration::days(number),
            "week" | "weeks" => chrono::Duration::weeks(number),
            "month" | "months" => chrono::Duration::days(number * 30), // Approximate
            "year" | "years" => chrono::Duration::days(number * 365), // Approximate
            _ => return Err(DateParseError::UnsupportedUnit { unit: unit.to_string() }),
        };
        
        let result_time = now - duration;
        return Ok(result_time.into());
    }
    
    Err(DateParseError::InvalidRelativeFormat { input: input.to_string() })
}

/// Validate that a date range is logical (start <= end)
pub fn validate_date_range(start: Option<&str>, end: Option<&str>) -> Result<(), DateParseError> {
    if let (Some(start_str), Some(end_str)) = (start, end) {
        let start_time = parse_date(start_str)?;
        let end_time = parse_date(end_str)?;
        
        if start_time > end_time {
            return Err(DateParseError::InvalidRange {
                start: start_str.to_string(),
                end: end_str.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_parse_absolute_date_iso_8601() {
        // Test basic date format
        let result = parse_date("2023-01-01").unwrap();
        assert!(result > SystemTime::UNIX_EPOCH);
        
        // Test datetime format
        let result = parse_date("2023-01-01T10:30:00").unwrap();
        assert!(result > SystemTime::UNIX_EPOCH);
        
        // Test RFC3339 format
        let result = parse_date("2023-01-01T10:30:00Z").unwrap();
        assert!(result > SystemTime::UNIX_EPOCH);
    }
    
    #[test]
    fn test_parse_relative_date_named() {
        let now = SystemTime::now();
        
        // Test "today" - should be close to now
        let today = parse_date("today").unwrap();
        let diff = now.duration_since(today).unwrap_or_else(|_| today.duration_since(now).unwrap());
        assert!(diff < Duration::from_secs(24 * 60 * 60)); // Within 24 hours
        
        // Test "yesterday" 
        let yesterday = parse_date("yesterday").unwrap();
        assert!(yesterday < now);
        
        // Test "tomorrow"
        let tomorrow = parse_date("tomorrow").unwrap();
        assert!(tomorrow > now);
    }
    
    #[test]
    fn test_parse_relative_date_ago_format() {
        let now = SystemTime::now();
        
        // Test "1 day ago"
        let one_day_ago = parse_date("1 day ago").unwrap();
        assert!(one_day_ago < now);
        
        // Test "2 weeks ago"
        let two_weeks_ago = parse_date("2 weeks ago").unwrap();
        assert!(two_weeks_ago < one_day_ago);
        
        // Test "3 months ago"
        let three_months_ago = parse_date("3 months ago").unwrap();
        assert!(three_months_ago < two_weeks_ago);
    }
    
    #[test]
    fn test_parse_relative_date_units() {
        let now = SystemTime::now();
        
        // Test all supported units
        assert!(parse_date("1 second ago").unwrap() < now);
        assert!(parse_date("1 minute ago").unwrap() < now);
        assert!(parse_date("1 hour ago").unwrap() < now);
        assert!(parse_date("1 day ago").unwrap() < now);
        assert!(parse_date("1 week ago").unwrap() < now);
        assert!(parse_date("1 month ago").unwrap() < now);
        assert!(parse_date("1 year ago").unwrap() < now);
        
        // Test plural forms
        assert!(parse_date("2 days ago").unwrap() < now);
        assert!(parse_date("3 weeks ago").unwrap() < now);
    }
    
    #[test]
    fn test_parse_date_error_cases() {
        // Test invalid absolute format
        assert!(parse_date("not-a-date").is_err());
        assert!(parse_date("2023-13-01").is_err()); // Invalid month
        
        // Test invalid relative format
        assert!(parse_date("abc days ago").is_err()); // Invalid number
        assert!(parse_date("1 invalid ago").is_err()); // Invalid unit
        assert!(parse_date("1 day").is_err()); // Missing "ago"
    }
    
    #[test]
    fn test_validate_date_range() {
        // Valid range
        assert!(validate_date_range(Some("2023-01-01"), Some("2023-12-31")).is_ok());
        
        // Invalid range (start > end)
        assert!(validate_date_range(Some("2023-12-31"), Some("2023-01-01")).is_err());
        
        // Single dates (should be valid)
        assert!(validate_date_range(Some("2023-01-01"), None).is_ok());
        assert!(validate_date_range(None, Some("2023-12-31")).is_ok());
        
        // No dates (should be valid)
        assert!(validate_date_range(None, None).is_ok());
    }
    
    #[test]
    fn test_whitespace_handling() {
        // Test trimming whitespace
        assert!(parse_date("  2023-01-01  ").is_ok());
        assert!(parse_date("  1 day ago  ").is_ok());
        assert!(parse_date("  yesterday  ").is_ok());
    }
    
    #[test]
    fn test_case_insensitive() {
        // Test case insensitive parsing
        assert!(parse_date("YESTERDAY").is_ok());
        assert!(parse_date("Today").is_ok());
        assert!(parse_date("1 DAY AGO").is_ok());
        assert!(parse_date("LAST WEEK").is_ok());
    }
}
