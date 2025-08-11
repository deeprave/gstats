//! Plugin Processors Module
//! 
//! Contains comprehensive event processors that can be used by plugins
//! for advanced repository analysis. These processors implement the
//! EventProcessor trait and provide reusable analysis capabilities.

pub mod change_frequency;
pub mod complexity;
pub mod hotspot;
pub mod debt_assessment;
pub mod format_detection;
pub mod duplication_detector;

// Re-export processors for easier access
pub use change_frequency::ChangeFrequencyProcessor;
pub use complexity::ComplexityProcessor;
pub use hotspot::HotspotProcessor;
pub use debt_assessment::DebtAssessmentProcessor;
pub use format_detection::FormatDetectionProcessor;
pub use duplication_detector::DuplicationDetectorProcessor;
