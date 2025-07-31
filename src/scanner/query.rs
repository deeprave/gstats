//! Query Parameters
//! 
//! Query parameter structures for filtering scenarios.

use std::time::SystemTime;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Query parameters for repository scanning
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryParams {
    /// Date range for filtering
    pub date_range: Option<DateRange>,
    /// File path filters
    pub file_paths: FilePathFilter,
    /// Maximum number of records
    pub limit: Option<usize>,
    /// Author filters
    pub authors: AuthorFilter,
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
        let after_start = self.start.map_or(true, |start| time >= start);
        let before_end = self.end.map_or(true, |end| time <= end);
        after_start && before_end
    }
}

/// File path filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilePathFilter {
    /// Paths to include (empty means include all)
    pub include: Vec<PathBuf>,
    /// Paths to exclude
    pub exclude: Vec<PathBuf>,
}

/// Author filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
}

/// Builder for constructing query parameters
#[derive(Debug, Default)]
pub struct QueryBuilder {
    date_range: Option<DateRange>,
    file_paths: FilePathFilter,
    limit: Option<usize>,
    authors: AuthorFilter,
}

impl Default for QueryParams {
    fn default() -> Self {
        Self {
            date_range: None,
            file_paths: FilePathFilter::default(),
            limit: None,
            authors: AuthorFilter::default(),
        }
    }
}

impl Default for FilePathFilter {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

impl Default for AuthorFilter {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
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
    
    /// Check if this query has any file path constraints
    pub fn has_path_filter(&self) -> bool {
        !self.file_paths.include.is_empty() || !self.file_paths.exclude.is_empty()
    }
    
    /// Check if this query has any author constraints
    pub fn has_author_filter(&self) -> bool {
        !self.authors.include.is_empty() || !self.authors.exclude.is_empty()
    }
    
    /// Check if this query has a limit constraint
    pub fn has_limit(&self) -> bool {
        self.limit.is_some()
    }
    
    /// Get the effective limit, returning None if no limit is set
    pub fn effective_limit(&self) -> Option<usize> {
        self.limit
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

        Ok(())
    }

    /// Create a new builder for query parameters
    pub fn builder() -> QueryBuilder {
        QueryBuilder::default()
    }
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set date range filter
    pub fn date_range(mut self, start: Option<SystemTime>, end: Option<SystemTime>) -> Self {
        self.date_range = Some(DateRange { start, end });
        self
    }
    
    /// Set start date filter
    pub fn since(mut self, start: SystemTime) -> Self {
        let range = self.date_range.get_or_insert(DateRange { start: None, end: None });
        range.start = Some(start);
        self
    }
    
    /// Set end date filter
    pub fn until(mut self, end: SystemTime) -> Self {
        let range = self.date_range.get_or_insert(DateRange { start: None, end: None });
        range.end = Some(end);
        self
    }

    /// Add file path to include
    pub fn include_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        let path_buf = path.into();
        self.file_paths.include.push(path_buf);
        self
    }

    /// Add file path to exclude
    pub fn exclude_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        let path_buf = path.into();
        self.file_paths.exclude.push(path_buf);
        self
    }

    /// Set maximum number of records
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Add author to include
    pub fn include_author<S: Into<String>>(mut self, author: S) -> Self {
        let author_string = author.into();
        self.authors.include.push(author_string);
        self
    }
    
    /// Add author to include (convenience alias)
    pub fn author<S: Into<String>>(self, author: S) -> Self {
        self.include_author(author)
    }

    /// Add author to exclude
    pub fn exclude_author<S: Into<String>>(mut self, author: S) -> Self {
        let author_string = author.into();
        self.authors.exclude.push(author_string);
        self
    }

    /// Build and validate query parameters
    pub fn build(self) -> Result<QueryParams, QueryValidationError> {
        let params = QueryParams {
            date_range: self.date_range,
            file_paths: self.file_paths,
            limit: self.limit,
            authors: self.authors,
        };
        
        params.validate()?;
        Ok(params)
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
    }

    #[test]
    fn test_query_builder_pattern() {
        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        let end_time = UNIX_EPOCH + Duration::from_secs(2000);
        
        let result = QueryParams::builder()
            .date_range(Some(start_time), Some(end_time))
            .include_path("src/")
            .exclude_path("target/")
            .limit(100)
            .include_author("alice")
            .exclude_author("bot")
            .build();
            
        assert!(result.is_ok(), "Builder should succeed with valid parameters");
        let params = result.unwrap();
        
        // Test date range
        assert!(params.date_range.is_some());
        let date_range = params.date_range.unwrap();
        assert_eq!(date_range.start, Some(start_time));
        assert_eq!(date_range.end, Some(end_time));
        
        // Test file paths
        assert_eq!(params.file_paths.include.len(), 1);
        assert_eq!(params.file_paths.include[0], PathBuf::from("src/"));
        assert_eq!(params.file_paths.exclude.len(), 1);
        assert_eq!(params.file_paths.exclude[0], PathBuf::from("target/"));
        
        // Test limit
        assert_eq!(params.limit, Some(100));
        
        // Test authors
        assert_eq!(params.authors.include.len(), 1);
        assert_eq!(params.authors.include[0], "alice");
        assert_eq!(params.authors.exclude.len(), 1);
        assert_eq!(params.authors.exclude[0], "bot");
    }

    #[test]
    fn test_query_validation_invalid_date_range() {
        let start_time = UNIX_EPOCH + Duration::from_secs(2000);
        let end_time = UNIX_EPOCH + Duration::from_secs(1000);
        
        let result = QueryParams::builder()
            .date_range(Some(start_time), Some(end_time))
            .build();
            
        assert!(result.is_err(), "Builder should fail with invalid date range");
        let error = result.unwrap_err();
        match error {
            QueryValidationError::InvalidDateRange { start, end } => {
                assert_eq!(start, start_time);
                assert_eq!(end, end_time);
            }
            _ => panic!("Expected InvalidDateRange error"),
        }
    }

    #[test]
    fn test_query_validation_invalid_limit() {
        let result = QueryParams::builder()
            .limit(0)
            .build();
            
        assert!(result.is_err(), "Builder should fail with zero limit");
        let error = result.unwrap_err();
        match error {
            QueryValidationError::InvalidLimit { limit } => {
                assert_eq!(limit, 0);
            }
            _ => panic!("Expected InvalidLimit error"),
        }
    }

    #[test]
    fn test_query_validation_empty_file_path() {
        let result = QueryParams::builder()
            .include_path("")
            .build();
            
        assert!(result.is_err(), "Builder should fail with empty file path");
        assert_eq!(result.unwrap_err(), QueryValidationError::EmptyFilePath);
    }

    #[test]
    fn test_query_validation_empty_author() {
        let result = QueryParams::builder()
            .include_author("")
            .build();
            
        assert!(result.is_err(), "Builder should fail with empty author");
        assert_eq!(result.unwrap_err(), QueryValidationError::EmptyAuthor);
    }

    #[test]
    fn test_query_serialization() {
        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        let end_time = UNIX_EPOCH + Duration::from_secs(2000);
        
        let params = QueryParams::builder()
            .date_range(Some(start_time), Some(end_time))
            .include_path("src/")
            .limit(50)
            .include_author("alice")
            .build()
            .expect("Valid query params");
            
        // Test serialization round-trip
        let serialized = bincode::serialize(&params).expect("Serialization should succeed");
        let deserialized: QueryParams = bincode::deserialize(&serialized).expect("Deserialization should succeed");
        
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_multiple_file_paths() {
        let result = QueryParams::builder()
            .include_path("src/")
            .include_path("tests/")
            .exclude_path("target/")
            .exclude_path("build/")
            .build();
            
        assert!(result.is_ok());
        let params = result.unwrap();
        
        assert_eq!(params.file_paths.include.len(), 2);
        assert!(params.file_paths.include.contains(&PathBuf::from("src/")));
        assert!(params.file_paths.include.contains(&PathBuf::from("tests/")));
        
        assert_eq!(params.file_paths.exclude.len(), 2);
        assert!(params.file_paths.exclude.contains(&PathBuf::from("target/")));
        assert!(params.file_paths.exclude.contains(&PathBuf::from("build/")));
    }

    #[test]
    fn test_multiple_authors() {
        let result = QueryParams::builder()
            .include_author("alice")
            .include_author("bob")
            .exclude_author("bot1")
            .exclude_author("bot2")
            .build();
            
        assert!(result.is_ok());
        let params = result.unwrap();
        
        assert_eq!(params.authors.include.len(), 2);
        assert!(params.authors.include.contains(&"alice".to_string()));
        assert!(params.authors.include.contains(&"bob".to_string()));
        
        assert_eq!(params.authors.exclude.len(), 2);
        assert!(params.authors.exclude.contains(&"bot1".to_string()));
        assert!(params.authors.exclude.contains(&"bot2".to_string()));
    }

    #[test]
    fn test_partial_date_range() {
        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        
        // Test with only start date
        let result1 = QueryParams::builder()
            .date_range(Some(start_time), None)
            .build();
        assert!(result1.is_ok());
        let params1 = result1.unwrap();
        let date_range1 = params1.date_range.as_ref().unwrap();
        assert_eq!(date_range1.start, Some(start_time));
        assert_eq!(date_range1.end, None);
        
        // Test with only end date
        let result2 = QueryParams::builder()
            .date_range(None, Some(start_time))
            .build();
        assert!(result2.is_ok());
        let params2 = result2.unwrap();
        let date_range2 = params2.date_range.as_ref().unwrap();
        assert_eq!(date_range2.start, None);
        assert_eq!(date_range2.end, Some(start_time));
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
    fn test_query_params_convenience_methods() {
        let params = QueryParams::builder()
            .date_range(Some(UNIX_EPOCH), Some(UNIX_EPOCH + Duration::from_secs(1000)))
            .include_path("src/")
            .include_author("alice")
            .limit(100)
            .build()
            .unwrap();
            
        assert!(params.has_date_filter());
        assert!(params.has_path_filter());
        assert!(params.has_author_filter());
        assert!(params.has_limit());
        assert_eq!(params.effective_limit(), Some(100));
        
        let empty_params = QueryParams::new();
        assert!(!empty_params.has_date_filter());
        assert!(!empty_params.has_path_filter());
        assert!(!empty_params.has_author_filter());
        assert!(!empty_params.has_limit());
        assert_eq!(empty_params.effective_limit(), None);
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
        
        // Test from range (start only)
        let from_range = DateRange::from(start_time);
        assert!(!from_range.is_bounded());
        assert!(from_range.contains(start_time));
        assert!(from_range.contains(middle_time));
        assert!(from_range.contains(end_time));
        assert!(from_range.contains(after_time));
        assert!(!from_range.contains(before_time));
        
        // Test until range (end only)
        let until_range = DateRange::until(end_time);
        assert!(!until_range.is_bounded());
        assert!(until_range.contains(start_time));
        assert!(until_range.contains(middle_time));
        assert!(until_range.contains(end_time));
        assert!(until_range.contains(before_time));
        assert!(!until_range.contains(after_time));
    }
}
