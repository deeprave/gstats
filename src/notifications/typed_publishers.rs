//! Typed Publisher/Subscriber Wrappers
//! 
//! Provides type-safe wrappers around the unified notification manager
//! to allow components to work with specific event types.

use std::sync::Arc;
use async_trait::async_trait;
use crate::notifications::traits::{Publisher, NotificationManager};
use crate::notifications::error::NotificationResult;
use crate::notifications::events::{UnifiedEvent, ScanEvent, QueueEvent, PluginEvent};
use crate::notifications::manager::AsyncNotificationManager;

/// Type-safe publisher for ScanEvent
pub struct ScanEventPublisher {
    pub(crate) unified_manager: Arc<AsyncNotificationManager<UnifiedEvent>>,
}

impl ScanEventPublisher {
    pub fn new(unified_manager: Arc<AsyncNotificationManager<UnifiedEvent>>) -> Self {
        Self { unified_manager }
    }
}

#[async_trait]
impl Publisher<ScanEvent> for ScanEventPublisher {
    async fn publish(&self, event: ScanEvent) -> NotificationResult<()> {
        let unified = UnifiedEvent::from(event);
        NotificationManager::publish(&*self.unified_manager, unified).await
    }
}

/// Type-safe publisher for QueueEvent
pub struct QueueEventPublisher {
    pub(crate) unified_manager: Arc<AsyncNotificationManager<UnifiedEvent>>,
}

impl QueueEventPublisher {
    pub fn new(unified_manager: Arc<AsyncNotificationManager<UnifiedEvent>>) -> Self {
        Self { unified_manager }
    }
}

#[async_trait]
impl Publisher<QueueEvent> for QueueEventPublisher {
    async fn publish(&self, event: QueueEvent) -> NotificationResult<()> {
        let unified = UnifiedEvent::from(event);
        NotificationManager::publish(&*self.unified_manager, unified).await
    }
}

/// Type-safe publisher for PluginEvent
pub struct PluginEventPublisher {
    pub(crate) unified_manager: Arc<AsyncNotificationManager<UnifiedEvent>>,
}

impl PluginEventPublisher {
    pub fn new(unified_manager: Arc<AsyncNotificationManager<UnifiedEvent>>) -> Self {
        Self { unified_manager }
    }
}

#[async_trait]
impl Publisher<PluginEvent> for PluginEventPublisher {
    async fn publish(&self, event: PluginEvent) -> NotificationResult<()> {
        let unified = UnifiedEvent::from(event);
        NotificationManager::publish(&*self.unified_manager, unified).await
    }
}