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
    fn event_filter(&self) -> EventFilter<T> {
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
}

/// Event filtering options for subscribers
#[derive(Debug, Clone)]
pub enum EventFilter<T> 
where 
    T: NotificationEvent
{
    /// Accept all events
    AcceptAll,
    
    /// Accept no events (effectively unsubscribed)
    AcceptNone,
    
    /// Custom filter function
    Custom(fn(&T) -> bool),
}

impl<T> EventFilter<T> 
where 
    T: NotificationEvent
{
    /// Check if an event should be accepted
    pub fn should_accept(&self, event: &T) -> bool {
        match self {
            EventFilter::AcceptAll => true,
            EventFilter::AcceptNone => false,
            EventFilter::Custom(filter_fn) => filter_fn(event),
        }
    }
}

/// Rate limiting configuration for subscribers
#[derive(Debug, Clone)]
pub struct RateLimit {
    /// Maximum events per second
    pub max_events_per_second: u32,
    
    /// Burst allowance (events that can be processed immediately)
    pub burst_allowance: u32,
    
    /// Action to take when rate limit is exceeded
    pub overflow_action: OverflowAction,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            max_events_per_second: 100,
            burst_allowance: 10,
            overflow_action: OverflowAction::Drop,
        }
    }
}

/// Action to take when rate limit is exceeded
#[derive(Debug, Clone)]
pub enum OverflowAction {
    /// Drop the event silently
    Drop,
    
    /// Queue the event for later delivery
    Queue,
    
    /// Return an error
    Error,
}

/// Subscription preferences for fine-grained control
#[derive(Debug, Clone)]
pub struct SubscriptionPreferences {
    /// Event filter
    pub filter: String, // JSON or query string for complex filtering
    
    /// Rate limiting
    pub rate_limit: Option<RateLimit>,
    
    /// Priority level (higher numbers = higher priority)
    pub priority: u8,
    
    /// Whether to receive events during system shutdown
    pub receive_during_shutdown: bool,
}

impl Default for SubscriptionPreferences {
    fn default() -> Self {
        Self {
            filter: "*".to_string(), // Accept all by default
            rate_limit: Some(RateLimit::default()),
            priority: 50, // Medium priority
            receive_during_shutdown: false,
        }
    }
}

/// Statistics about notification delivery
#[derive(Debug, Clone)]
pub struct DeliveryStats {
    /// Total events published
    pub events_published: u64,
    
    /// Total events delivered successfully
    pub events_delivered: u64,
    
    /// Total events dropped due to rate limiting
    pub events_dropped: u64,
    
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
            events_dropped: 0,
            delivery_failures: 0,
            avg_delivery_time_us: 0,
        }
    }
}
