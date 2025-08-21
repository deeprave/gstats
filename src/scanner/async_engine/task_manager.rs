//! Simple Task Manager
//! 
//! Basic concurrent task management for scanner operations.

use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use std::time::{Duration, Instant};
use super::error::{ScanError, ScanResult};

/// Simple task handle for basic tracking
#[derive(Debug)]
pub struct TaskHandle {
    pub name: String,
    pub started_at: Instant,
    pub handle: JoinHandle<ScanResult<()>>,
}

/// Simple task manager for basic concurrency control
pub struct TaskManager {
    /// Semaphore for limiting concurrent tasks
    semaphore: Arc<Semaphore>,
    /// Global cancellation token
    cancellation_token: CancellationToken,
    /// Active tasks for cleanup
    active_tasks: Arc<tokio::sync::Mutex<Vec<TaskHandle>>>,
}

impl TaskManager {
    /// Create a new task manager with specified concurrency limit
    pub fn new(max_concurrent_tasks: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent_tasks)),
            cancellation_token: CancellationToken::new(),
            active_tasks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
    
    /// Spawn a new scanning task
    pub async fn spawn_task<F, Fut>(
        &self,
        task_name: String,
        task_fn: F,
    ) -> ScanResult<()>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ScanResult<()>> + Send + 'static,
    {
        // Acquire semaphore permit
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| ScanError::resource_limit("Failed to acquire task permit"))?;
        
        let cancellation = self.cancellation_token.child_token();
        let active_tasks = Arc::clone(&self.active_tasks);
        let task_name_clone = task_name.clone();
        
        // Spawn the task
        let handle = tokio::spawn(async move {
            let _permit = permit; // Keep permit alive
            let result = task_fn(cancellation).await;
            
            // Remove from active tasks on completion
            {
                let mut tasks = active_tasks.lock().await;
                tasks.retain(|t| t.name != task_name_clone);
            }
            
            result
        });
        
        // Add to active tasks
        {
            let mut tasks = self.active_tasks.lock().await;
            tasks.push(TaskHandle {
                name: task_name,
                started_at: Instant::now(),
                handle,
            });
        }
        
        Ok(())
    }
    
    
    /// Cancel all active tasks
    pub async fn cancel_all(&self) {
        self.cancellation_token.cancel();
        
        // Wait for all tasks to complete with timeout
        let mut tasks = self.active_tasks.lock().await;
        let handles: Vec<_> = tasks.drain(..).map(|t| t.handle).collect();
        drop(tasks); // Release lock
        
        for handle in handles {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }
    }
    
    /// Wait for all active tasks to complete
    pub async fn wait_all(&self) -> ScanResult<()> {
        let mut tasks = self.active_tasks.lock().await;
        let handles: Vec<_> = tasks.drain(..).map(|t| t.handle).collect();
        drop(tasks); // Release lock
        
        for handle in handles {
            match handle.await {
                Ok(Ok(())) => {},
                Ok(Err(e)) => return Err(e),
                Err(join_err) => return Err(ScanError::from(join_err)),
            }
        }
        
        Ok(())
    }
    
    /// Get the number of active tasks
    pub async fn active_task_count(&self) -> usize {
        self.active_tasks.lock().await.len()
    }
    
    /// Check if cancellation was requested
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_task_spawning() {
        let manager = TaskManager::new(2);
        
        manager.spawn_task("test-task".to_string(), |_cancel| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        }).await.unwrap();
        
        assert_eq!(manager.active_task_count().await, 1);
        
        manager.wait_all().await.unwrap();
        
        assert_eq!(manager.active_task_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_cancellation() {
        let manager = TaskManager::new(5);
        
        // Spawn a long-running task
        manager.spawn_task("long-task".to_string(), |cancel| async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    Err(ScanError::async_operation("Should have been cancelled"))
                }
                _ = cancel.cancelled() => {
                    Ok(()) // Normal cancellation
                }
            }
        }).await.unwrap();
        
        // Cancel after a short delay
        tokio::time::sleep(Duration::from_millis(10)).await;
        manager.cancel_all().await;
        
        assert!(manager.is_cancelled());
        assert_eq!(manager.active_task_count().await, 0);
    }
    
    
}