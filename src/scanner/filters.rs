//! Filtering System
//! 
//! Hybrid filtering system with built-in filters and functional callbacks using zero-cost abstractions.

use std::ops::ControlFlow;
use std::time::SystemTime;
use std::path::Path;
use crate::scanner::query::{QueryParams, DateRange, FilePathFilter, AuthorFilter as QueryAuthorFilter};

/// Filter result for early termination using ControlFlow
pub type FilterResult<T> = ControlFlow<(), T>;

/// Filter composition result for chaining operations
pub type FilterChainResult<T> = ControlFlow<T, ()>;

/// Core filter trait using zero-cost abstractions
pub trait ScanFilter<T> {
    /// Apply filter to input, returning ControlFlow for early termination
    fn apply(&self, input: &T) -> FilterResult<()>;
    
    /// Chain this filter with another using iterator-style composition
    fn and_then<F>(self, other: F) -> ChainedFilter<Self, F>
    where
        Self: Sized,
        F: ScanFilter<T>,
    {
        ChainedFilter::new(self, other)
    }
}

/// Built-in date range filter using zero-cost abstractions
#[derive(Debug, Clone)]
pub struct DateFilter {
    range: DateRange,
}

/// Built-in file path filter using iterator combinators
#[derive(Debug, Clone)]
pub struct PathFilter {
    filter: FilePathFilter,
}

/// Built-in author filter using pattern matching
#[derive(Debug, Clone)]
pub struct AuthorFilter {
    filter: QueryAuthorFilter,
}

/// Functional callback filter using closures for zero-cost abstractions
pub struct CallbackFilter<F> {
    predicate: F,
}

/// Chained filter for composition using iterator-style patterns
pub struct ChainedFilter<F1, F2> {
    first: F1,
    second: F2,
}

/// Commit data for filtering operations
#[derive(Debug, Clone)]
pub struct CommitData {
    pub timestamp: SystemTime,
    pub author: String,
    pub file_paths: Vec<String>,
    pub message: String,
}

/// Filter executor for applying filters with early termination
pub struct FilterExecutor {
    limit: Option<usize>,
}

impl ScanFilter<CommitData> for DateFilter {
    fn apply(&self, input: &CommitData) -> FilterResult<()> {
        if self.range.contains(input.timestamp) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl ScanFilter<CommitData> for PathFilter {
    fn apply(&self, input: &CommitData) -> FilterResult<()> {
        let has_included = if self.filter.include.is_empty() {
            true
        } else {
            input.file_paths.iter().any(|file_path| {
                self.filter.include.iter().any(|include_pattern| {
                    file_path.starts_with(include_pattern.to_string_lossy().as_ref())
                })
            })
        };
        
        let has_excluded = input.file_paths.iter().any(|file_path| {
            self.filter.exclude.iter().any(|exclude_pattern| {
                file_path.starts_with(exclude_pattern.to_string_lossy().as_ref())
            })
        });
        
        if has_included && !has_excluded {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl ScanFilter<CommitData> for AuthorFilter {
    fn apply(&self, input: &CommitData) -> FilterResult<()> {
        let is_included = if self.filter.include.is_empty() {
            true
        } else {
            self.filter.include.contains(&input.author)
        };
        
        let is_excluded = self.filter.exclude.contains(&input.author);
        
        if is_included && !is_excluded {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl<F> ScanFilter<CommitData> for CallbackFilter<F>
where
    F: Fn(&CommitData) -> FilterResult<()>,
{
    fn apply(&self, input: &CommitData) -> FilterResult<()> {
        (self.predicate)(input)
    }
}

impl<F1, F2> ScanFilter<CommitData> for ChainedFilter<F1, F2>
where
    F1: ScanFilter<CommitData>,
    F2: ScanFilter<CommitData>,
{
    fn apply(&self, input: &CommitData) -> FilterResult<()> {
        match self.first.apply(input) {
            ControlFlow::Continue(()) => self.second.apply(input),
            ControlFlow::Break(()) => ControlFlow::Break(()),
        }
    }
}
impl DateFilter {
    /// Create new date filter with range
    pub fn new(range: DateRange) -> Self {
        Self { range }
    }
    
    /// Create filter for commits after given date
    pub fn after(date: SystemTime) -> Self {
        Self {
            range: DateRange::from(date),
        }
    }
    
    /// Create filter for commits before given date
    pub fn before(date: SystemTime) -> Self {
        Self {
            range: DateRange::until(date),
        }
    }
}

impl PathFilter {
    /// Create new path filter
    pub fn new(filter: FilePathFilter) -> Self {
        Self { filter }
    }
    
    /// Create filter that includes specific paths
    pub fn include_paths<I, P>(paths: I) -> Self 
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let include_paths = paths.into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();
        Self {
            filter: FilePathFilter {
                include: include_paths,
                exclude: Vec::new(),
            }
        }
    }
    
    /// Create filter that excludes specific paths
    pub fn exclude_paths<I, P>(paths: I) -> Self 
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let exclude_paths = paths.into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();
        Self {
            filter: FilePathFilter {
                include: Vec::new(),
                exclude: exclude_paths,
            }
        }
    }
}

impl AuthorFilter {
    /// Create new author filter
    pub fn new(filter: QueryAuthorFilter) -> Self {
        Self { filter }
    }
    
    /// Create filter for specific authors
    pub fn include_authors<I, S>(authors: I) -> Self 
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let author_list = authors.into_iter().map(|s| s.into()).collect();
        Self {
            filter: QueryAuthorFilter {
                include: author_list,
                exclude: Vec::new(),
            }
        }
    }
    
    /// Create filter excluding specific authors
    pub fn exclude_authors<I, S>(authors: I) -> Self 
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let author_list = authors.into_iter().map(|s| s.into()).collect();
        Self {
            filter: QueryAuthorFilter {
                include: Vec::new(),
                exclude: author_list,
            }
        }
    }
}

impl<F> CallbackFilter<F>
where
    F: Fn(&CommitData) -> FilterResult<()>,
{
    /// Create new callback filter with predicate
    pub fn new(predicate: F) -> Self {
        Self { predicate }
    }
}

impl<F1, F2> ChainedFilter<F1, F2> {
    /// Create new chained filter
    pub fn new(first: F1, second: F2) -> Self {
        Self { first, second }
    }
}

impl FilterExecutor {
    /// Create new filter executor
    pub fn new() -> Self {
        Self { limit: None }
    }
    
    /// Set limit for maximum results
    pub fn with_limit(limit: usize) -> Self {
        Self { limit: Some(limit) }
    }
    
    /// Apply filter to iterator with early termination
    pub fn apply_filter<I, T, F>(&self, iter: I, filter: F) -> Vec<T>
    where
        I: Iterator<Item = T>,
        F: ScanFilter<T>,
        T: Clone,
    {
        let mut results = Vec::new();
        
        for item in iter {
            match filter.apply(&item) {
                ControlFlow::Continue(()) => {
                    results.push(item);
                    if let Some(limit) = self.limit {
                        if results.len() >= limit {
                            break; // Early termination
                        }
                    }
                }
                ControlFlow::Break(()) => {
                    // Item filtered out, continue to next
                }
            }
        }
        
        results
    }
    
    /// Apply multiple filters using iterator combinators
    pub fn apply_filters_boxed<I, T>(&self, iter: I, filters: Vec<Box<dyn ScanFilter<T>>>) -> Vec<T>
    where
        I: Iterator<Item = T>,
        T: Clone,
    {
        let mut results = Vec::new();
        
        'outer: for item in iter {
            // Apply all filters - all must pass
            for filter in &filters {
                match filter.apply(&item) {
                    ControlFlow::Continue(()) => continue,
                    ControlFlow::Break(()) => continue 'outer, // Skip this item
                }
            }
            
            // All filters passed
            results.push(item);
            if let Some(limit) = self.limit {
                if results.len() >= limit {
                    break; // Early termination
                }
            }
        }
        
        results
    }
    
    /// Create filter from query parameters
    pub fn filter_from_query(query: &QueryParams) -> Box<dyn ScanFilter<CommitData>>
    {
        let mut combined_filters: Vec<Box<dyn ScanFilter<CommitData>>> = Vec::new();
        
        // Add date filter if present
        if let Some(date_range) = &query.date_range {
            combined_filters.push(Box::new(DateFilter::new(date_range.clone())));
        }
        
        // Always add path filter - it correctly handles empty include lists as "include all"
        combined_filters.push(Box::new(PathFilter::new(query.file_paths.clone())));
        
        // Always add author filter - it correctly handles empty include lists as "include all"
        combined_filters.push(Box::new(AuthorFilter::new(query.authors.clone())));
        
        Box::new(CombinedFilter::new(combined_filters))
    }
}

/// Combined filter for multiple filters
struct CombinedFilter {
    filters: Vec<Box<dyn ScanFilter<CommitData>>>,
}

impl CombinedFilter {
    fn new(filters: Vec<Box<dyn ScanFilter<CommitData>>>) -> Self {
        Self { filters }
    }
}

impl ScanFilter<CommitData> for CombinedFilter {
    fn apply(&self, input: &CommitData) -> FilterResult<()> {
        for filter in &self.filters {
            match filter.apply(input) {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(()) => return ControlFlow::Break(()),
            }
        }
        ControlFlow::Continue(())
    }
}

/// Convenience function for creating callback filters
pub fn callback_filter<F>(predicate: F) -> CallbackFilter<F>
where
    F: Fn(&CommitData) -> FilterResult<()>,
{
    CallbackFilter::new(predicate)
}

/// Convenience function for combining multiple filters
pub fn combine_filters<F1, F2>(first: F1, second: F2) -> ChainedFilter<F1, F2>
where
    F1: ScanFilter<CommitData>,
    F2: ScanFilter<CommitData>,
{
    ChainedFilter::new(first, second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};
    use crate::scanner::query::{DateRange, FilePathFilter, AuthorFilter as QueryAuthorFilter};

    fn create_test_commit(timestamp: SystemTime, author: &str, files: Vec<&str>) -> CommitData {
        CommitData {
            timestamp,
            author: author.to_string(),
            file_paths: files.into_iter().map(|s| s.to_string()).collect(),
            message: "test commit".to_string(),
        }
    }

    #[test]
    fn test_date_filter_creation() {
        let start = UNIX_EPOCH + Duration::from_secs(1000);
        let end = UNIX_EPOCH + Duration::from_secs(2000);
        let range = DateRange::new(start, end);
        
        let _filter = DateFilter::new(range);
        // Test that filter was created successfully
        
        let _after_filter = DateFilter::after(start);
        let _before_filter = DateFilter::before(end);
        // Filters should be created without panicking
    }

    #[test]
    fn test_date_filter_application() {
        let start = UNIX_EPOCH + Duration::from_secs(1000);
        let end = UNIX_EPOCH + Duration::from_secs(2000);
        let filter = DateFilter::new(DateRange::new(start, end));
        
        let valid_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(1500), 
            "alice", 
            vec!["src/main.rs"]
        );
        let invalid_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(500), 
            "bob", 
            vec!["src/lib.rs"]
        );
        
        // Valid commit should pass filter
        let result1 = filter.apply(&valid_commit);
        assert!(matches!(result1, ControlFlow::Continue(())));
        
        // Invalid commit should be filtered out
        let result2 = filter.apply(&invalid_commit);
        assert!(matches!(result2, ControlFlow::Break(())));
    }

    #[test]
    fn test_path_filter_creation() {
        let include_paths = vec!["src/", "tests/"];
        let exclude_paths = vec!["target/", "build/"];
        
        let _include_filter = PathFilter::include_paths(include_paths);
        let _exclude_filter = PathFilter::exclude_paths(exclude_paths);
        
        let file_filter = FilePathFilter {
            include: vec!["src/".into(), "tests/".into()],
            exclude: vec!["target/".into()],
        };
        let _combined_filter = PathFilter::new(file_filter);
        // Filters should be created successfully
    }

    #[test]
    fn test_path_filter_application() {
        let filter = PathFilter::include_paths(vec!["src/"]);
        
        let valid_commit = create_test_commit(
            UNIX_EPOCH,
            "alice",
            vec!["src/main.rs", "src/lib.rs"]
        );
        let invalid_commit = create_test_commit(
            UNIX_EPOCH,
            "bob", 
            vec!["target/debug/main", "build/output"]
        );
        
        // Commit with included paths should pass
        let result1 = filter.apply(&valid_commit);
        assert!(matches!(result1, ControlFlow::Continue(())));
        
        // Commit without included paths should be filtered out
        let result2 = filter.apply(&invalid_commit);
        assert!(matches!(result2, ControlFlow::Break(())));
    }

    #[test]
    fn test_author_filter_creation() {
        let include_authors = vec!["alice", "bob"];
        let exclude_authors = vec!["bot", "automated"];
        
        let _include_filter = AuthorFilter::include_authors(include_authors);
        let _exclude_filter = AuthorFilter::exclude_authors(exclude_authors);
        
        let query_filter = QueryAuthorFilter {
            include: vec!["alice".to_string()],
            exclude: vec!["bot".to_string()],
        };
        let _combined_filter = AuthorFilter::new(query_filter);
        // Filters should be created successfully
    }

    #[test]
    fn test_author_filter_application() {
        let filter = AuthorFilter::include_authors(vec!["alice", "bob"]);
        
        let valid_commit = create_test_commit(UNIX_EPOCH, "alice", vec!["src/main.rs"]);
        let invalid_commit = create_test_commit(UNIX_EPOCH, "charlie", vec!["src/lib.rs"]);
        
        // Included author should pass
        let result1 = filter.apply(&valid_commit);
        assert!(matches!(result1, ControlFlow::Continue(())));
        
        // Non-included author should be filtered out
        let result2 = filter.apply(&invalid_commit);
        assert!(matches!(result2, ControlFlow::Break(())));
    }

    #[test]
    fn test_callback_filter() {
        let filter = callback_filter(|commit: &CommitData| {
            if commit.message.contains("fix") {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        });
        
        let fix_commit = CommitData {
            timestamp: UNIX_EPOCH,
            author: "alice".to_string(),
            file_paths: vec!["src/main.rs".to_string()],
            message: "fix: resolve bug in parser".to_string(),
        };
        let feature_commit = CommitData {
            timestamp: UNIX_EPOCH,
            author: "bob".to_string(),
            file_paths: vec!["src/lib.rs".to_string()],
            message: "feat: add new feature".to_string(),
        };
        
        // Fix commit should pass
        let result1 = filter.apply(&fix_commit);
        assert!(matches!(result1, ControlFlow::Continue(())));
        
        // Feature commit should be filtered out
        let result2 = filter.apply(&feature_commit);
        assert!(matches!(result2, ControlFlow::Break(())));
    }

    #[test]
    fn test_filter_chaining() {
        let date_filter = DateFilter::after(UNIX_EPOCH + Duration::from_secs(1000));
        let author_filter = AuthorFilter::include_authors(vec!["alice"]);
        
        let chained = date_filter.and_then(author_filter);
        
        let valid_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(1500),
            "alice",
            vec!["src/main.rs"]
        );
        let invalid_date_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(500),
            "alice", 
            vec!["src/main.rs"]
        );
        let invalid_author_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(1500),
            "bob",
            vec!["src/main.rs"]
        );
        
        // Valid commit should pass both filters
        let result1 = chained.apply(&valid_commit);
        assert!(matches!(result1, ControlFlow::Continue(())));
        
        // Invalid date should be filtered out
        let result2 = chained.apply(&invalid_date_commit);
        assert!(matches!(result2, ControlFlow::Break(())));
        
        // Invalid author should be filtered out
        let result3 = chained.apply(&invalid_author_commit);
        assert!(matches!(result3, ControlFlow::Break(())));
    }

    #[test]
    fn test_filter_executor() {
        let executor = FilterExecutor::new();
        let limited_executor = FilterExecutor::with_limit(2);
        
        let commits = vec![
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1000), "alice", vec!["src/a.rs"]),
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1100), "alice", vec!["src/b.rs"]),
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1200), "alice", vec!["src/c.rs"]),
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1300), "bob", vec!["src/d.rs"]),
        ];
        
        let author_filter = AuthorFilter::include_authors(vec!["alice"]);
        
        // Unlimited executor should return all matching commits
        let result1 = executor.apply_filter(commits.clone().into_iter(), author_filter.clone());
        assert_eq!(result1.len(), 3);
        
        // Limited executor should return only 2 commits (early termination)
        let result2 = limited_executor.apply_filter(commits.into_iter(), author_filter);
        assert_eq!(result2.len(), 2);
    }

    #[test]
    fn test_multiple_filters_application() {
        let executor = FilterExecutor::new();
        
        let commits = vec![
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1000), "alice", vec!["src/main.rs"]),
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1100), "alice", vec!["target/debug"]),
            create_test_commit(UNIX_EPOCH + Duration::from_secs(1200), "bob", vec!["src/lib.rs"]),
            create_test_commit(UNIX_EPOCH + Duration::from_secs(500), "alice", vec!["src/test.rs"]),
        ];
        
        let filters = vec![
            Box::new(DateFilter::after(UNIX_EPOCH + Duration::from_secs(900))) as Box<dyn ScanFilter<CommitData>>,
            Box::new(AuthorFilter::include_authors(vec!["alice"])),
            Box::new(PathFilter::include_paths(vec!["src/"])),
        ];
        
        let result = executor.apply_filters_boxed(commits.into_iter(), filters);
        assert_eq!(result.len(), 1); // Only one commit should pass all filters
    }

    #[test]
    fn test_filter_from_query_params() {
        use crate::scanner::query::QueryParams;
        
        let query = QueryParams::builder()
            .date_range(
                Some(UNIX_EPOCH + Duration::from_secs(1000)),
                Some(UNIX_EPOCH + Duration::from_secs(2000))
            )
            .include_author("alice")
            .include_path("src/")
            .build()
            .unwrap();
            
        let filter = FilterExecutor::filter_from_query(&query);
        
        let valid_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(1500),
            "alice",
            vec!["src/main.rs"]
        );
        let invalid_commit = create_test_commit(
            UNIX_EPOCH + Duration::from_secs(500),
            "bob", 
            vec!["target/debug"]
        );
        
        // Valid commit should pass
        let result1 = filter.apply(&valid_commit);
        assert!(matches!(result1, ControlFlow::Continue(())));
        
        // Invalid commit should be filtered out
        let result2 = filter.apply(&invalid_commit);
        assert!(matches!(result2, ControlFlow::Break(())));
    }

    #[test]
    fn test_zero_cost_abstractions() {
        // Test that closure-based filters compile to efficient code
        let predicate = |commit: &CommitData| {
            if commit.author.starts_with("a") && commit.file_paths.len() > 0 {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        };
        
        let filter = callback_filter(predicate);
        let commit = create_test_commit(UNIX_EPOCH, "alice", vec!["src/main.rs"]);
        
        let result = filter.apply(&commit);
        assert!(matches!(result, ControlFlow::Continue(())));
    }

    #[test] 
    fn test_early_termination_performance() {
        let executor = FilterExecutor::with_limit(1);
        
        // Create large dataset to test early termination
        let commits: Vec<CommitData> = (0..1000)
            .map(|i| create_test_commit(
                UNIX_EPOCH + Duration::from_secs(i), 
                "alice", 
                vec!["src/main.rs"]
            ))
            .collect();
            
        let filter = AuthorFilter::include_authors(vec!["alice"]);
        
        // Should terminate early after finding 1 match
        let result = executor.apply_filter(commits.into_iter(), filter);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_empty_filters_include_all() {
        use crate::scanner::query::QueryParams;
        
        // Create query with no filters (empty include/exclude lists)
        let empty_query = QueryParams::default();
        
        // Verify the query has empty include lists
        assert!(empty_query.file_paths.include.is_empty());
        assert!(empty_query.authors.include.is_empty());
        
        // Create filter from empty query
        let filter = FilterExecutor::filter_from_query(&empty_query);
        
        // Test commits with different authors and paths
        let commits = vec![
            create_test_commit(UNIX_EPOCH, "alice", vec!["src/main.rs"]),
            create_test_commit(UNIX_EPOCH, "bob", vec!["tests/test.rs"]),
            create_test_commit(UNIX_EPOCH, "charlie", vec!["docs/README.md"]),
        ];
        
        // All commits should pass through empty filters (include all behavior)
        for commit in &commits {
            let result = filter.apply(commit);
            assert!(matches!(result, ControlFlow::Continue(())), 
                "Empty filters should include all commits, but commit {:?} was filtered out", commit);
        }
    }
}