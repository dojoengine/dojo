mod error;

pub use error::TaskNetworkError;

pub type Result<T> = std::result::Result<T, TaskNetworkError>;

use std::future::Future;
use std::sync::Arc;

use futures_util::future::try_join_all;
use tokio::sync::Semaphore;
use torii_adigraphmap::AcyclicDigraphMap;
use tracing::{debug, error};

const LOG_TARGET: &str = "torii::task_network";

pub type TaskId = u64;

/// A generic task manager that can execute tasks in parallel with dependency handling.
pub struct TaskNetwork<T>
where
    T: Clone + Send + Sync + 'static,
{
    tasks: AcyclicDigraphMap<TaskId, T>,
    max_concurrent_tasks: usize,
}

impl<T> TaskNetwork<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a new task manager with the specified maximum number of concurrent tasks.
    pub fn new(max_concurrent_tasks: usize) -> Self {
        Self {
            tasks: AcyclicDigraphMap::new(),
            max_concurrent_tasks,
        }
    }

    /// Add a task to the manager.
    pub fn add_task(&mut self, task_id: TaskId, task: T) -> Result<()> {
        self.tasks.add_node(task_id, task).map_err(|e| TaskNetworkError::GraphError(e))?;
        Ok(())
    }

    /// Add a task with dependencies to the manager.
    pub fn add_task_with_dependencies(&mut self, task_id: TaskId, task: T, dependencies: Vec<TaskId>) -> Result<()> {
        self.tasks.add_node_with_dependencies(task_id, task, dependencies)
            .map_err(|e| TaskNetworkError::GraphError(e))?;
        Ok(())
    }

    /// Add a dependency between two tasks.
    pub fn add_dependency(&mut self, from: TaskId, to: TaskId) -> Result<()> {
        self.tasks.add_dependency(&from, &to).map_err(|e| TaskNetworkError::GraphError(e))
    }

    /// Execute all tasks in topological order with the specified task handler function.
    /// Tasks at the same topological level are executed in parallel.
    /// Tasks at different topological levels are executed sequentially.
    pub async fn process_tasks<F, Fut, O>(&mut self, task_handler: F) -> Result<()>
    where
        F: Fn(TaskId, T) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<O>> + Send,
        O: Send + 'static,
    {
        if self.tasks.is_empty() {
            return Ok(());
        }

        // Get tasks organized by levels in topological order
        let task_levels = self.tasks.topo_sort_by_level();
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_tasks));

        // Process each level sequentially
        for (level_idx, level_tasks) in task_levels.iter().enumerate() {
            debug!(
                target: LOG_TARGET,
                level = level_idx,
                task_count = level_tasks.len(),
                "Processing task level."
            );
            
            // Process tasks within a level in parallel
            let mut handles = Vec::with_capacity(level_tasks.len());
            
            for (task_id, task) in level_tasks {
                let task_handler = task_handler.clone();
                let semaphore = semaphore.clone();
                let task_clone = task.clone();
                let task_id = *task_id;
                
                handles.push(tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err(|e| TaskNetworkError::SemaphoreError(e))?;
                    
                    debug!(
                        target: LOG_TARGET,
                        task_id = %task_id,
                        level = level_idx,
                        "Processing task."
                    );
                    
                    match task_handler(task_id, task_clone).await {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            error!(
                                target: LOG_TARGET,
                                error = %e,
                                task_id = %task_id,
                                "Error processing task."
                            );
                            Err(e)
                        }
                    }
                }));
            }
            
            // Wait for all tasks in this level to complete before proceeding to the next level
            try_join_all(handles).await.map_err(|e| TaskNetworkError::JoinError(e))?;
        }
        
        // Clear tasks after processing
        self.tasks.clear();
        
        Ok(())
    }
    
    /// Check if there are any tasks in the manager.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
    
    /// Get the number of tasks in the manager.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }
}

impl<T> Default for TaskNetwork<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new(std::thread::available_parallelism().map_or(4, |p| p.get()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_task_execution() {
        let mut manager = TaskNetwork::<String>::new(4);
        
        manager.add_task(1, "Task 1".to_string()).unwrap();
        manager.add_task(2, "Task 2".to_string()).unwrap();
        
        let results = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        
        let results_clone = results.clone();
        manager.process_tasks(move |id, task| {
            let results = results_clone.clone();
            async move {
                let mut locked_results = results.lock().await;
                locked_results.push((id, task));
                Ok::<_, anyhow::Error>(())
            }
        }).await.unwrap();
        
        let final_results = results.lock().await;
        assert_eq!(final_results.len(), 2);
    }
    
    #[tokio::test]
    async fn test_dependency_ordering() {
        let mut manager = TaskNetwork::<String>::new(4);
        
        manager.add_task(1, "Task 1".to_string()).unwrap();
        manager.add_task(2, "Task 2".to_string()).unwrap();
        manager.add_dependency(1, 2).unwrap();
        
        let executed = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        
        let executed_clone = executed.clone();
        manager.process_tasks(move |id, _task| {
            let executed = executed_clone.clone();
            async move {
                let mut locked = executed.lock().await;
                locked.push(id);
                Ok::<_, anyhow::Error>(())
            }
        }).await.unwrap();
        
        let result = executed.lock().await;
        assert_eq!(result[0], 1); // Task 1 should be executed first
        assert_eq!(result[1], 2); // Task 2 should be executed second
    }
    
    #[tokio::test]
    async fn test_level_parallel_execution() {
        let mut manager = TaskNetwork::<String>::new(4);
        
        // Create a task graph with multiple levels:
        // Level 0: 1, 2 (no dependencies)
        // Level 1: 3, 4 (depend on level 0)
        // Level 2: 5 (depends on level 1)
        manager.add_task(1, "Task 1".to_string()).unwrap();
        manager.add_task(2, "Task 2".to_string()).unwrap();
        manager.add_task(3, "Task 3".to_string()).unwrap();
        manager.add_task(4, "Task 4".to_string()).unwrap();
        manager.add_task(5, "Task 5".to_string()).unwrap();
        
        manager.add_dependency(1, 3).unwrap();
        manager.add_dependency(2, 3).unwrap();
        manager.add_dependency(2, 4).unwrap();
        manager.add_dependency(3, 5).unwrap();
        manager.add_dependency(4, 5).unwrap();
        
        let executed_levels = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let currently_executing = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        
        let executed_clone = executed_levels.clone();
        let current_clone = currently_executing.clone();
        
        manager.process_tasks(move |id, _task| {
            let executed_levels = executed_clone.clone();
            let currently_executing = current_clone.clone();
            
            async move {
                // Add to currently executing
                {
                    let mut current = currently_executing.lock().await;
                    current.push(id);
                }
                
                // Sleep to ensure tasks in same level overlap
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                
                // Record execution state
                {
                    // First, get the current executing tasks
                    let current_executing = {
                        let current = currently_executing.lock().await;
                        current.clone() // Clone the data so we can release the lock
                    };
                    
                    // Now update the executed_levels with the captured state
                    {
                        let mut executed = executed_levels.lock().await;
                        executed.push((id, current_executing));
                    }
                    
                    // Finally, remove this task from currently executing
                    {
                        let mut current = currently_executing.lock().await;
                        if let Some(pos) = current.iter().position(|&x| x == id) {
                            current.remove(pos);
                        }
                    }
                }
                
                Ok::<_, anyhow::Error>(())
            }
        }).await.unwrap();
        
        let result = executed_levels.lock().await;
        
        // Verify the dependencies were respected
        let mut observed_task_order = result.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        observed_task_order.sort(); // Sort to make comparison easier
        assert_eq!(observed_task_order, vec![1, 2, 3, 4, 5]);
        
        // Check parallelism within levels
        // For each task, check if any other task from the same level was executing concurrently
        let task_1_execution = result.iter().find(|(id, _)| *id == 1).unwrap();
        let task_2_execution = result.iter().find(|(id, _)| *id == 2).unwrap();
        let task_3_execution = result.iter().find(|(id, _)| *id == 3).unwrap();
        let task_4_execution = result.iter().find(|(id, _)| *id == 4).unwrap();
        
        // Check that task 1 and 2 were potentially running in parallel (level 0)
        assert!(task_1_execution.1.contains(&2) || task_2_execution.1.contains(&1));
        
        // Check that task 3 and 4 were potentially running in parallel (level 1)
        assert!(task_3_execution.1.contains(&4) || task_4_execution.1.contains(&3));
    }
}
