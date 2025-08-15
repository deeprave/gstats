//! Plugin Data Coordinator
//! 
//! Manages collection of data from multiple plugins for export.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use crate::plugin::data_export::PluginDataExport;

/// Coordinates data collection from multiple plugins
#[derive(Debug, Clone)]
pub struct DataCoordinator {
    /// Data collected from plugins, keyed by plugin ID
    pending_data: HashMap<String, Vec<Arc<PluginDataExport>>>,
    
    /// Current scan ID being processed
    scan_id: Option<String>,
    
    /// Set of plugin IDs expected to provide data
    expected_plugins: HashSet<String>,
    
    /// Set of plugins that have provided data
    received_plugins: HashSet<String>,
}

impl DataCoordinator {
    /// Create a new data coordinator
    pub fn new() -> Self {
        Self {
            pending_data: HashMap::new(),
            scan_id: None,
            expected_plugins: HashSet::new(),
            received_plugins: HashSet::new(),
        }
    }
    
    /// Create with expected plugins
    pub fn with_expected_plugins(plugins: Vec<String>) -> Self {
        Self {
            pending_data: HashMap::new(),
            scan_id: None,
            expected_plugins: plugins.into_iter().collect(),
            received_plugins: HashSet::new(),
        }
    }
    
    /// Set the current scan ID
    pub fn set_scan_id(&mut self, scan_id: String) {
        self.scan_id = Some(scan_id);
    }
    
    /// Set expected plugins
    pub fn set_expected_plugins(&mut self, plugins: Vec<String>) {
        self.expected_plugins = plugins.into_iter().collect();
    }
    
    /// Add data from a plugin
    pub fn add_data(&mut self, plugin_id: String, data: Arc<PluginDataExport>) {
        self.pending_data
            .entry(plugin_id.clone())
            .or_insert_with(Vec::new)
            .push(data);
        self.received_plugins.insert(plugin_id);
    }
    
    /// Check if all expected plugins have provided data
    pub fn is_complete(&self) -> bool {
        if self.expected_plugins.is_empty() {
            // If no plugins expected, consider complete when we have any data
            !self.pending_data.is_empty()
        } else {
            // Check if all expected plugins have reported
            self.expected_plugins.is_subset(&self.received_plugins)
        }
    }
    
    /// Get the number of plugins that have provided data
    pub fn received_count(&self) -> usize {
        self.received_plugins.len()
    }
    
    /// Get the number of expected plugins
    pub fn expected_count(&self) -> usize {
        self.expected_plugins.len()
    }
    
    /// Check if a specific plugin has provided data
    pub fn has_data_from(&self, plugin_id: &str) -> bool {
        self.received_plugins.contains(plugin_id)
    }
    
    /// Get all collected data
    pub fn get_all_data(&self) -> Vec<Arc<PluginDataExport>> {
        self.pending_data
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect()
    }
    
    /// Get data from a specific plugin
    pub fn get_plugin_data(&self, plugin_id: &str) -> Option<&Vec<Arc<PluginDataExport>>> {
        self.pending_data.get(plugin_id)
    }
    
    /// Clear all data for a new scan
    pub fn clear(&mut self) {
        self.pending_data.clear();
        self.received_plugins.clear();
        self.scan_id = None;
    }
    
    /// Get the current scan ID
    pub fn scan_id(&self) -> Option<&String> {
        self.scan_id.as_ref()
    }
    
    /// Get a list of plugins that haven't reported yet
    pub fn get_pending_plugins(&self) -> Vec<String> {
        self.expected_plugins
            .difference(&self.received_plugins)
            .cloned()
            .collect()
    }
}

impl Default for DataCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::data_export::{
        PluginDataExport, DataExportType
    };
    
    #[test]
    fn test_data_coordinator_new() {
        let coordinator = DataCoordinator::new();
        assert_eq!(coordinator.received_count(), 0);
        assert_eq!(coordinator.expected_count(), 0);
        assert!(!coordinator.is_complete());
    }
    
    #[test]
    fn test_data_coordinator_with_expected() {
        let coordinator = DataCoordinator::with_expected_plugins(vec![
            "plugin1".to_string(),
            "plugin2".to_string(),
        ]);
        assert_eq!(coordinator.expected_count(), 2);
        assert!(!coordinator.is_complete());
    }
    
    #[test]
    fn test_add_data() {
        let mut coordinator = DataCoordinator::with_expected_plugins(vec![
            "plugin1".to_string(),
        ]);
        
        let export = Arc::new(
            PluginDataExport::builder()
                .plugin_id("plugin1")
                .title("Test Data")
                .data_type(DataExportType::Tabular)
                .build()
                .unwrap()
        );
        
        coordinator.add_data("plugin1".to_string(), export);
        assert!(coordinator.has_data_from("plugin1"));
        assert!(coordinator.is_complete());
    }
    
    #[test]
    fn test_pending_plugins() {
        let mut coordinator = DataCoordinator::with_expected_plugins(vec![
            "plugin1".to_string(),
            "plugin2".to_string(),
            "plugin3".to_string(),
        ]);
        
        let export = Arc::new(
            PluginDataExport::builder()
                .plugin_id("plugin1")
                .title("Test Data")
                .build()
                .unwrap()
        );
        
        coordinator.add_data("plugin1".to_string(), export);
        
        let pending = coordinator.get_pending_plugins();
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&"plugin2".to_string()));
        assert!(pending.contains(&"plugin3".to_string()));
    }
}