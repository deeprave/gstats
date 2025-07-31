//! Message Listener and Registry System
//! 
//! Observer pattern implementation for message distribution

use std::sync::Arc;
use std::collections::HashMap;
use crate::scanner::messages::ScanMessage;
use crate::scanner::modes::ScanMode;
use crate::queue::QueueError;

/// Trait for message listeners (observer pattern)
pub trait MessageListener: Send + Sync {
    /// Get the scan modes this listener is interested in
    fn interested_modes(&self) -> ScanMode;
    
    /// Handle a message that matches the listener's interests
    fn on_message(&self, message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Get listener identifier for registration/unregistration
    fn listener_id(&self) -> String;
    
    /// Optional: Get listener name for debugging/monitoring
    fn listener_name(&self) -> String {
        self.listener_id()
    }
}

/// Registry for managing message listeners
pub trait ListenerRegistry: Send + Sync {
    /// Register a listener with the registry
    fn register_listener(&mut self, listener: Arc<dyn MessageListener>) -> Result<(), QueueError>;
    
    /// Unregister a listener by ID
    fn unregister_listener(&mut self, listener_id: &str) -> Result<(), QueueError>;
    
    /// Get all listeners interested in a specific scan mode
    fn get_interested_listeners(&self, scan_mode: ScanMode) -> Vec<Arc<dyn MessageListener>>;
    
    /// Get total number of registered listeners
    fn listener_count(&self) -> usize;
}

/// Default implementation of ListenerRegistry
pub struct DefaultListenerRegistry {
    listeners: HashMap<String, Arc<dyn MessageListener>>,
}

impl DefaultListenerRegistry {
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
        }
    }
}

impl Default for DefaultListenerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ListenerRegistry for DefaultListenerRegistry {
    fn register_listener(&mut self, listener: Arc<dyn MessageListener>) -> Result<(), QueueError> {
        let id = listener.listener_id();
        if self.listeners.contains_key(&id) {
            return Err(QueueError::ListenerError(format!("Listener with ID '{}' already registered", id)));
        }
        self.listeners.insert(id, listener);
        Ok(())
    }
    
    fn unregister_listener(&mut self, listener_id: &str) -> Result<(), QueueError> {
        if self.listeners.remove(listener_id).is_none() {
            return Err(QueueError::ListenerError(format!("Listener with ID '{}' not found", listener_id)));
        }
        Ok(())
    }
    
    fn get_interested_listeners(&self, scan_mode: ScanMode) -> Vec<Arc<dyn MessageListener>> {
        self.listeners
            .values()
            .filter(|listener| listener.interested_modes().intersects(scan_mode))
            .cloned()
            .collect()
    }
    
    fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};

    // Mock listener for testing
    struct MockListener {
        id: String,
        interested_modes: ScanMode,
        received_messages: std::sync::Mutex<Vec<ScanMessage>>,
    }

    impl MockListener {
        fn new(id: &str, modes: ScanMode) -> Self {
            Self {
                id: id.to_string(),
                interested_modes: modes,
                received_messages: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn received_count(&self) -> usize {
            self.received_messages.lock().unwrap().len()
        }
    }

    impl MessageListener for MockListener {
        fn interested_modes(&self) -> ScanMode {
            self.interested_modes
        }
        
        fn on_message(&self, message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
            self.received_messages.lock().unwrap().push(message.clone());
            Ok(())
        }
        
        fn listener_id(&self) -> String {
            self.id.clone()
        }
    }

    #[test]
    fn test_listener_registry_creation() {
        let registry = DefaultListenerRegistry::new();
        assert_eq!(registry.listener_count(), 0);
    }

    #[test]
    fn test_listener_registration() {
        let mut registry = DefaultListenerRegistry::new();
        let listener = Arc::new(MockListener::new("test1", ScanMode::FILES));
        
        assert!(registry.register_listener(listener).is_ok());
        assert_eq!(registry.listener_count(), 1);
    }

    #[test]
    fn test_duplicate_listener_registration() {
        let mut registry = DefaultListenerRegistry::new();
        let listener1 = Arc::new(MockListener::new("test1", ScanMode::FILES));
        let listener2 = Arc::new(MockListener::new("test1", ScanMode::HISTORY)); // Same ID
        
        assert!(registry.register_listener(listener1).is_ok());
        assert!(registry.register_listener(listener2).is_err()); // Should fail
        assert_eq!(registry.listener_count(), 1);
    }

    #[test]
    fn test_interested_listeners_filtering() {
        let mut registry = DefaultListenerRegistry::new();
        let files_listener = Arc::new(MockListener::new("files", ScanMode::FILES));
        let history_listener = Arc::new(MockListener::new("history", ScanMode::HISTORY));
        let combined_listener = Arc::new(MockListener::new("combined", ScanMode::FILES | ScanMode::HISTORY));
        
        registry.register_listener(files_listener).unwrap();
        registry.register_listener(history_listener).unwrap();
        registry.register_listener(combined_listener).unwrap();
        
        // Test FILES mode filtering
        let files_interested = registry.get_interested_listeners(ScanMode::FILES);
        assert_eq!(files_interested.len(), 2); // files and combined
        
        // Test HISTORY mode filtering  
        let history_interested = registry.get_interested_listeners(ScanMode::HISTORY);
        assert_eq!(history_interested.len(), 2); // history and combined
        
        // Test METRICS mode filtering (none interested)
        let metrics_interested = registry.get_interested_listeners(ScanMode::METRICS);
        assert_eq!(metrics_interested.len(), 0);
    }
}