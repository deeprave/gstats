//! Generic Async Notification Manager
//! 
//! Central coordinator for the pub/sub notification system. Manages subscriber
//! registration, event routing, rate limiting, and delivery statistics.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tokio::time::timeout;
use log::{debug, warn, error};

use crate::notifications::traits::{
    NotificationManager, Subscriber, RateLimit, OverflowAction, DeliveryStats
};
use crate::notifications::events::NotificationEvent;
use crate::notifications::error::{NotificationError, NotificationResult};

/// Subscriber information with rate limiting and statistics
struct SubscriberInfo<T> 
where 
    T: NotificationEvent
{
    subscriber: Arc<dyn Subscriber<T>>,
    rate_limit: Option<RateLimit>,
    last_event_times: VecDeque<Instant>,
    stats: SubscriberStats,
}

/// Statistics for individual subscribers
#[derive(Debug, Clone, Default)]
pub struct SubscriberStats {
    events_received: u64,
    events_processed: u64,
    events_dropped: u64,
    processing_failures: u64,
    total_processing_time_us: u64,
    last_event_at: Option<SystemTime>,
}

/// Generic async notification manager
pub struct AsyncNotificationManager<T> 
where 
    T: NotificationEvent
{
    subscribers: Arc<RwLock<HashMap<String, SubscriberInfo<T>>>>,
    global_stats: Arc<RwLock<DeliveryStats>>,
    default_timeout: Duration,
    shutdown: Arc<RwLock<bool>>,
    max_subscribers: Option<usize>,
}

impl<T> AsyncNotificationManager<T> 
where 
    T: NotificationEvent
{
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            global_stats: Arc::new(RwLock::new(DeliveryStats::default())),
            default_timeout: Duration::from_secs(5),
            shutdown: Arc::new(RwLock::new(false)),
            max_subscribers: Some(1000), // Reasonable default limit
        }
    }
    
    /// Create a new notification manager with custom configuration
    pub fn with_config(default_timeout: Duration, max_subscribers: Option<usize>) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            global_stats: Arc::new(RwLock::new(DeliveryStats::default())),
            default_timeout,
            shutdown: Arc::new(RwLock::new(false)),
            max_subscribers,
        }
    }
    
    /// Check if the system is shutting down
    async fn is_shutting_down(&self) -> bool {
        *self.shutdown.read().await
    }
    
    /// Check rate limiting for a subscriber
    fn check_rate_limit(subscriber_info: &mut SubscriberInfo<T>) -> bool {
        if let Some(rate_limit) = &subscriber_info.rate_limit {
            let now = Instant::now();
            let one_second_ago = now - Duration::from_secs(1);
            
            // Remove old timestamps
            while let Some(&front_time) = subscriber_info.last_event_times.front() {
                if front_time < one_second_ago {
                    subscriber_info.last_event_times.pop_front();
                } else {
                    break;
                }
            }
            
            // Check if we're at the limit
            if subscriber_info.last_event_times.len() >= rate_limit.max_events_per_second as usize {
                match rate_limit.overflow_action {
                    OverflowAction::Drop => {
                        subscriber_info.stats.events_dropped += 1;
                        return false;
                    }
                    OverflowAction::Error => {
                        return false;
                    }
                    OverflowAction::Queue => {
                        // For now, treat queue as drop - could implement actual queuing later
                        subscriber_info.stats.events_dropped += 1;
                        return false;
                    }
                }
            }
            
            // Record this event
            subscriber_info.last_event_times.push_back(now);
        }
        true
    }
    
    /// Deliver an event to a specific subscriber
    async fn deliver_to_subscriber(
        subscriber_info: &mut SubscriberInfo<T>,
        event: &T,
        timeout_duration: Duration,
    ) -> NotificationResult<()> {
        let subscriber_id = subscriber_info.subscriber.subscriber_id().to_string();
        
        // Check if subscriber should receive this event
        if !subscriber_info.subscriber.should_receive(event) {
            debug!("Subscriber '{}' filtered out event", subscriber_id);
            return Ok(());
        }
        
        // Check rate limiting
        if !Self::check_rate_limit(subscriber_info) {
            debug!("Rate limit exceeded for subscriber '{}'", subscriber_id);
            return Err(NotificationError::rate_limit_exceeded(
                subscriber_id,
                subscriber_info.rate_limit.as_ref().unwrap().max_events_per_second,
            ));
        }
        
        // Update stats
        subscriber_info.stats.events_received += 1;
        subscriber_info.stats.last_event_at = Some(SystemTime::now());
        
        // Deliver the event with timeout
        let start_time = Instant::now();
        let delivery_result = timeout(
            timeout_duration,
            subscriber_info.subscriber.handle_event(event.clone())
        ).await;
        
        let processing_time = start_time.elapsed();
        subscriber_info.stats.total_processing_time_us += processing_time.as_micros() as u64;
        
        match delivery_result {
            Ok(Ok(())) => {
                subscriber_info.stats.events_processed += 1;
                debug!("Successfully delivered event to '{}' in {:?}", subscriber_id, processing_time);
                Ok(())
            }
            Ok(Err(e)) => {
                subscriber_info.stats.processing_failures += 1;
                error!("Subscriber '{}' failed to process event: {}", subscriber_id, e);
                Err(NotificationError::delivery_failed(subscriber_id, e.to_string()))
            }
            Err(_) => {
                subscriber_info.stats.processing_failures += 1;
                error!("Timeout delivering event to subscriber '{}'", subscriber_id);
                Err(NotificationError::timeout("event_delivery", timeout_duration.as_millis() as u64))
            }
        }
    }
    
    /// Get delivery statistics
    pub async fn get_stats(&self) -> DeliveryStats {
        self.global_stats.read().await.clone()
    }
    
    /// Get subscriber-specific statistics
    pub async fn get_subscriber_stats(&self, subscriber_id: &str) -> Option<SubscriberStats> {
        let subscribers = self.subscribers.read().await;
        subscribers.get(subscriber_id).map(|info| info.stats.clone())
    }
    
    /// List all subscriber IDs
    pub async fn list_subscribers(&self) -> Vec<String> {
        let subscribers = self.subscribers.read().await;
        subscribers.keys().cloned().collect()
    }
    
    /// Clear all statistics
    pub async fn clear_stats(&self) {
        let mut global_stats = self.global_stats.write().await;
        *global_stats = DeliveryStats::default();
        
        let mut subscribers = self.subscribers.write().await;
        for subscriber_info in subscribers.values_mut() {
            subscriber_info.stats = SubscriberStats::default();
        }
    }
    
    /// Unsubscribe a subscriber by ID (public method that works with &self)
    pub async fn unsubscribe_by_id(&self, subscriber_id: &str) -> NotificationResult<()> {
        let mut subscribers = self.subscribers.write().await;
        
        if subscribers.remove(subscriber_id).is_some() {
            debug!("Unsubscribed '{}' from notifications", subscriber_id);
            Ok(())
        } else {
            Err(NotificationError::subscriber_not_found(subscriber_id))
        }
    }
}

#[async_trait::async_trait]
impl<T> NotificationManager<T> for AsyncNotificationManager<T> 
where 
    T: NotificationEvent
{
    async fn subscribe(&mut self, subscriber: Arc<dyn Subscriber<T>>) -> NotificationResult<()> {
        if self.is_shutting_down().await {
            return Err(NotificationError::SystemShutdown);
        }
        
        let subscriber_id = subscriber.subscriber_id().to_string();
        let mut subscribers = self.subscribers.write().await;
        
        // Check subscriber limit
        if let Some(max) = self.max_subscribers {
            if subscribers.len() >= max {
                return Err(NotificationError::generic(
                    format!("Maximum number of subscribers ({}) reached", max)
                ));
            }
        }
        
        // Check if subscriber already exists
        if subscribers.contains_key(&subscriber_id) {
            return Err(NotificationError::subscriber_already_exists(subscriber_id));
        }
        
        // Create subscriber info
        let rate_limit = subscriber.rate_limit();
        let subscriber_info = SubscriberInfo {
            subscriber,
            rate_limit,
            last_event_times: VecDeque::new(),
            stats: SubscriberStats::default(),
        };
        
        subscribers.insert(subscriber_id.clone(), subscriber_info);
        debug!("Subscribed '{}' to notifications", subscriber_id);
        
        Ok(())
    }
    
    async fn unsubscribe(&mut self, subscriber_id: &str) -> NotificationResult<()> {
        let mut subscribers = self.subscribers.write().await;
        
        if subscribers.remove(subscriber_id).is_some() {
            debug!("Unsubscribed '{}' from notifications", subscriber_id);
            Ok(())
        } else {
            Err(NotificationError::subscriber_not_found(subscriber_id))
        }
    }
    
    async fn publish(&self, event: T) -> NotificationResult<()> {
        if self.is_shutting_down().await {
            return Err(NotificationError::SystemShutdown);
        }
        
        let mut subscribers = self.subscribers.write().await;
        let mut global_stats = self.global_stats.write().await;
        
        global_stats.events_published += 1;
        let start_time = Instant::now();
        
        let mut delivery_count = 0;
        let mut failure_count = 0;
        
        // Deliver to all subscribers
        for (subscriber_id, subscriber_info) in subscribers.iter_mut() {
            match Self::deliver_to_subscriber(subscriber_info, &event, self.default_timeout).await {
                Ok(()) => {
                    delivery_count += 1;
                }
                Err(e) => {
                    failure_count += 1;
                    warn!("Failed to deliver event to '{}': {}", subscriber_id, e);
                }
            }
        }
        
        // Update global stats
        global_stats.events_delivered += delivery_count;
        global_stats.delivery_failures += failure_count;
        
        let total_time = start_time.elapsed();
        if delivery_count > 0 {
            let avg_time = total_time.as_micros() as u64 / delivery_count;
            global_stats.avg_delivery_time_us = 
                (global_stats.avg_delivery_time_us + avg_time) / 2;
        }
        
        debug!("Published event to {} subscribers ({} successful, {} failed) in {:?}", 
               subscribers.len(), delivery_count, failure_count, total_time);
        
        Ok(())
    }
    
    async fn publish_to(&self, event: T, subscriber_id: &str) -> NotificationResult<()> {
        if self.is_shutting_down().await {
            return Err(NotificationError::SystemShutdown);
        }
        
        let mut subscribers = self.subscribers.write().await;
        let mut global_stats = self.global_stats.write().await;
        
        global_stats.events_published += 1;
        
        if let Some(subscriber_info) = subscribers.get_mut(subscriber_id) {
            match Self::deliver_to_subscriber(subscriber_info, &event, self.default_timeout).await {
                Ok(()) => {
                    global_stats.events_delivered += 1;
                    Ok(())
                }
                Err(e) => {
                    global_stats.delivery_failures += 1;
                    Err(e)
                }
            }
        } else {
            Err(NotificationError::subscriber_not_found(subscriber_id))
        }
    }
    
    async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }
    
    async fn has_subscriber(&self, subscriber_id: &str) -> bool {
        self.subscribers.read().await.contains_key(subscriber_id)
    }
    
    async fn shutdown(&mut self) -> NotificationResult<()> {
        debug!("Shutting down notification manager");
        
        // Mark as shutting down
        *self.shutdown.write().await = true;
        
        // Clear all subscribers
        let mut subscribers = self.subscribers.write().await;
        let subscriber_count = subscribers.len();
        subscribers.clear();
        
        debug!("Notification manager shutdown complete ({} subscribers removed)", subscriber_count);
        Ok(())
    }
}

impl<T> Default for AsyncNotificationManager<T> 
where 
    T: NotificationEvent
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for AsyncNotificationManager<T> 
where 
    T: NotificationEvent
{
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
            global_stats: Arc::clone(&self.global_stats),
            default_timeout: self.default_timeout,
            shutdown: Arc::clone(&self.shutdown),
            max_subscribers: self.max_subscribers,
        }
    }
}
