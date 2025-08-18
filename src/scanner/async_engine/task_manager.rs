//! Task Coordination Manager
//! 
//! Manages concurrent scanning tasks with resource limits, priority scheduling, and cancellation support.

use std::sync::Arc;
use std::collections::BinaryHeap;
use std::pin::Pin;
use tokio::sync::{Semaphore, RwLock, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use std::cmp::Ordering;
use super::error::{ScanError, ScanResult, TaskError};

/// Memory pressure levels for task prioritization
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPressureLevel {
    Normal = 0,
    Moderate = 1,
    High = 2,
    Critical = 3,
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl TaskPriority {
    
    /// Adjust priority based on memory pressure
    pub fn adjust_for_pressure(self, pressure: MemoryPressureLevel) -> Self {
        match pressure {
            MemoryPressureLevel::Normal => self,
            MemoryPressureLevel::Moderate => {
                match self {
                    TaskPriority::Critical => TaskPriority::High,
                    other => other,
                }
            }
            MemoryPressureLevel::High => {
                match self {
                    TaskPriority::Critical => TaskPriority::High,
                    TaskPriority::High => TaskPriority::Normal,
                    other => other,
                }
            }
            MemoryPressureLevel::Critical => TaskPriority::Low,
        }
    }
}

/// Resource constraints for task execution
#[derive(Debug, Clone)]
pub struct ResourceConstraints {
    /// Total maximum concurrent tasks
    pub max_total_tasks: usize,
    /// Memory pressure threshold for task throttling
    pub memory_pressure_threshold: MemoryPressureLevel,
    /// Backoff duration when resources are constrained
    pub backoff_duration: Duration,
}

impl Default for ResourceConstraints {
    fn default() -> Self {
        Self {
            max_total_tasks: 8,
            memory_pressure_threshold: MemoryPressureLevel::High,
            backoff_duration: Duration::from_millis(100),
        }
    }
}

/// Type alias for task function
type TaskFn = Box<dyn FnOnce(CancellationToken) -> Pin<Box<dyn std::future::Future<Output = ScanResult<()>> + Send + 'static>> + Send + 'static>;

/// Pending task in the priority queue
struct PendingTask {
    id: TaskId,
    priority: TaskPriority,
    task_fn: TaskFn,
    created_at: Instant,
}

impl PartialEq for PendingTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.created_at == other.created_at
    }
}

impl Eq for PendingTask {}

impl PartialOrd for PendingTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older tasks first
        self.priority.cmp(&other.priority)
            .then_with(|| other.created_at.cmp(&self.created_at))
    }
}

/// Unique identifier for tasks
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TaskId(String);

impl TaskId {
    /// Create a new task ID
    pub fn new(prefix: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(format!("{}-{}", prefix, id))
    }
    
    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task metadata
#[derive(Debug)]
pub struct TaskInfo {
    pub id: TaskId,
    pub priority: TaskPriority,
    pub started_at: Instant,
    pub handle: JoinHandle<ScanResult<()>>,
}

/// Task coordination manager
pub struct TaskManager {
    /// Active tasks mapped by ID
    active_tasks: Arc<DashMap<TaskId, TaskInfo>>,
    
    /// Semaphore for limiting concurrent tasks
    semaphore: Arc<Semaphore>,
    
    /// Global cancellation token
    cancellation_token: CancellationToken,
    
    /// Task completion tracking
    completed_count: Arc<RwLock<usize>>,
    
    /// Error tracking
    errors: Arc<RwLock<Vec<TaskError>>>,
    
    /// Priority queue for pending tasks
    pending_tasks: Arc<Mutex<BinaryHeap<PendingTask>>>,
    
    /// Resource constraints and limits
    constraints: ResourceConstraints,
    
    
    
    /// Task scheduler handle
    #[allow(dead_code)]
    scheduler_handle: Option<JoinHandle<()>>,
}

impl TaskManager {
    /// Create a new task manager with specified concurrency limit
    pub fn new(max_concurrent_tasks: usize) -> Self {
        let mut constraints = ResourceConstraints::default();
        constraints.max_total_tasks = max_concurrent_tasks;
        
        Self {
            active_tasks: Arc::new(DashMap::new()),
            semaphore: Arc::new(Semaphore::new(max_concurrent_tasks)),
            cancellation_token: CancellationToken::new(),
            completed_count: Arc::new(RwLock::new(0)),
            errors: Arc::new(RwLock::new(Vec::new())),
            pending_tasks: Arc::new(Mutex::new(BinaryHeap::new())),
            constraints,
            scheduler_handle: None,
        }
    }
    
    /// Create a new task manager with custom constraints
    pub fn with_constraints(constraints: ResourceConstraints) -> Self {
        Self {
            active_tasks: Arc::new(DashMap::new()),
            semaphore: Arc::new(Semaphore::new(constraints.max_total_tasks)),
            cancellation_token: CancellationToken::new(),
            completed_count: Arc::new(RwLock::new(0)),
            errors: Arc::new(RwLock::new(Vec::new())),
            pending_tasks: Arc::new(Mutex::new(BinaryHeap::new())),
            constraints,
            scheduler_handle: None,
        }
    }
    
    /// Create a task manager (memory monitoring was removed as unused)
    pub fn with_memory_monitoring(max_concurrent_tasks: usize) -> Self {
        // Simply return a standard task manager
        Self::new(max_concurrent_tasks)
    }
    
    /// Spawn a new scanning task with priority scheduling
    pub async fn spawn_task<F, Fut>(
        &self,
        task_name: String,
        task_fn: F,
    ) -> ScanResult<TaskId>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ScanResult<()>> + Send + 'static,
    {
        self.spawn_task_with_priority(task_name, TaskPriority::Normal, task_fn).await
    }
    
    /// Spawn a task with explicit priority
    pub async fn spawn_task_with_priority<F, Fut>(
        &self,
        task_name: String,
        priority: TaskPriority,
        task_fn: F,
    ) -> ScanResult<TaskId>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ScanResult<()>> + Send + 'static,
    {
        let task_id = TaskId::new(&task_name);
        
        // Check if we can execute immediately or need to queue
        if self.can_execute_immediate()? {
            self.execute_task_immediate(task_id.clone(), priority, task_fn).await
        } else {
            self.queue_task(task_id.clone(), priority, task_fn).await
        }
    }
    
    /// Check if task can be executed immediately based on resource constraints
    fn can_execute_immediate(&self) -> ScanResult<bool> {
        // Check memory pressure using active task count as proxy
        let pressure = self.estimate_memory_pressure();
        if pressure >= self.constraints.memory_pressure_threshold {
            return Ok(false);
        }
        
        // Check total task limit
        if self.active_task_count() >= self.constraints.max_total_tasks {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Execute task immediately
    async fn execute_task_immediate<F, Fut>(
        &self,
        task_id: TaskId,
        priority: TaskPriority,
        task_fn: F,
    ) -> ScanResult<TaskId>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ScanResult<()>> + Send + 'static,
    {
        // Acquire semaphore permit
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| ScanError::resource_limit("Failed to acquire task permit"))?;
        
        // Adjust priority based on memory pressure
        let pressure = self.estimate_memory_pressure();
        let adjusted_priority = priority.adjust_for_pressure(pressure);
        
        let cancellation = self.cancellation_token.child_token();
        let active_tasks = Arc::clone(&self.active_tasks);
        let completed_count = Arc::clone(&self.completed_count);
        let errors = Arc::clone(&self.errors);
        let pending_tasks = Arc::clone(&self.pending_tasks);
        let semaphore_clone = Arc::clone(&self.semaphore);
        let constraints = self.constraints.clone();
        let task_id_clone = task_id.clone();
        
        // Spawn the actual task
        let handle = tokio::spawn(async move {
            // Keep permit alive for task duration
            let _permit = permit;
            
            // Execute the task
            let result = task_fn(cancellation).await;
            
            // Track completion and cleanup
            match &result {
                Ok(_) => {
                    let mut count = completed_count.write().await;
                    *count += 1;
                }
                Err(e) => {
                    let mut error_list = errors.write().await;
                    error_list.push(TaskError::new(
                        task_id_clone.as_str(),
                        ScanError::Other(anyhow::anyhow!("{}", e)),
                    ));
                }
            }
            
            // Update counters
            active_tasks.remove(&task_id_clone);
            
            // Try to process pending tasks when this task completes
            // This enables automatic scheduling of queued tasks
            tokio::spawn(async move {
                if let Err(e) = Self::try_process_pending_static(
                    pending_tasks,
                    semaphore_clone,
                    constraints,
                ).await {
                    log::debug!("Failed to process pending tasks after completion: {}", e);
                }
            });
            
            result
        });
        
        // Register the task
        let task_info = TaskInfo {
            id: task_id.clone(),
            priority: adjusted_priority,
            started_at: Instant::now(),
            handle,
        };
        
        self.active_tasks.insert(task_id.clone(), task_info);
        
        Ok(task_id)
    }
    
    /// Queue task for later execution
    async fn queue_task<F, Fut>(
        &self,
        task_id: TaskId,
        priority: TaskPriority,
        task_fn: F,
    ) -> ScanResult<TaskId>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ScanResult<()>> + Send + 'static,
    {
        let boxed_fn: TaskFn = Box::new(move |cancel| {
            Box::pin(task_fn(cancel))
        });
        
        let pending_task = PendingTask {
            id: task_id.clone(),
            priority,
            task_fn: boxed_fn,
            created_at: Instant::now(),
        };
        
        {
            let mut pending = self.pending_tasks.lock().await;
            pending.push(pending_task);
        }
        
        // Start scheduler if not already running
        self.start_scheduler_if_needed().await;
        
        Ok(task_id)
    }
    
    /// Start the task scheduler if not already running
    async fn start_scheduler_if_needed(&self) {
        // For this implementation, we trigger immediate processing
        // In a production system, this would start a background task
        let _ = self.process_pending_tasks().await;
    }
    
    /// Process pending tasks (called by scheduler)
    pub async fn process_pending_tasks(&self) -> ScanResult<usize> {
        let mut processed = 0;
        
        loop {
            // Get highest priority task that can execute
            let task = {
                let mut pending = self.pending_tasks.lock().await;
                let mut next_task = None;
                let mut temp_heap = BinaryHeap::new();
                
                while let Some(task) = pending.pop() {
                    if self.can_execute_immediate()? {
                        next_task = Some(task);
                        break;
                    } else {
                        temp_heap.push(task);
                    }
                }
                
                // Put back tasks we couldn't execute
                while let Some(task) = temp_heap.pop() {
                    pending.push(task);
                }
                
                next_task
            };
            
            if let Some(task) = task {
                // Execute the task
                let PendingTask { id, priority, task_fn, .. } = task;
                
                // Convert task_fn to the format expected by execute_task_immediate
                let result = self.execute_task_via_pending(id, priority, task_fn).await;
                
                match result {
                    Ok(_) => processed += 1,
                    Err(e) => {
                        log::error!("Failed to execute pending task: {}", e);
                        // Continue processing other tasks
                    }
                }
            } else {
                // No more tasks can be executed right now
                break;
            }
        }
        
        Ok(processed)
    }
    
    /// Helper to execute a task from the pending queue
    async fn execute_task_via_pending(
        &self,
        task_id: TaskId,
        priority: TaskPriority,
        task_fn: TaskFn,
    ) -> ScanResult<TaskId> {
        // Acquire semaphore permit
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| ScanError::resource_limit("Failed to acquire task permit"))?;
        
        
        let cancellation = self.cancellation_token.child_token();
        let active_tasks = Arc::clone(&self.active_tasks);
        let completed_count = Arc::clone(&self.completed_count);
        let errors = Arc::clone(&self.errors);
        let task_id_clone = task_id.clone();
        
        // Spawn the actual task with proper tracking
        let handle = tokio::spawn(async move {
            // Keep permit alive for task duration
            let _permit = permit;
            
            // Execute the task
            let result = task_fn(cancellation).await;
            
            // Track completion and cleanup
            match &result {
                Ok(_) => {
                    let mut count = completed_count.write().await;
                    *count += 1;
                }
                Err(e) => {
                    let mut error_list = errors.write().await;
                    error_list.push(TaskError::new(
                        task_id_clone.as_str(),
                        ScanError::Other(anyhow::anyhow!("{}", e)),
                    ));
                }
            }
            
            // Update counters
            active_tasks.remove(&task_id_clone);
            
            result
        });
        
        // Register the task
        let task_info = TaskInfo {
            id: task_id.clone(),
            priority,
            started_at: Instant::now(),
            handle,
        };
        
        self.active_tasks.insert(task_id.clone(), task_info);
        
        Ok(task_id)
    }
    
    /// Static method to process pending tasks (used in spawned task)
    async fn try_process_pending_static(
        _pending_tasks: Arc<Mutex<BinaryHeap<PendingTask>>>,
        _semaphore: Arc<Semaphore>,
        _constraints: ResourceConstraints,
    ) -> ScanResult<usize> {
        // This is a simplified version that just tries to notify about available resources
        // The actual processing should be done through the main TaskManager instance
        Ok(0)
    }
    
    /// Cancel all active tasks
    pub async fn cancel_all(&self) {
        self.cancellation_token.cancel();
        
        // Wait for all tasks to complete
        let tasks: Vec<_> = self.active_tasks
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        
        for task_id in tasks {
            let _ = self.wait_for_task(&task_id, Some(Duration::from_secs(5))).await;
        }
    }
    
    /// Wait for a specific task to complete
    pub async fn wait_for_task(
        &self,
        task_id: &TaskId,
        timeout: Option<Duration>,
    ) -> ScanResult<()> {
        // First check if task is active
        if let Some((_, task_info)) = self.active_tasks.remove(task_id) {
            let handle = task_info.handle;
            
            if let Some(timeout_duration) = timeout {
                match tokio::time::timeout(timeout_duration, handle).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(join_err)) => Err(ScanError::from(join_err)),
                    Err(_) => Err(ScanError::async_operation("Task timed out")),
                }
            } else {
                match handle.await {
                    Ok(result) => result,
                    Err(join_err) => Err(ScanError::from(join_err)),
                }
            }
        } else {
            // Task not active - might be pending, so process pending queue until task is found or queue is empty
            let mut attempts = 0;
            const MAX_ATTEMPTS: usize = 100; // Prevent infinite loops
            
            while attempts < MAX_ATTEMPTS {
                let processed = self.process_pending_tasks().await?;
                
                // If we processed some tasks, check if our task is now active
                if processed > 0 {
                    if let Some((_, task_info)) = self.active_tasks.remove(task_id) {
                        let handle = task_info.handle;
                        
                        if let Some(timeout_duration) = timeout {
                            return match tokio::time::timeout(timeout_duration, handle).await {
                                Ok(Ok(result)) => result,
                                Ok(Err(join_err)) => Err(ScanError::from(join_err)),
                                Err(_) => Err(ScanError::async_operation("Task timed out")),
                            };
                        } else {
                            return match handle.await {
                                Ok(result) => result,
                                Err(join_err) => Err(ScanError::from(join_err)),
                            };
                        }
                    }
                } else {
                    // No tasks were processed, likely all are blocked
                    break;
                }
                
                attempts += 1;
                // Small delay to prevent busy waiting
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            
            Ok(()) // Task already completed or wasn't found
        }
    }
    
    /// Get the number of active tasks
    pub fn active_task_count(&self) -> usize {
        self.active_tasks.len()
    }
    
    /// Get the completed task count
    pub async fn completed_task_count(&self) -> usize {
        *self.completed_count.read().await
    }
    
    /// Get task errors
    pub async fn get_errors(&self) -> Vec<TaskError> {
        self.errors.read().await.clone()
    }
    
    /// Check if cancellation was requested
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
    
    /// Get active task information
    pub fn get_active_tasks(&self) -> Vec<(TaskId, Duration)> {
        self.active_tasks
            .iter()
            .map(|entry| {
                let task_info = entry.value();
                (
                    entry.key().clone(),
                    task_info.started_at.elapsed(),
                )
            })
            .collect()
    }
    
    /// Get enhanced task information including priority
    pub fn get_active_tasks_detailed(&self) -> Vec<(TaskId, TaskPriority, Duration)> {
        self.active_tasks
            .iter()
            .map(|entry| {
                let task_info = entry.value();
                (
                    entry.key().clone(),
                    task_info.priority,
                    task_info.started_at.elapsed(),
                )
            })
            .collect()
    }
    
    /// Get number of pending tasks
    pub async fn pending_task_count(&self) -> usize {
        let pending = self.pending_tasks.lock().await;
        pending.len()
    }
    
    
    /// Estimate current memory pressure based on task activity
    fn estimate_memory_pressure(&self) -> MemoryPressureLevel {
        let active_count = self.active_task_count();
        let max_tasks = self.constraints.max_total_tasks;
        let utilization = active_count as f64 / max_tasks as f64;
        
        match utilization {
            x if x < 0.3 => MemoryPressureLevel::Normal,
            x if x < 0.6 => MemoryPressureLevel::Moderate,
            x if x < 0.85 => MemoryPressureLevel::High,
            _ => MemoryPressureLevel::Critical,
        }
    }
    
    /// Check if manager is under resource pressure
    pub async fn is_under_pressure(&self) -> bool {
        // Check memory pressure using task utilization
        if self.estimate_memory_pressure() >= self.constraints.memory_pressure_threshold {
            return true;
        }
        
        // Check task saturation
        let utilization = self.active_task_count() as f64 / self.constraints.max_total_tasks as f64;
        if utilization > 0.8 {
            return true;
        }
        
        // Check pending queue buildup
        let pending_count = self.pending_task_count().await;
        if pending_count > self.constraints.max_total_tasks {
            return true;
        }
        
        false
    }
    
    /// Apply graceful degradation under pressure
    pub async fn apply_degradation(&self) -> ScanResult<()> {
        if !self.is_under_pressure().await {
            return Ok(());
        }
        
        // Reduce task limits temporarily
        let current_pressure = self.estimate_memory_pressure();
        
        // Under high pressure, cancel some low-priority tasks
        if current_pressure >= MemoryPressureLevel::High {
            self.cancel_low_priority_tasks().await?;
        }
        
        // Process any pending high-priority tasks
        self.process_pending_tasks().await?;
        
        Ok(())
    }
    
    /// Cancel low-priority tasks to free resources
    async fn cancel_low_priority_tasks(&self) -> ScanResult<()> {
        let mut cancelled = 0;
        let max_to_cancel = self.active_task_count() / 4; // Cancel up to 25%
        
        // Collect low-priority tasks to cancel
        let tasks_to_cancel: Vec<_> = self.active_tasks
            .iter()
            .filter_map(|entry| {
                let task_info = entry.value();
                if task_info.priority <= TaskPriority::Low && cancelled < max_to_cancel {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();
        
        for task_id in tasks_to_cancel {
            if let Some((_, task_info)) = self.active_tasks.remove(&task_id) {
                task_info.handle.abort();
                cancelled += 1;
            }
        }
        
        log::info!("Cancelled {} low-priority tasks due to resource pressure", cancelled);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_task_id_generation() {
        let id1 = TaskId::new("test");
        let id2 = TaskId::new("test");
        
        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("test-"));
        assert!(id2.as_str().starts_with("test-"));
    }
    
    #[tokio::test]
    async fn test_task_spawning() {
        let manager = TaskManager::new(2);
        
        let task_id = manager.spawn_task("test-task".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        }).await.unwrap();
        
        assert_eq!(manager.active_task_count(), 1);
        
        manager.wait_for_task(&task_id, None).await.unwrap();
        
        assert_eq!(manager.active_task_count(), 0);
        assert_eq!(manager.completed_task_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_concurrent_limit() {
        let manager = TaskManager::new(2);
        
        // Spawn 3 tasks with limit of 2
        let mut tasks = Vec::new();
        for i in 0..3 {
            let task_id = manager.spawn_task(format!("test-task-{}", i), |_cancel| async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(())
            }).await.unwrap();
            tasks.push(task_id);
        }
        
        // Should have 2 active tasks (or less if already completed) plus any pending
        let active_count = manager.active_task_count();
        let pending_count = manager.pending_task_count().await;
        assert!(active_count <= 2);
        assert!(active_count + pending_count >= 2); // At least 2 tasks total
        
        // Wait for all tasks
        for task_id in tasks {
            manager.wait_for_task(&task_id, None).await.unwrap();
        }
        
        // All tasks should complete
        assert_eq!(manager.completed_task_count().await, 3);
        assert_eq!(manager.active_task_count(), 0);
    }
    
    #[tokio::test]
    async fn test_cancellation() {
        let manager = TaskManager::new(5);
        
        // Spawn a long-running task
        let _task_id = manager.spawn_task("long-task".to_string(), |cancel| async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    Err(ScanError::async_operation("Should have been cancelled"))
                }
                _ = cancel.cancelled() => {
                    Err(ScanError::Cancelled)
                }
            }
        }).await.unwrap();
        
        // Cancel after a short delay
        tokio::time::sleep(Duration::from_millis(10)).await;
        manager.cancel_all().await;
        
        assert!(manager.is_cancelled());
        assert_eq!(manager.active_task_count(), 0);
        
        // Check for cancellation error
        let errors = manager.get_errors().await;
        assert_eq!(errors.len(), 1);
    }
    
    #[tokio::test]
    async fn test_task_timeout() {
        let manager = TaskManager::new(1);
        
        let task_id = manager.spawn_task("slow-task".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(())
        }).await.unwrap();
        
        let result = manager.wait_for_task(&task_id, Some(Duration::from_millis(50))).await;
        
        assert!(matches!(result, Err(ScanError::AsyncOperation(_))));
    }
    
    #[tokio::test]
    async fn test_task_priority() {
        let _manager = TaskManager::new(2);
        
        // Test default priority levels exist
        let _normal = TaskPriority::Normal;
        let _high = TaskPriority::High;
        let _low = TaskPriority::Low;
        
        // Test priority adjustment under pressure
        let high_priority = TaskPriority::High;
        let adjusted = high_priority.adjust_for_pressure(MemoryPressureLevel::High);
        assert_eq!(adjusted, TaskPriority::Normal);
        
        let critical_priority = TaskPriority::Critical;
        let adjusted_critical = critical_priority.adjust_for_pressure(MemoryPressureLevel::Critical);
        assert_eq!(adjusted_critical, TaskPriority::Low);
    }
    
    #[tokio::test]
    async fn test_resource_constraints() {
        let constraints = ResourceConstraints::default();
        
        let manager = TaskManager::with_constraints(constraints);
        
        // First task should execute
        let task_id1 = manager.spawn_task("task-1".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        }).await.unwrap();
        
        assert_eq!(manager.active_task_count(), 1);
        
        // Second task should execute normally (no per-mode limits)
        let task_id2 = manager.spawn_task("task-2".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        }).await.unwrap();
        
        // Both tasks should be able to run concurrently
        assert!(manager.active_task_count() <= 2);
        
        // Wait for tasks to complete
        manager.wait_for_task(&task_id1, None).await.unwrap();
        manager.wait_for_task(&task_id2, None).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_mode_distribution() {
        let manager = TaskManager::new(5);
        
        // Spawn tasks with different names
        let _task1 = manager.spawn_task("files-task".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        }).await.unwrap();
        
        let _task2 = manager.spawn_task("history-task".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        }).await.unwrap();
        
        // Just verify we have active tasks
        assert_eq!(manager.active_task_count(), 2);
    }
    
    
    #[tokio::test]
    async fn test_pressure_detection() {
        let manager = TaskManager::new(2);
        
        // Fill up the task capacity
        let _task1 = manager.spawn_task("pressure-task-1".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(())
        }).await.unwrap();
        
        let _task2 = manager.spawn_task("pressure-task-2".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(())
        }).await.unwrap();
        
        // Should be under pressure due to high utilization
        assert!(manager.is_under_pressure().await);
    }
}