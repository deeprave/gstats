//! Generic Publisher/Subscriber Traits
//! 
//! Core traits for the pub/sub notification system that enable loose coupling
//! between event producers and consumers.

use std::sync::Arc;
use async_trait::async_trait;
use crate::notifications::error::NotificationResult;
use crate::notifications::events::NotificationEvent;

/// Generic publisher trait for components that emit events
#[async_trait]
pub trait Publisher<T>: Send + Sync 
where 
    T: NotificationEvent
{
    /// Publish an event to all subscribers
    async fn publish(&self, event: T) -> NotificationResult<()>;
    
    /// Publish an event to a specific subscriber
    async fn publish_to(&self, event: T, subscriber_id: &str) -> NotificationResult<()>;
    
    /// Get the publisher identifier
    fn publisher_id(&self) -> &str;
}

/// Generic subscriber trait for components that handle events
#[async_trait]
pub trait Subscriber<T>: Send + Sync 
where 
    T: NotificationEvent
{
    /// Handle an incoming event
    async fn handle_event(&self, event: T) -> NotificationResult<()>;
    
    /// Get the subscriber identifier (must be unique)
    fn subscriber_id(&self) -> &str;
    
    /// Get event filter preferences
    fn event_filter(&self) -> EventFilter {
        EventFilter::AcceptAll
    }
    
    /// Get rate limiting preferences
    fn rate_limit(&self) -> Option<RateLimit> {
        Some(RateLimit::default())
    }
    
    /// Check if this subscriber should receive the event
    fn should_receive(&self, event: &T) -> bool {
        self.event_filter().should_accept(event)
    }
}

/// Generic notification manager trait
#[async_trait]
pub trait NotificationManager<T>: Send + Sync 
where 
    T: NotificationEvent
{
    /// Subscribe a component to receive events
    async fn subscribe(&self, subscriber: Arc<dyn Subscriber<T>>) -> NotificationResult<()>;
    
    /// Unsubscribe a component from receiving events
    async fn unsubscribe(&self, subscriber_id: &str) -> NotificationResult<()>;
    
    /// Publish an event to all subscribers
    async fn publish(&self, event: T) -> NotificationResult<()>;
    
    /// Publish an event to a specific subscriber
    async fn publish_to(&self, event: T, subscriber_id: &str) -> NotificationResult<()>;
    
    /// Get the number of active subscribers
    async fn subscriber_count(&self) -> usize;
    
    /// Check if a subscriber exists
    async fn has_subscriber(&self, subscriber_id: &str) -> bool;
    
    /// Shutdown the notification manager
    async fn shutdown(&mut self) -> NotificationResult<()>;
    
    /// Get delivery statistics
    async fn get_stats(&self) -> DeliveryStats;
}

/// Event filtering options for subscribers
#[derive(Debug, Clone)]
pub enum EventFilter {
    /// Accept all events
    AcceptAll,
}

impl EventFilter {
    /// Check if an event should be accepted
    pub fn should_accept<T>(&self, _event: &T) -> bool {
        match self {
            EventFilter::AcceptAll => true,
        }
    }
}

/// Rate limiting configuration for subscribers
#[derive(Debug, Clone)]
pub struct RateLimit {
    /// Maximum events per second
    pub max_events_per_second: u32,
    
    /// Action to take when rate limit is exceeded
    pub overflow_action: OverflowAction,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            max_events_per_second: 100,
            overflow_action: OverflowAction::Drop,
        }
    }
}

/// Action to take when rate limit is exceeded
#[derive(Debug, Clone)]
pub enum OverflowAction {
    /// Drop the event silently
    Drop,
}


/// Statistics about notification delivery
#[derive(Debug, Clone)]
pub struct DeliveryStats {
    /// Total events published
    pub events_published: u64,
    
    /// Total events delivered successfully
    pub events_delivered: u64,
    
    
    /// Total delivery failures
    pub delivery_failures: u64,
    
    /// Average delivery time in microseconds
    pub avg_delivery_time_us: u64,
}

impl Default for DeliveryStats {
    fn default() -> Self {
        Self {
            events_published: 0,
            events_delivered: 0,
            delivery_failures: 0,
            avg_delivery_time_us: 0,
        }
    }
}
