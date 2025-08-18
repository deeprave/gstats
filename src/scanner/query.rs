//! Query Parameters
//! 
//! Query parameter structures for filtering scenarios.

use std::time::SystemTime;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Query parameters for repository scanning
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct QueryParams {
    /// Date range for filtering
    pub date_range: Option<DateRange>,
    /// File path filters
    pub file_paths: FilePathFilter,
    /// Maximum number of records
    pub limit: Option<usize>,
    /// Author filters
    pub authors: AuthorFilter,
    /// Branch to scan (if None, uses default branch detection)
    pub branch: Option<String>,
}

/// Date range specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DateRange {
    /// Start date (inclusive)
    pub start: Option<SystemTime>,
    /// End date (inclusive)  
    pub end: Option<SystemTime>,
}

impl DateRange {
    /// Create a new date range with both start and end dates
    pub fn new(start: SystemTime, end: SystemTime) -> Self {
        Self {
            start: Some(start),
            end: Some(end),
        }
    }
    
    /// Create a date range starting from a specific date (no end date)
    pub fn from(start: SystemTime) -> Self {
        Self {
            start: Some(start),
            end: None,
        }
    }
    
    /// Create a date range ending at a specific date (no start date)
    pub fn until(end: SystemTime) -> Self {
        Self {
            start: None,
            end: Some(end),
        }
    }
    
    /// Check if this date range has both start and end dates
    pub fn is_bounded(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }
    
    /// Check if a given time falls within this date range
    pub fn contains(&self, time: SystemTime) -> bool {
        let after_start = self.start.is_none_or(|start| time >= start);
        let before_end = self.end.is_none_or(|end| time <= end);
        after_start && before_end
    }
}

/// File path filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FilePathFilter {
    /// Paths to include (empty means include all)
    pub include: Vec<PathBuf>,
    /// Paths to exclude
    pub exclude: Vec<PathBuf>,
}

/// Author filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AuthorFilter {
    /// Authors to include (empty means include all)
    pub include: Vec<String>,
    /// Authors to exclude
    pub exclude: Vec<String>,
}

/// Query parameter validation errors
#[derive(Error, Debug, PartialEq)]
pub enum QueryValidationError {
    #[error("Invalid date range: start date {start:?} is after end date {end:?}")]
    InvalidDateRange { start: SystemTime, end: SystemTime },
    #[error("Invalid limit: {limit} must be greater than 0")]
    InvalidLimit { limit: usize },
    #[error("Empty file path provided")]
    EmptyFilePath,
    #[error("Empty author name provided")]
    EmptyAuthor,
    #[error("Empty branch name provided")]
    EmptyBranch,
}




impl QueryParams {
    /// Create a new empty query parameters instance
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if this query has any date constraints
    pub fn has_date_filter(&self) -> bool {
        self.date_range.is_some()
    }
    
    /// Check if this query has a limit constraint
    pub fn has_limit(&self) -> bool {
        self.limit.is_some()
    }
    
    /// Get the effective limit, returning None if no limit is set
    pub fn effective_limit(&self) -> Option<usize> {
        self.limit
    }
    
    /// Check if this query has a branch constraint
    pub fn has_branch(&self) -> bool {
        self.branch.is_some()
    }
    
    /// Get the effective branch, returning None if no branch is set
    pub fn effective_branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// Validate query parameters for consistency
    pub fn validate(&self) -> Result<(), QueryValidationError> {
        // Validate date range if present
        if let Some(date_range) = &self.date_range {
            if let (Some(start), Some(end)) = (date_range.start, date_range.end) {
                if start > end {
                    return Err(QueryValidationError::InvalidDateRange { start, end });
                }
            }
        }

        // Validate limit if present
        if let Some(limit) = self.limit {
            if limit == 0 {
                return Err(QueryValidationError::InvalidLimit { limit });
            }
        }

        // Validate file paths
        for path in &self.file_paths.include {
            if path.as_os_str().is_empty() {
                return Err(QueryValidationError::EmptyFilePath);
            }
        }
        for path in &self.file_paths.exclude {
            if path.as_os_str().is_empty() {
                return Err(QueryValidationError::EmptyFilePath);
            }
        }

        // Validate authors
        for author in &self.authors.include {
            if author.is_empty() {
                return Err(QueryValidationError::EmptyAuthor);
            }
        }
        for author in &self.authors.exclude {
            if author.is_empty() {
                return Err(QueryValidationError::EmptyAuthor);
            }
        }

        // Validate branch
        if let Some(ref branch) = self.branch {
            if branch.is_empty() {
                return Err(QueryValidationError::EmptyBranch);
            }
        }

        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn test_default_query_params() {
        let params = QueryParams::default();
        assert!(params.date_range.is_none());
        assert!(params.file_paths.include.is_empty());
        assert!(params.file_paths.exclude.is_empty());
        assert!(params.limit.is_none());
        assert!(params.authors.include.is_empty());
        assert!(params.authors.exclude.is_empty());
        assert!(params.branch.is_none());
    }

    #[test]
    fn test_query_params_validation_direct() {
        // Test validation of already-constructed QueryParams
        let valid_params = QueryParams::default();
        assert!(valid_params.validate().is_ok());
        
        // Test with valid date range
        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        let end_time = UNIX_EPOCH + Duration::from_secs(2000);
        let valid_range = QueryParams {
            date_range: Some(DateRange {
                start: Some(start_time),
                end: Some(end_time),
            }),
            ..Default::default()
        };
        assert!(valid_range.validate().is_ok());
    }

    #[test]
    fn test_date_range_convenience_methods() {
        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        let end_time = UNIX_EPOCH + Duration::from_secs(2000);
        let middle_time = UNIX_EPOCH + Duration::from_secs(1500);
        let before_time = UNIX_EPOCH + Duration::from_secs(500);
        let after_time = UNIX_EPOCH + Duration::from_secs(2500);
        
        // Test bounded range
        let bounded_range = DateRange::new(start_time, end_time);
        assert!(bounded_range.is_bounded());
        assert!(bounded_range.contains(start_time));
        assert!(bounded_range.contains(middle_time));
        assert!(bounded_range.contains(end_time));
        assert!(!bounded_range.contains(before_time));
        assert!(!bounded_range.contains(after_time));
        
        // Test unbounded start
        let unbounded_start = DateRange::until(end_time);
        assert!(!unbounded_start.is_bounded());
        assert!(unbounded_start.contains(before_time));
        assert!(unbounded_start.contains(start_time));
        assert!(unbounded_start.contains(end_time));
        assert!(!unbounded_start.contains(after_time));
        
        // Test unbounded end
        let unbounded_end = DateRange::from(start_time);
        assert!(!unbounded_end.is_bounded());
        assert!(!unbounded_end.contains(before_time));
        assert!(unbounded_end.contains(start_time));
        assert!(unbounded_end.contains(middle_time));
        assert!(unbounded_end.contains(after_time));
    }

    #[test] 
    fn test_query_params_with_branch_field() {
        let mut params = QueryParams::default();
        // This should fail until branch field is implemented
        assert!(params.branch.is_none());
        
        params.branch = Some("develop".to_string());
        assert_eq!(params.branch, Some("develop".to_string()));
    }

    #[test]
    fn test_branch_validation() {
        let params = QueryParams {
            branch: Some("".to_string()), // Empty branch should fail validation
            ..Default::default()
        };
        
        // This should fail until branch validation is implemented
        let result = params.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), QueryValidationError::EmptyBranch);
    }

    #[test]
    fn test_query_params_branch_convenience_methods() {
        let params_without_branch = QueryParams::default();
        assert!(!params_without_branch.has_branch());
        assert_eq!(params_without_branch.effective_branch(), None);

        let params_with_branch = QueryParams {
            branch: Some("main".to_string()),
            ..Default::default()
        };
        assert!(params_with_branch.has_branch());
        assert_eq!(params_with_branch.effective_branch(), Some("main"));
    }
}
