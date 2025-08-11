//! Format Detection Processor
//! 
//! Event-driven processor that detects and analyzes file formats
//! by examining file extensions, content patterns, and metadata.
//! This processor can be used by any plugin that needs format analysis.

use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::scanner::modes::ScanMode;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;
use serde::{Serialize, Deserialize};

/// File format information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFormat {
    pub file_path: String,
    pub extension: String,
    pub format_type: FormatType,
    pub language: Option<String>,
    pub is_binary: bool,
    pub is_generated: bool,
    pub confidence: f64, // 0.0 to 1.0
    pub detected_patterns: Vec<String>,
}

impl FileFormat {
    pub fn new(file_path: String) -> Self {
        let extension = std::path::Path::new(&file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (format_type, language, is_binary) = Self::classify_by_extension(&extension);
        let is_generated = Self::detect_generated_file(&file_path);

        Self {
            file_path,
            extension,
            format_type,
            language,
            is_binary,
            is_generated,
            confidence: 1.0, // High confidence for extension-based detection
            detected_patterns: vec![],
        }
    }

    fn classify_by_extension(extension: &str) -> (FormatType, Option<String>, bool) {
        match extension {
            // Programming languages
            "rs" => (FormatType::SourceCode, Some("Rust".to_string()), false),
            "py" => (FormatType::SourceCode, Some("Python".to_string()), false),
            "js" => (FormatType::SourceCode, Some("JavaScript".to_string()), false),
            "ts" => (FormatType::SourceCode, Some("TypeScript".to_string()), false),
            "java" => (FormatType::SourceCode, Some("Java".to_string()), false),
            "c" => (FormatType::SourceCode, Some("C".to_string()), false),
            "cpp" | "cc" | "cxx" => (FormatType::SourceCode, Some("C++".to_string()), false),
            "h" | "hpp" => (FormatType::SourceCode, Some("C/C++ Header".to_string()), false),
            "cs" => (FormatType::SourceCode, Some("C#".to_string()), false),
            "go" => (FormatType::SourceCode, Some("Go".to_string()), false),
            "rb" => (FormatType::SourceCode, Some("Ruby".to_string()), false),
            "php" => (FormatType::SourceCode, Some("PHP".to_string()), false),
            "swift" => (FormatType::SourceCode, Some("Swift".to_string()), false),
            "kt" => (FormatType::SourceCode, Some("Kotlin".to_string()), false),
            "scala" => (FormatType::SourceCode, Some("Scala".to_string()), false),
            "clj" => (FormatType::SourceCode, Some("Clojure".to_string()), false),
            "hs" => (FormatType::SourceCode, Some("Haskell".to_string()), false),
            "ml" => (FormatType::SourceCode, Some("OCaml".to_string()), false),
            "fs" => (FormatType::SourceCode, Some("F#".to_string()), false),
            "dart" => (FormatType::SourceCode, Some("Dart".to_string()), false),
            "lua" => (FormatType::SourceCode, Some("Lua".to_string()), false),
            "r" => (FormatType::SourceCode, Some("R".to_string()), false),
            "m" => (FormatType::SourceCode, Some("Objective-C".to_string()), false),
            "mm" => (FormatType::SourceCode, Some("Objective-C++".to_string()), false),

            // Web technologies
            "html" | "htm" => (FormatType::Markup, Some("HTML".to_string()), false),
            "css" => (FormatType::Stylesheet, Some("CSS".to_string()), false),
            "scss" | "sass" => (FormatType::Stylesheet, Some("Sass".to_string()), false),
            "less" => (FormatType::Stylesheet, Some("Less".to_string()), false),
            "jsx" => (FormatType::SourceCode, Some("React JSX".to_string()), false),
            "tsx" => (FormatType::SourceCode, Some("React TSX".to_string()), false),
            "vue" => (FormatType::SourceCode, Some("Vue.js".to_string()), false),
            "svelte" => (FormatType::SourceCode, Some("Svelte".to_string()), false),

            // Scripts
            "sh" | "bash" => (FormatType::Script, Some("Bash".to_string()), false),
            "zsh" => (FormatType::Script, Some("Zsh".to_string()), false),
            "fish" => (FormatType::Script, Some("Fish".to_string()), false),
            "ps1" => (FormatType::Script, Some("PowerShell".to_string()), false),
            "bat" | "cmd" => (FormatType::Script, Some("Batch".to_string()), false),

            // Configuration
            "json" => (FormatType::Configuration, Some("JSON".to_string()), false),
            "yaml" | "yml" => (FormatType::Configuration, Some("YAML".to_string()), false),
            "toml" => (FormatType::Configuration, Some("TOML".to_string()), false),
            "xml" => (FormatType::Configuration, Some("XML".to_string()), false),
            "ini" => (FormatType::Configuration, Some("INI".to_string()), false),
            "cfg" | "conf" => (FormatType::Configuration, Some("Config".to_string()), false),
            "properties" => (FormatType::Configuration, Some("Properties".to_string()), false),

            // Documentation
            "md" => (FormatType::Documentation, Some("Markdown".to_string()), false),
            "rst" => (FormatType::Documentation, Some("reStructuredText".to_string()), false),
            "txt" => (FormatType::Documentation, Some("Plain Text".to_string()), false),
            "adoc" => (FormatType::Documentation, Some("AsciiDoc".to_string()), false),

            // Database
            "sql" => (FormatType::Database, Some("SQL".to_string()), false),
            "plsql" => (FormatType::Database, Some("PL/SQL".to_string()), false),
            "psql" => (FormatType::Database, Some("PostgreSQL".to_string()), false),

            // Build files
            "dockerfile" => (FormatType::Build, Some("Docker".to_string()), false),
            "makefile" => (FormatType::Build, Some("Make".to_string()), false),
            "cmake" => (FormatType::Build, Some("CMake".to_string()), false),
            "gradle" => (FormatType::Build, Some("Gradle".to_string()), false),
            "sbt" => (FormatType::Build, Some("SBT".to_string()), false),

            // Binary files
            "exe" | "dll" | "so" | "dylib" => (FormatType::Binary, None, true),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" => (FormatType::Binary, Some("Image".to_string()), true),
            "pdf" => (FormatType::Binary, Some("PDF".to_string()), true),
            "zip" | "tar" | "gz" | "bz2" | "xz" => (FormatType::Binary, Some("Archive".to_string()), true),

            // Default
            _ => (FormatType::Unknown, None, false),
        }
    }

    fn detect_generated_file(file_path: &str) -> bool {
        let path_lower = file_path.to_lowercase();
        
        // Common patterns for generated files
        path_lower.contains("generated") ||
        path_lower.contains("gen/") ||
        path_lower.contains("build/") ||
        path_lower.contains("target/") ||
        path_lower.contains("dist/") ||
        path_lower.contains("out/") ||
        path_lower.contains("node_modules/") ||
        path_lower.contains(".min.") ||
        path_lower.ends_with(".lock") ||
        path_lower.ends_with(".cache")
    }

    /// Analyze content patterns (would be implemented with actual file content)
    pub fn analyze_content_patterns(&mut self, _content: &str) {
        // In a full implementation, this would analyze file content for:
        // - Shebang lines
        // - Magic numbers
        // - Content structure patterns
        // - Language-specific patterns
        
        // For now, just add some example patterns
        if self.language.as_ref().map_or(false, |l| l == "Python") {
            self.detected_patterns.push("python_imports".to_string());
        }
        if self.language.as_ref().map_or(false, |l| l == "Rust") {
            self.detected_patterns.push("rust_use_statements".to_string());
        }
    }

    /// Get format category for grouping
    pub fn get_category(&self) -> FormatCategory {
        match self.format_type {
            FormatType::SourceCode => FormatCategory::Code,
            FormatType::Script => FormatCategory::Code,
            FormatType::Markup => FormatCategory::Web,
            FormatType::Stylesheet => FormatCategory::Web,
            FormatType::Configuration => FormatCategory::Config,
            FormatType::Documentation => FormatCategory::Docs,
            FormatType::Database => FormatCategory::Data,
            FormatType::Build => FormatCategory::Build,
            FormatType::Binary => FormatCategory::Binary,
            FormatType::Unknown => FormatCategory::Other,
        }
    }
}

/// File format type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatType {
    SourceCode,
    Script,
    Markup,
    Stylesheet,
    Configuration,
    Documentation,
    Database,
    Build,
    Binary,
    Unknown,
}

/// Format category for high-level grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatCategory {
    Code,
    Web,
    Config,
    Docs,
    Data,
    Build,
    Binary,
    Other,
}

/// Format statistics for the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatStatistics {
    pub total_files: usize,
    pub format_distribution: HashMap<String, usize>,
    pub language_distribution: HashMap<String, usize>,
    pub category_distribution: HashMap<String, usize>,
    pub binary_files: usize,
    pub generated_files: usize,
    pub source_code_files: usize,
}

impl FormatStatistics {
    pub fn new() -> Self {
        Self {
            total_files: 0,
            format_distribution: HashMap::new(),
            language_distribution: HashMap::new(),
            category_distribution: HashMap::new(),
            binary_files: 0,
            generated_files: 0,
            source_code_files: 0,
        }
    }

    pub fn add_file_format(&mut self, format: &FileFormat) {
        self.total_files += 1;

        // Update format distribution
        *self.format_distribution.entry(format.extension.clone()).or_insert(0) += 1;

        // Update language distribution
        if let Some(language) = &format.language {
            *self.language_distribution.entry(language.clone()).or_insert(0) += 1;
        }

        // Update category distribution
        let category = format.get_category();
        *self.category_distribution.entry(format!("{:?}", category)).or_insert(0) += 1;

        // Update counters
        if format.is_binary {
            self.binary_files += 1;
        }
        if format.is_generated {
            self.generated_files += 1;
        }
        if matches!(format.format_type, FormatType::SourceCode | FormatType::Script) {
            self.source_code_files += 1;
        }
    }

    /// Get the most common file formats
    pub fn get_top_formats(&self, limit: usize) -> Vec<(String, usize)> {
        let mut formats: Vec<(String, usize)> = self.format_distribution.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        formats.sort_by(|a, b| b.1.cmp(&a.1));
        formats.into_iter().take(limit).collect()
    }

    /// Get the most common languages
    pub fn get_top_languages(&self, limit: usize) -> Vec<(String, usize)> {
        let mut languages: Vec<(String, usize)> = self.language_distribution.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        languages.sort_by(|a, b| b.1.cmp(&a.1));
        languages.into_iter().take(limit).collect()
    }
}

/// Format Detection Processor - can be used by any plugin
pub struct FormatDetectionProcessor {
    file_formats: HashMap<String, FileFormat>,
    statistics: FormatStatistics,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl FormatDetectionProcessor {
    pub fn new() -> Self {
        Self {
            file_formats: HashMap::new(),
            statistics: FormatStatistics::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    fn detect_file_format(&self, file_path: &str) -> FileFormat {
        FileFormat::new(file_path.to_string())
    }

    /// Get the collected file formats (for use by other processors)
    pub fn get_file_formats(&self) -> &HashMap<String, FileFormat> {
        &self.file_formats
    }

    /// Get format statistics
    pub fn get_statistics(&self) -> &FormatStatistics {
        &self.statistics
    }

    fn create_format_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();
        
        // Create a summary message with format statistics
        let header = MessageHeader::new(
            ScanMode::FILES,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        let data = MessageData::FileInfo {
            path: "format_summary".to_string(),
            size: self.statistics.total_files as u64,
            lines: self.statistics.source_code_files as u32,
        };

        messages.push(ScanMessage::new(header, data));
        
        messages
    }
}

#[async_trait]
impl EventProcessor for FormatDetectionProcessor {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::FILES
    }

    fn name(&self) -> &'static str {
        "format_detection"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initialized FormatDetectionProcessor");
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        match event {
            RepositoryEvent::FileChanged { file_path, .. } => {
                let format = self.detect_file_format(file_path);
                self.statistics.add_file_format(&format);
                self.file_formats.insert(file_path.clone(), format);
            }
            _ => {}
        }
        self.stats.events_processed += 1;
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let messages = self.create_format_messages();
        self.stats.messages_generated = messages.len();
        
        debug!(
            "FormatDetectionProcessor finalized: {} files analyzed, {} formats detected",
            self.statistics.total_files,
            self.statistics.format_distribution.len()
        );
        
        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for FormatDetectionProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_format_detection_processor_creation() {
        let processor = FormatDetectionProcessor::new();
        assert_eq!(processor.name(), "format_detection");
        assert_eq!(processor.supported_modes(), ScanMode::FILES);
        assert!(processor.file_formats.is_empty());
        assert_eq!(processor.statistics.total_files, 0);
    }

    #[tokio::test]
    async fn test_file_format_detection() {
        let processor = FormatDetectionProcessor::new();
        
        let rust_format = processor.detect_file_format("src/main.rs");
        assert_eq!(rust_format.extension, "rs");
        assert_eq!(rust_format.format_type, FormatType::SourceCode);
        assert_eq!(rust_format.language, Some("Rust".to_string()));
        assert!(!rust_format.is_binary);

        let python_format = processor.detect_file_format("script.py");
        assert_eq!(python_format.extension, "py");
        assert_eq!(python_format.language, Some("Python".to_string()));

        let binary_format = processor.detect_file_format("image.png");
        assert_eq!(binary_format.extension, "png");
        assert_eq!(binary_format.format_type, FormatType::Binary);
        assert!(binary_format.is_binary);
    }

    #[tokio::test]
    async fn test_generated_file_detection() {
        let processor = FormatDetectionProcessor::new();
        
        let generated = processor.detect_file_format("target/debug/main.rs");
        assert!(generated.is_generated);

        let normal = processor.detect_file_format("src/main.rs");
        assert!(!normal.is_generated);
    }

    #[tokio::test]
    async fn test_format_statistics() {
        let mut stats = FormatStatistics::new();
        
        let rust_format = FileFormat::new("main.rs".to_string());
        let python_format = FileFormat::new("script.py".to_string());
        
        stats.add_file_format(&rust_format);
        stats.add_file_format(&python_format);
        
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.source_code_files, 2);
        assert_eq!(stats.binary_files, 0);
        
        let top_formats = stats.get_top_formats(5);
        assert_eq!(top_formats.len(), 2);
        
        let top_languages = stats.get_top_languages(5);
        assert_eq!(top_languages.len(), 2);
    }

    #[tokio::test]
    async fn test_file_processing() {
        let mut processor = FormatDetectionProcessor::new();
        processor.initialize().await.unwrap();

        let event = RepositoryEvent::FileChanged {
            file_path: "src/main.rs".to_string(),
            change_data: crate::scanner::async_engine::events::FileChangeData {
                change_type: crate::scanner::async_engine::events::ChangeType::Modified,
                old_path: Some("src/main.rs".to_string()),
                new_path: "src/main.rs".to_string(),
                insertions: 10,
                deletions: 2,
                is_binary: false,
            },
            commit_context: crate::scanner::async_engine::events::CommitInfo {
                hash: "abc123".to_string(),
                short_hash: "abc123".to_string(),
                author_name: "Test Author".to_string(),
                author_email: "test@example.com".to_string(),
                committer_name: "Test Author".to_string(),
                committer_email: "test@example.com".to_string(),
                timestamp: SystemTime::now(),
                message: "Test commit".to_string(),
                parent_hashes: vec![],
                changed_files: vec![],
                insertions: 0,
                deletions: 0,
            },
        };

        let messages = processor.process_event(&event).await.unwrap();
        assert!(messages.is_empty()); // No messages during processing

        assert_eq!(processor.file_formats.len(), 1);
        assert!(processor.file_formats.contains_key("src/main.rs"));
        assert_eq!(processor.statistics.total_files, 1);
    }

    #[tokio::test]
    async fn test_format_categories() {
        let rust_format = FileFormat::new("main.rs".to_string());
        assert_eq!(rust_format.get_category(), FormatCategory::Code);

        let html_format = FileFormat::new("index.html".to_string());
        assert_eq!(html_format.get_category(), FormatCategory::Web);

        let config_format = FileFormat::new("config.json".to_string());
        assert_eq!(config_format.get_category(), FormatCategory::Config);

        let doc_format = FileFormat::new("README.md".to_string());
        assert_eq!(doc_format.get_category(), FormatCategory::Docs);
    }
}
