//! Async Notification Manager
//! 
//! Real-time notification system for plugin communication using broadcast channels.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use crate::plugin::traits::{NotificationPlugin, QueueUpdate, ScanProgress, SystemEvent, NotificationPreferences};
use crate::plugin::error::{PluginError, PluginResult};

/// Type alias for a notification plugin reference
type NotificationPluginRef = Arc<dyn NotificationPlugin>;

/// Subscriber information with rate limiting state
struct Subscriber {
    plugin: NotificationPluginRef,
    preferences: NotificationPreferences,
    last_notification_times: VecDeque<Instant>,
}

impl Subscriber {
    fn new(plugin: NotificationPluginRef) -> Self {
        let preferences = plugin.notification_preferences();
        Self {
            plugin,
            preferences,
            last_notification_times: VecDeque::new(),
        }
    }

    /// Check if notification should be rate limited
    fn should_rate_limit(&mut self) -> bool {
        if let Some(max_freq) = self.preferences.max_frequency {
            let now = Instant::now();
            let one_second_ago = now - Duration::from_secs(1);
            
            // Remove old timestamps
            while let Some(&front_time) = self.last_notification_times.front() {
                if front_time < one_second_ago {
                    self.last_notification_times.pop_front();
                } else {
                    break;
                }
            }
            
            // Check if we're at the limit
            if self.last_notification_times.len() >= max_freq as usize {
                return true;
            }
            
            // Record this notification
            self.last_notification_times.push_back(now);
        }
        false
    }
}

/// Async notification manager for plugin communication
pub struct AsyncNotificationManager {
    subscribers: Arc<RwLock<HashMap<String, Subscriber>>>,
    default_timeout: Duration,
    shutdown: Arc<RwLock<bool>>,
}

impl AsyncNotificationManager {
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            default_timeout: Duration::from_secs(5),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a new notification manager with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            default_timeout: timeout,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Subscribe a plugin to notifications
    pub async fn subscribe_plugin(&mut self, plugin: NotificationPluginRef) -> PluginResult<()> {
        let plugin_name = plugin.plugin_info().name.clone();
        let mut subscribers = self.subscribers.write().await;
        
        if subscribers.contains_key(&plugin_name) {
            return Err(PluginError::plugin_already_registered(plugin_name));
        }
        
        subscribers.insert(plugin_name, Subscriber::new(plugin));
        Ok(())
    }

    /// Unsubscribe a plugin from notifications
    pub async fn unsubscribe_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        let mut subscribers = self.subscribers.write().await;
        
        if subscribers.remove(plugin_name).is_none() {
            return Err(PluginError::plugin_not_found(plugin_name.to_string()));
        }
        
        Ok(())
    }

    /// Get the number of subscribers
    pub fn subscriber_count(&self) -> usize {
        // Use try_read to avoid blocking in sync context
        match self.subscribers.try_read() {
            Ok(subscribers) => subscribers.len(),
            Err(_) => 0, // Return 0 if lock is held
        }
    }


    /// Get list of subscriber names
    pub async fn get_subscribers(&self) -> Vec<String> {
        let subscribers = self.subscribers.read().await;
        subscribers.keys().cloned().collect()
    }

    /// Get subscribers that want queue updates
    pub async fn get_subscribers_for_queue_updates(&self) -> Vec<String> {
        let subscribers = self.subscribers.read().await;
        subscribers
            .iter()
            .filter(|(_, sub)| sub.preferences.queue_updates)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Notify all subscribers of a queue update
    pub async fn notify_queue_update(&self, update: QueueUpdate) -> PluginResult<()> {
        if *self.shutdown.read().await {
            return Err(PluginError::generic("Notification manager has been shutdown"));
        }

        let mut subscribers = self.subscribers.write().await;
        
        for (_, subscriber) in subscribers.iter_mut() {
            if !subscriber.preferences.queue_updates {
                continue; // Skip if plugin doesn't want queue updates
            }
            
            if subscriber.should_rate_limit() {
                continue; // Skip if rate limited
            }
            
            // Send notification directly (for reliable delivery in tests)
            let _ = subscriber.plugin.on_queue_update(update.clone()).await;
        }
        
        Ok(())
    }

    /// Notify all subscribers of scan progress
    pub async fn notify_scan_progress(&self, progress: ScanProgress) -> PluginResult<()> {
        if *self.shutdown.read().await {
            return Err(PluginError::generic("Notification manager has been shutdown"));
        }

        let mut subscribers = self.subscribers.write().await;
        
        for (_, subscriber) in subscribers.iter_mut() {
            if !subscriber.preferences.scan_progress {
                continue;
            }
            
            if subscriber.should_rate_limit() {
                continue;
            }
            
            let _ = subscriber.plugin.on_scan_progress(progress.clone()).await;
        }
        
        Ok(())
    }

    /// Notify all subscribers of an error
    pub async fn notify_error(&self, error: PluginError) -> PluginResult<()> {
        if *self.shutdown.read().await {
            return Err(PluginError::generic("Notification manager has been shutdown"));
        }

        let mut subscribers = self.subscribers.write().await;
        
        for (_, subscriber) in subscribers.iter_mut() {
            if !subscriber.preferences.error_notifications {
                continue;
            }
            
            if subscriber.should_rate_limit() {
                continue;
            }
            
            let _ = subscriber.plugin.on_error(error.clone()).await;
        }
        
        Ok(())
    }

    /// Notify all subscribers of a system event
    pub async fn notify_system_event(&self, event: SystemEvent) -> PluginResult<()> {
        if *self.shutdown.read().await {
            return Err(PluginError::generic("Notification manager has been shutdown"));
        }

        let mut subscribers = self.subscribers.write().await;
        
        for (_, subscriber) in subscribers.iter_mut() {
            // Check if plugin wants this specific event type
            if !subscriber.preferences.system_events.contains(&event.event_type) {
                continue;
            }
            
            if subscriber.should_rate_limit() {
                continue;
            }
            
            let _ = subscriber.plugin.on_system_event(event.clone()).await;
        }
        
        Ok(())
    }

    /// Shutdown the notification manager
    pub async fn shutdown(&mut self) -> PluginResult<()> {
        *self.shutdown.write().await = true;
        self.subscribers.write().await.clear();
        Ok(())
    }
}

impl Clone for AsyncNotificationManager {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
            default_timeout: self.default_timeout,
            shutdown: Arc::clone(&self.shutdown),
        }
    }
}

impl Default for AsyncNotificationManager {
    fn default() -> Self {
        Self::new()
    }
}