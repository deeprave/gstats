//! Memory size parsing utilities
//! 
//! Parses memory size strings with various units (MB, GB, K, T, etc.)

use thiserror::Error;

/// Memory parsing errors
#[derive(Debug, Error)]
pub enum MemoryParseError {
    #[error("Invalid memory size format: {input}. Expected format like '512MB', '1GB', '2048K'")]
    InvalidFormat { input: String },
    
    #[error("Invalid memory unit: {unit}. Supported units: B, K, M, G, T (with optional 'B' suffix)")]
    InvalidUnit { unit: String },
    
    #[error("Invalid memory value: {value}. Must be a positive number")]
    InvalidValue { value: String },
    
    #[error("Memory size too large: {size} bytes exceeds maximum supported size")]
    SizeTooLarge { size: u64 },
}

/// Parse a memory size string into bytes
/// 
/// Supports various units:
/// - B, Bytes: 1024B = 1024 bytes
/// - K, KB, KiB: 1K = 1024 bytes
/// - M, MB, MiB: 1M = 1024 * 1024 bytes  
/// - G, GB, GiB: 1G = 1024 * 1024 * 1024 bytes
/// - T, TB, TiB: 1T = 1024^4 bytes
/// 
/// Also supports decimal notation:
/// - 0.5G = 512MB
/// - 1.5T = 1536GB
/// 
/// # Examples
/// 
/// ```
/// use gstats::cli::memory_parser::parse_memory_size;
/// 
/// assert_eq!(parse_memory_size("1024").unwrap(), 1024);
/// assert_eq!(parse_memory_size("1K").unwrap(), 1024);
/// assert_eq!(parse_memory_size("1KB").unwrap(), 1024);
/// assert_eq!(parse_memory_size("1M").unwrap(), 1024 * 1024);
/// assert_eq!(parse_memory_size("1MB").unwrap(), 1024 * 1024);
/// assert_eq!(parse_memory_size("1G").unwrap(), 1024 * 1024 * 1024);
/// assert_eq!(parse_memory_size("0.5G").unwrap(), 512 * 1024 * 1024);
/// ```
pub fn parse_memory_size(input: &str) -> Result<usize, MemoryParseError> {
    let input = input.trim().to_uppercase();
    
    if input.is_empty() {
        return Err(MemoryParseError::InvalidFormat { input: input.to_string() });
    }
    
    // Extract numeric part and unit part
    let (number_str, unit_str) = extract_number_and_unit(&input)?;
    
    // Parse the numeric value
    let value = number_str.parse::<f64>()
        .map_err(|_| MemoryParseError::InvalidValue { value: number_str.to_string() })?;
    
    if value < 0.0 {
        return Err(MemoryParseError::InvalidValue { value: number_str.to_string() });
    }
    
    // Parse the unit and get multiplier
    let multiplier = parse_unit(&unit_str)?;
    
    // Calculate final size in bytes
    let size_bytes = (value * multiplier as f64) as u64;
    
    // Check for overflow
    if size_bytes > usize::MAX as u64 {
        return Err(MemoryParseError::SizeTooLarge { size: size_bytes });
    }
    
    Ok(size_bytes as usize)
}

/// Extract numeric and unit parts from input string
fn extract_number_and_unit(input: &str) -> Result<(String, String), MemoryParseError> {
    let mut number_end = 0;
    let mut found_decimal = false;
    
    for (i, ch) in input.char_indices() {
        match ch {
            '0'..='9' => number_end = i + 1,
            '.' if !found_decimal => {
                found_decimal = true;
                number_end = i + 1;
            }
            _ => break,
        }
    }
    
    if number_end == 0 {
        return Err(MemoryParseError::InvalidFormat { input: input.to_string() });
    }
    
    let number_str = input[..number_end].to_string();
    let unit_str = input[number_end..].to_string();
    
    Ok((number_str, unit_str))
}

/// Parse unit string and return multiplier
fn parse_unit(unit: &str) -> Result<u64, MemoryParseError> {
    let unit = unit.trim();
    
    match unit {
        "" | "B" | "BYTES" => Ok(1),
        "K" | "KB" | "KIB" => Ok(1024),
        "M" | "MB" | "MIB" => Ok(1024 * 1024),
        "G" | "GB" | "GIB" => Ok(1024 * 1024 * 1024),
        "T" | "TB" | "TIB" => Ok(1024_u64.pow(4)),
        _ => Err(MemoryParseError::InvalidUnit { unit: unit.to_string() }),
    }
}

/// Format bytes as human-readable string
pub fn format_memory_size(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else if size.fract() == 0.0 {
        format!("{:.0} {}", size, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_size_bytes() {
        assert_eq!(parse_memory_size("1024").unwrap(), 1024);
        assert_eq!(parse_memory_size("1024B").unwrap(), 1024);
        assert_eq!(parse_memory_size("1024 bytes").unwrap(), 1024);
    }
    
    #[test]
    fn test_parse_memory_size_kilobytes() {
        assert_eq!(parse_memory_size("1K").unwrap(), 1024);
        assert_eq!(parse_memory_size("1KB").unwrap(), 1024);
        assert_eq!(parse_memory_size("1KiB").unwrap(), 1024);
        assert_eq!(parse_memory_size("2K").unwrap(), 2048);
    }
    
    #[test]
    fn test_parse_memory_size_megabytes() {
        assert_eq!(parse_memory_size("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_size("1MB").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_size("512M").unwrap(), 512 * 1024 * 1024);
    }
    
    #[test]
    fn test_parse_memory_size_gigabytes() {
        assert_eq!(parse_memory_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("2G").unwrap(), 2 * 1024 * 1024 * 1024);
    }
    
    #[test]
    fn test_parse_memory_size_terabytes() {
        assert_eq!(parse_memory_size("1T").unwrap(), 1024_usize.pow(4));
        assert_eq!(parse_memory_size("1TB").unwrap(), 1024_usize.pow(4));
    }
    
    #[test]
    fn test_parse_memory_size_decimal() {
        assert_eq!(parse_memory_size("0.5G").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_memory_size("1.5M").unwrap(), (1.5 * 1024.0 * 1024.0) as usize);
        assert_eq!(parse_memory_size("2.5K").unwrap(), (2.5 * 1024.0) as usize);
    }
    
    #[test]
    fn test_parse_memory_size_case_insensitive() {
        assert_eq!(parse_memory_size("1gb").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("512mb").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_memory_size("1k").unwrap(), 1024);
    }
    
    #[test]
    fn test_parse_memory_size_whitespace() {
        assert_eq!(parse_memory_size("  1GB  ").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("512 MB").unwrap(), 512 * 1024 * 1024);
    }
    
    #[test]
    fn test_parse_memory_size_errors() {
        // Invalid format
        assert!(parse_memory_size("").is_err());
        assert!(parse_memory_size("abc").is_err());
        assert!(parse_memory_size("1.2.3GB").is_err());
        
        // Invalid unit
        assert!(parse_memory_size("1X").is_err());
        assert!(parse_memory_size("1ZB").is_err());
        
        // Invalid value
        assert!(parse_memory_size("-1GB").is_err());
    }
    
    #[test]
    fn test_format_memory_size() {
        assert_eq!(format_memory_size(1024), "1 KB");
        assert_eq!(format_memory_size(1024 * 1024), "1 MB");
        assert_eq!(format_memory_size(1024 * 1024 * 1024), "1 GB");
        assert_eq!(format_memory_size(512 * 1024 * 1024), "512 MB");
        assert_eq!(format_memory_size(1536 * 1024 * 1024), "1.5 GB");
        assert_eq!(format_memory_size(1023), "1023 B");
    }
    
    #[test]
    fn test_extract_number_and_unit() {
        assert_eq!(extract_number_and_unit("1024").unwrap(), ("1024".to_string(), "".to_string()));
        assert_eq!(extract_number_and_unit("1024MB").unwrap(), ("1024".to_string(), "MB".to_string()));
        assert_eq!(extract_number_and_unit("1.5G").unwrap(), ("1.5".to_string(), "G".to_string()));
        assert_eq!(extract_number_and_unit("0.5TB").unwrap(), ("0.5".to_string(), "TB".to_string()));
    }
}
