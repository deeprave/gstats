use crate::scanner::async_engine::events::{RepositoryEvent, FileInfo};
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata, ProcessorSharedData, SharedStateAccess};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use log::{debug, info};

/// Event processor for handling file system events
pub struct FileEventProcessor {
    file_patterns: Vec<String>,
    excluded_patterns: Vec<String>,
    file_count: usize,
    total_size: u64,
    processing_start_time: Option<Instant>,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl FileEventProcessor {
    /// Create a new file event processor
    pub fn new() -> Self {
        Self {
            file_patterns: Vec::new(),
            excluded_patterns: Vec::new(),
            file_count: 0,
            total_size: 0,
            processing_start_time: None,
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }


    /// Create a new file event processor with custom patterns
    pub fn with_patterns(file_patterns: Vec<String>, excluded_patterns: Vec<String>) -> Self {
        Self {
            file_patterns,
            excluded_patterns,
            file_count: 0,
            total_size: 0,
            processing_start_time: None,
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    /// Convert FileInfo to ScanMessage
    fn create_file_message(&self, file_info: &FileInfo) -> ScanMessage {
        let header = MessageHeader::new(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "files-processor".to_string(),
        );

        let file_data = MessageData::FileInfo {
            path: file_info.relative_path.clone(),
            size: file_info.size,
            lines: file_info.line_count.unwrap_or(0) as u32,
        };

        ScanMessage::new(header, file_data)
    }

    /// Check if file should be included based on patterns
    fn should_include_file(&self, file_info: &FileInfo) -> bool {
        let file_path = &file_info.relative_path;

        // Check exclusion patterns first
        if !self.excluded_patterns.is_empty() {
            for pattern in &self.excluded_patterns {
                if file_path.contains(pattern) {
                    debug!("File '{file_path}' excluded by pattern '{pattern}'");
                    return false;
                }
            }
        }

        // Check inclusion patterns
        if !self.file_patterns.is_empty() {
            let included = self.file_patterns.iter().any(|pattern| {
                file_path.contains(pattern)
            });
            if !included {
                debug!("File '{file_path}' not included by any pattern");
                return false;
            }
        }

        // Additional filtering based on file properties
        if file_info.is_binary && self.should_exclude_binary_files() {
            debug!("Binary file '{file_path}' excluded");
            return false;
        }

        true
    }

    /// Check if binary files should be excluded
    fn should_exclude_binary_files(&self) -> bool {
        // For now, include binary files by default
        // This could be made configurable in the future
        false
    }


    /// Check if file matches common source code extensions
    fn is_source_code_file(&self, file_info: &FileInfo) -> bool {
        if let Some(ext) = &file_info.extension {
            matches!(ext.as_str(), 
                "rs" | "py" | "js" | "ts" | "java" | "cpp" | "c" | "h" | 
                "go" | "rb" | "php" | "cs" | "swift" | "kt" | "scala" |
                "html" | "css" | "scss" | "less" | "vue" | "jsx" | "tsx"
            )
        } else {
            false
        }
    }

    /// Estimate file complexity based on size and line count
    fn estimate_file_complexity(&self, file_info: &FileInfo) -> f64 {
        let size_factor = (file_info.size as f64 / 1024.0).min(10.0) / 10.0; // Normalize to 0-1
        let line_factor = if let Some(lines) = file_info.line_count {
            (lines as f64 / 100.0).min(10.0) / 10.0 // Normalize to 0-1
        } else {
            0.5 // Default for binary files
        };
        
        (size_factor * 0.3 + line_factor * 0.7) * 100.0 // Weighted score out of 100
    }
}

#[async_trait]
impl EventProcessor for FileEventProcessor {
    fn name(&self) -> &'static str {
        "files"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        self.processing_start_time = Some(Instant::now());
        debug!("Initialized FileEventProcessor with {} include patterns and {} exclude patterns", 
               self.file_patterns.len(), self.excluded_patterns.len());
        Ok(())
    }

    async fn on_repository_metadata(&mut self, metadata: &RepositoryMetadata) -> PluginResult<()> {
        debug!(
            "FileEventProcessor received repository metadata: {} files expected",
            metadata.total_files.unwrap_or(0)
        );
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let start_time = Instant::now();
        let mut messages = Vec::new();

        match event {
            RepositoryEvent::FileScanned { file_info } => {
                if self.should_include_file(file_info) {
                    // Cache file in shared state if available
                    if let Some(shared_state) = &self.shared_state {
                        if let Err(e) = shared_state.cache_file(file_info.clone()) {
                            debug!("Failed to cache file in shared state: {e}");
                        }

                        // Share file complexity data with other processors if it's a source code file
                        if self.is_source_code_file(file_info) {
                            let complexity_data = ProcessorSharedData::FileComplexity {
                                file_path: file_info.relative_path.clone(),
                                complexity_score: self.estimate_file_complexity(file_info),
                                lines_of_code: file_info.line_count.unwrap_or(0),
                                cyclomatic_complexity: 1, // Basic estimate
                            };

                            let key = format!("file_complexity_{}", file_info.relative_path);
                            if let Err(e) = shared_state.share_processor_data(key, complexity_data) {
                                debug!("Failed to share file complexity data: {e}");
                            }
                        }
                    }

                    let message = self.create_file_message(file_info);
                    messages.push(message);
                    self.file_count += 1;
                    self.total_size += file_info.size;
                    
                    debug!("Processed file '{}' ({} bytes)", 
                           file_info.relative_path, file_info.size);
                }
            }
            RepositoryEvent::RepositoryStarted { total_files: Some(total), .. } => {
                info!("Starting file processing for {total} files");
            }
            RepositoryEvent::RepositoryStarted { total_files: None, .. } => {
                // No total count available
            }
            RepositoryEvent::RepositoryCompleted { stats } => {
                info!(
                    "File processing completed: {} files processed ({} bytes total) from {} total files",
                    self.file_count, self.total_size, stats.total_files
                );
            }
            _ => {
                // Ignore other event types
            }
        }

        // Update statistics
        self.stats.events_processed += 1;
        self.stats.messages_generated += messages.len();
        self.stats.processing_time += start_time.elapsed();

        Ok(messages)
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        if let Some(start_time) = self.processing_start_time {
            let total_duration = start_time.elapsed();
            info!(
                "FileEventProcessor finalized: {} files processed ({} bytes) in {:?}",
                self.file_count, self.total_size, total_duration
            );
        }

        // No additional messages to generate during finalization
        Ok(vec![])
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl SharedStateAccess for FileEventProcessor {
    fn shared_state(&self) -> &SharedProcessorState {
        self.shared_state.as_ref()
            .expect("SharedProcessorState not initialized")
    }
}

impl Default for FileEventProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::RepositoryStats;
    use std::path::PathBuf;
    use std::time::Duration;

    fn create_test_file_info(relative_path: &str, size: u64, is_binary: bool) -> FileInfo {
        let path = PathBuf::from(relative_path);
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_string());

        FileInfo {
            path,
            relative_path: relative_path.to_string(),
            size,
            extension,
            is_binary,
            line_count: if is_binary { None } else { Some(50) },
            last_modified: Some(SystemTime::now()),
        }
    }

    #[tokio::test]
    async fn test_file_processor_creation() {
        let processor = FileEventProcessor::new();
        assert_eq!(processor.name(), "files");
        // FileEventProcessor processes file-related events
        assert_eq!(processor.file_count, 0);
        assert_eq!(processor.total_size, 0);
    }

    #[tokio::test]
    async fn test_file_processor_with_patterns() {
        let include_patterns = vec!["*.rs".to_string(), "*.py".to_string()];
        let exclude_patterns = vec!["target/".to_string()];
        
        let processor = FileEventProcessor::with_patterns(include_patterns.clone(), exclude_patterns.clone());
        assert_eq!(processor.file_patterns, include_patterns);
        assert_eq!(processor.excluded_patterns, exclude_patterns);
    }

    #[tokio::test]
    async fn test_file_processing() {
        let mut processor = FileEventProcessor::new();
        processor.initialize().await.unwrap();

        let file_info = create_test_file_info("src/main.rs", 1024, false);
        let event = RepositoryEvent::FileScanned {
            file_info: file_info.clone(),
        };

        let messages = processor.process_event(&event).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(processor.file_count, 1);
        assert_eq!(processor.total_size, 1024);

        // Verify message content
        let message = &messages[0];
        match &message.data {
            MessageData::FileInfo { path, size, .. } => {
                assert_eq!(path, &file_info.relative_path);
                assert_eq!(*size, file_info.size);
            }
            _ => panic!("Expected file message data"),
        }
    }

    #[tokio::test]
    async fn test_file_filtering_by_patterns() {
        let include_patterns = vec!["src/".to_string()];
        let exclude_patterns = vec!["target/".to_string()];
        
        let mut processor = FileEventProcessor::with_patterns(include_patterns, exclude_patterns);
        processor.initialize().await.unwrap();

        // Test included file
        let included_file = create_test_file_info("src/main.rs", 1024, false);
        let event1 = RepositoryEvent::FileScanned {
            file_info: included_file,
        };
        let messages1 = processor.process_event(&event1).await.unwrap();
        assert_eq!(messages1.len(), 1);

        // Test excluded file
        let excluded_file = create_test_file_info("target/debug/main", 2048, true);
        let event2 = RepositoryEvent::FileScanned {
            file_info: excluded_file,
        };
        let messages2 = processor.process_event(&event2).await.unwrap();
        assert_eq!(messages2.len(), 0);

        // Test file not matching include pattern
        let non_matching_file = create_test_file_info("docs/readme.md", 512, false);
        let event3 = RepositoryEvent::FileScanned {
            file_info: non_matching_file,
        };
        let messages3 = processor.process_event(&event3).await.unwrap();
        assert_eq!(messages3.len(), 0);

        assert_eq!(processor.file_count, 1); // Only one file should be counted
        assert_eq!(processor.total_size, 1024);
    }

    #[tokio::test]
    async fn test_multiple_files() {
        let mut processor = FileEventProcessor::new();
        processor.initialize().await.unwrap();

        let files = [
            ("src/main.rs", 1024, false),
            ("src/lib.rs", 2048, false),
            ("README.md", 512, false),
        ];

        for (path, size, is_binary) in &files {
            let file_info = create_test_file_info(path, *size, *is_binary);
            let event = RepositoryEvent::FileScanned { file_info };
            processor.process_event(&event).await.unwrap();
        }

        assert_eq!(processor.file_count, 3);
        assert_eq!(processor.total_size, 1024 + 2048 + 512);
    }

    #[tokio::test]
    async fn test_source_code_file_detection() {
        let processor = FileEventProcessor::new();

        let rust_file = create_test_file_info("main.rs", 1024, false);
        assert!(processor.is_source_code_file(&rust_file));

        let python_file = create_test_file_info("script.py", 512, false);
        assert!(processor.is_source_code_file(&python_file));

        let text_file = create_test_file_info("readme.txt", 256, false);
        assert!(!processor.is_source_code_file(&text_file));

        let no_extension_file = create_test_file_info("Makefile", 128, false);
        assert!(!processor.is_source_code_file(&no_extension_file));
    }

    #[tokio::test]
    async fn test_repository_lifecycle_events() {
        let mut processor = FileEventProcessor::new();
        processor.initialize().await.unwrap();

        // Test repository started event
        let start_event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(100),
            total_files: Some(50),
        };
        let messages = processor.process_event(&start_event).await.unwrap();
        assert_eq!(messages.len(), 0);

        // Test repository completed event
        let stats = RepositoryStats {
            total_commits: 100,
            total_files: 50,
            total_changes: 200,
            scan_duration: Duration::from_secs(5),
            events_emitted: 150,
        };
        let complete_event = RepositoryEvent::RepositoryCompleted { stats };
        let messages = processor.process_event(&complete_event).await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn test_processor_statistics() {
        let mut processor = FileEventProcessor::new();
        processor.initialize().await.unwrap();

        let file_info = create_test_file_info("test.rs", 1024, false);
        let event = RepositoryEvent::FileScanned { file_info };

        processor.process_event(&event).await.unwrap();

        let stats = processor.get_stats();
        assert_eq!(stats.events_processed, 1);
        assert_eq!(stats.messages_generated, 1);
        assert!(stats.processing_time > Duration::from_nanos(0));
    }

    #[tokio::test]
    async fn test_finalization() {
        let mut processor = FileEventProcessor::new();
        processor.initialize().await.unwrap();

        // Process some files
        let file_info = create_test_file_info("test.rs", 1024, false);
        let event = RepositoryEvent::FileScanned { file_info };
        processor.process_event(&event).await.unwrap();

        // Finalize
        let final_messages = processor.finalize().await.unwrap();
        assert_eq!(final_messages.len(), 0); // No additional messages during finalization
    }
}
