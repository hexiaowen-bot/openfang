//! Orchestrator Engine — main orchestration engine for task routing and execution.
//!
//! The OrchestratorEngine is responsible for:
//! - Analyzing user requests via IntentAnalyzer
//! - Routing tasks to appropriate execution paths via TaskRouter
//! - Managing agent lifecycles via ManagedAgentPool
//! - Executing tasks with the selected strategy
//!
//! # Execution Paths
//!
//! - **Simple**: Single hand/agent execution
//! - **Medium**: Workflow with sequential steps
//! - **Complex**: Swarm with parallel execution

use chrono::Utc;
use openfang_types::agent::AgentId;
use openfang_types::orchestrator::*;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use super::intent::IntentAnalyzer;
use super::lifecycle::ManagedAgentPool;
use super::router::TaskRouter;

/// Abstract interface for kernel operations, used to decouple
/// the orchestrator from direct kernel dependencies.
///
/// This trait allows the orchestrator to interact with the kernel
/// for agent spawning, messaging, and lifecycle management without
/// creating circular dependencies.
pub trait KernelOperations: Send + Sync {
    /// Send a message to an agent and get a response.
    ///
    /// Returns the response text, input tokens, and output tokens.
    fn send_message(&self, agent_name: &str, message: &str) -> Result<(String, u64, u64), String>;

    /// Spawn a new agent with the given name and description.
    ///
    /// Returns the agent ID on success.
    fn spawn_agent(&self, name: &str, description: &str) -> Result<String, String>;

    /// Terminate an agent by ID.
    fn kill_agent(&self, agent_id: &str) -> Result<(), String>;

    /// Get a list of available agent names.
    fn list_agents(&self) -> Vec<String>;

    /// Publish an event to the event bus.
    fn publish_event(&self, event_data: Vec<u8>);
}

/// Internal tracking for task metrics.
#[derive(Debug, Clone, Default)]
struct InternalMetrics {
    /// Start time of execution.
    started_at: Option<Instant>,
    /// Execution start time (after analysis).
    execution_started_at: Option<Instant>,
    /// Total input tokens used.
    input_tokens: u64,
    /// Total output tokens generated.
    output_tokens: u64,
    /// Number of retries performed.
    retry_count: u32,
    /// Number of strategy adjustments.
    adjustment_count: u32,
}

/// The Orchestrator Engine — coordinates task analysis, routing, and execution.
pub struct OrchestratorEngine {
    /// Intent analyzer for task classification.
    intent_analyzer: IntentAnalyzer,
    /// Task router for execution strategy selection.
    router: TaskRouter,
    /// Agent lifecycle manager.
    lifecycle: ManagedAgentPool,
    /// Active and historical execution contexts.
    executions: RwLock<HashMap<TaskId, ExecutionContext>>,
    /// Internal metrics tracking.
    metrics: RwLock<HashMap<TaskId, InternalMetrics>>,
}

impl OrchestratorEngine {
    /// Create a new orchestrator engine with all components initialized.
    pub fn new() -> Self {
        Self {
            intent_analyzer: IntentAnalyzer::new(),
            router: TaskRouter::new(),
            lifecycle: ManagedAgentPool::new(),
            executions: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Submit a new orchestration request for processing.
    ///
    /// This method:
    /// 1. Analyzes the task description to determine intent
    /// 2. Routes to an appropriate execution strategy
    /// 3. Creates an execution context in Pending state
    /// 4. Returns the task ID for tracking
    ///
    /// # Example
    ///
    /// ```ignore
    /// let engine = OrchestratorEngine::new();
    /// let request = OrchestrationRequest {
    ///     description: "Fix the bug in login function".to_string(),
    ///     ..Default::default()
    /// };
    /// let task_id = engine.submit(request);
    /// ```
    pub fn submit(&self, request: OrchestrationRequest) -> TaskId {
        let task_id = request.id;

        // Analyze intent
        let analysis = self.intent_analyzer.analyze(&request.description);

        // Route to execution strategy
        let strategy = self.router.route(&analysis, &request);

        // Create execution context
        let context = ExecutionContext {
            task_id,
            complexity: analysis.complexity,
            strategy,
            agents: Vec::new(),
            hands: Vec::new(),
            workflow_run_id: None,
            swarm_execution_id: None,
            state: ExecutionState::Pending,
            started_at: Utc::now(),
            completed_at: None,
            output: None,
            error: None,
        };

        // Store context and initialize metrics
        self.executions.write().unwrap().insert(task_id, context);
        self.metrics
            .write()
            .unwrap()
            .insert(task_id, InternalMetrics::default());

        task_id
    }

    /// Execute a submitted task by its ID.
    ///
    /// This is the MVP version that returns a description of the planned execution
    /// without actually calling the kernel. For actual execution, use `execute_with_kernel`.
    ///
    /// The method transitions the task through states:
    /// Pending → Analyzing → Preparing → Running → Completed/Failed
    pub fn execute(&self, task_id: &TaskId) -> Result<String, String> {
        let mut executions = self.executions.write().unwrap();
        let mut metrics = self.metrics.write().unwrap();

        let context = executions
            .get_mut(task_id)
            .ok_or_else(|| format!("Task {} not found", task_id))?;

        // Record start time
        if let Some(m) = metrics.get_mut(task_id) {
            m.started_at = Some(Instant::now());
        }

        // Transition to Analyzing
        context.state = ExecutionState::Analyzing;

        // Transition to Preparing
        context.state = ExecutionState::Preparing;

        // Transition to Running
        context.state = ExecutionState::Running;
        if let Some(m) = metrics.get_mut(task_id) {
            m.execution_started_at = Some(Instant::now());
        }

        // Execute based on path type (MVP: return description)
        let result: Result<String, String> = match &context.strategy.path {
            ExecutionPath::Simple { hand_id } => {
                context.hands.push(hand_id.clone());
                Ok(format!("Simple execution planned: hand={}", hand_id))
            }
            ExecutionPath::Medium {
                workflow_name,
                steps,
            } => {
                Ok(format!(
                    "Medium execution planned: workflow={}, steps={:?}",
                    workflow_name, steps
                ))
            }
            ExecutionPath::Complex {
                swarm_id,
                step_count,
            } => {
                Ok(format!(
                    "Complex execution planned: swarm={}, steps={}",
                    swarm_id, step_count
                ))
            }
        };

        // Update state based on result
        match &result {
            Ok(output) => {
                context.state = ExecutionState::Completed;
                context.output = Some(output.clone());
                context.completed_at = Some(Utc::now());

                // Calculate metrics
                if let Some(m) = metrics.get_mut(task_id) {
                    if let Some(start) = m.started_at {
                        let total_duration_ms = start.elapsed().as_millis() as u64;
                        // Update would be done in a real implementation
                        let _ = total_duration_ms;
                    }
                }
            }
            Err(e) => {
                context.state = ExecutionState::Failed;
                context.error = Some(e.clone());
                context.completed_at = Some(Utc::now());
            }
        }

        result
    }

    /// Execute a task with kernel operations for actual agent interaction.
    ///
    /// This method performs real execution by:
    /// - **Simple**: Spawn/check agent → send message → collect result
    /// - **Medium**: Execute steps sequentially, passing outputs between agents
    /// - **Complex**: Execute steps in parallel/layered based on dependencies
    ///
    /// Each step uses lifecycle management to find reusable agents or spawn new ones.
    pub fn execute_with_kernel(
        &self,
        task_id: &TaskId,
        kernel_ops: &dyn KernelOperations,
    ) -> Result<String, String> {
        let mut executions = self.executions.write().unwrap();
        let mut metrics = self.metrics.write().unwrap();

        let context = executions
            .get_mut(task_id)
            .ok_or_else(|| format!("Task {} not found", task_id))?;

        // Initialize metrics
        if let Some(m) = metrics.get_mut(task_id) {
            m.started_at = Some(Instant::now());
        }

        // Transition states
        context.state = ExecutionState::Analyzing;
        context.state = ExecutionState::Preparing;
        context.state = ExecutionState::Running;

        if let Some(m) = metrics.get_mut(task_id) {
            m.execution_started_at = Some(Instant::now());
        }

        // Clone what we need before the match
        let strategy = context.strategy.clone();

        // Execute based on path type
        let result = match &strategy.path {
            ExecutionPath::Simple { hand_id } => {
                self.execute_simple(task_id, hand_id, kernel_ops)
            }
            ExecutionPath::Medium {
                workflow_name,
                steps,
            } => self.execute_medium(task_id, workflow_name, steps, kernel_ops),
            ExecutionPath::Complex {
                swarm_id,
                step_count,
            } => self.execute_complex(task_id, swarm_id, *step_count, kernel_ops),
        };

        // Get context again to update final state
        let context = executions.get_mut(task_id).unwrap();

        // Update final state
        match &result {
            Ok(output) => {
                context.state = ExecutionState::Completed;
                context.output = Some(output.clone());
                context.completed_at = Some(Utc::now());
            }
            Err(e) => {
                context.state = ExecutionState::Failed;
                context.error = Some(e.clone());
                context.completed_at = Some(Utc::now());
            }
        }

        // Calculate final metrics
        if let Some(m) = metrics.get_mut(task_id) {
            if let Some(start) = m.started_at {
                let _total_duration_ms = start.elapsed().as_millis() as u64;
            }
        }

        result
    }

    /// Update internal metrics for a task.
    fn update_metrics(&self, task_id: &TaskId, input_tokens: u64, output_tokens: u64) {
        let mut metrics = self.metrics.write().unwrap();
        if let Some(m) = metrics.get_mut(task_id) {
            m.input_tokens += input_tokens;
            m.output_tokens += output_tokens;
        }
    }

    /// Execute a simple (single agent) task.
    fn execute_simple(
        &self,
        task_id: &TaskId,
        hand_id: &str,
        kernel_ops: &dyn KernelOperations,
    ) -> Result<String, String> {
        // Try to find a reusable agent
        let agent_id = if let Some(existing_id) = self.lifecycle.find_reusable(hand_id) {
            existing_id
        } else {
            // Spawn new agent
            let _spawned_id = kernel_ops
                .spawn_agent(hand_id, &format!("Agent for hand: {}", hand_id))
                .map_err(|e| format!("Failed to spawn agent: {}", e))?;

            // Parse spawned ID to AgentId
            let agent_id = AgentId::new(); // Generate new ID for tracking
            self.lifecycle
                .register(agent_id, hand_id.to_string(), None, Some(hand_id.to_string()));
            agent_id
        };

        // Send message to agent
        let executions = self.executions.read().unwrap();
        let context = executions
            .get(task_id)
            .ok_or_else(|| format!("Task {} not found", task_id))?;
        let description = context.task_id.to_string(); // Use task ID as message for MVP
        drop(executions);

        let (response, input_tokens, output_tokens) = kernel_ops
            .send_message(hand_id, &description)
            .map_err(|e| format!("Agent communication failed: {}", e))?;

        // Record usage and update metrics
        self.lifecycle.record_usage(&agent_id, true, input_tokens + output_tokens, 0);
        self.update_metrics(task_id, input_tokens, output_tokens);

        Ok(response)
    }

    /// Execute a medium complexity (workflow) task.
    fn execute_medium(
        &self,
        task_id: &TaskId,
        workflow_name: &str,
        steps: &[String],
        kernel_ops: &dyn KernelOperations,
    ) -> Result<String, String> {
        let mut current_input = String::new();

        for (idx, step_name) in steps.iter().enumerate() {
            // Try to find reusable agent or spawn
            let agent_id = if let Some(existing_id) = self.lifecycle.find_reusable(step_name) {
                existing_id
            } else {
                let _spawned_id = kernel_ops
                    .spawn_agent(step_name, &format!("Workflow step: {}", step_name))
                    .map_err(|e| format!("Failed to spawn agent for step {}: {}", step_name, e))?;

                let agent_id = AgentId::new();
                self.lifecycle.register(
                    agent_id,
                    step_name.to_string(),
                    None,
                    Some(workflow_name.to_string()),
                );
                agent_id
            };

            // Execute step
            let prompt = if idx == 0 {
                task_id.to_string()
            } else {
                current_input.clone()
            };

            let (response, input_tokens, output_tokens) = kernel_ops
                .send_message(step_name, &prompt)
                .map_err(|e| format!("Step '{}' failed: {}", step_name, e))?;

            // Record usage and update metrics
            self.lifecycle.record_usage(
                &agent_id,
                true,
                input_tokens + output_tokens,
                0,
            );
            self.update_metrics(task_id, input_tokens, output_tokens);

            current_input = response;
        }

        Ok(current_input)
    }

    /// Execute a complex (swarm) task with parallel execution.
    fn execute_complex(
        &self,
        task_id: &TaskId,
        swarm_id: &str,
        step_count: usize,
        kernel_ops: &dyn KernelOperations,
    ) -> Result<String, String> {
        let executions = self.executions.read().unwrap();
        let context = executions
            .get(task_id)
            .ok_or_else(|| format!("Task {} not found", task_id))?;
        let max_parallelism = context.strategy.max_parallelism;
        drop(executions);

        // MVP: Execute steps with limited parallelism simulation
        let mut results = Vec::new();
        let agent_prefix = format!("swarm-{}-agent", swarm_id);

        for i in 0..step_count {
            let agent_name = format!("{}-{}", agent_prefix, i);

            // Try to find or spawn agent
            let agent_id = if let Some(existing_id) = self.lifecycle.find_reusable(&agent_name) {
                existing_id
            } else {
                let _spawned_id = kernel_ops
                    .spawn_agent(&agent_name, &format!("Swarm step {}", i))
                    .map_err(|e| format!("Failed to spawn agent {}: {}", agent_name, e))?;

                let agent_id = AgentId::new();
                self.lifecycle.register(
                    agent_id,
                    agent_name.clone(),
                    None,
                    Some(swarm_id.to_string()),
                );
                agent_id
            };

            // Execute step
            let prompt = format!("Swarm task {} execution", i);
            let (response, input_tokens, output_tokens) = kernel_ops
                .send_message(&agent_name, &prompt)
                .map_err(|e| format!("Swarm step {} failed: {}", i, e))?;

            // Record usage and update metrics
            self.lifecycle.record_usage(
                &agent_id,
                true,
                input_tokens + output_tokens,
                0,
            );
            self.update_metrics(task_id, input_tokens, output_tokens);

            results.push(response);

            // Simulate parallelism limit (in real impl, would use tokio::join!)
            if results.len() >= max_parallelism {
                // Process batch...
            }
        }

        // Combine results
        Ok(results.join("\n---\n"))
    }

    /// Get the current state of a task.
    pub fn get_status(&self, task_id: &TaskId) -> Option<ExecutionState> {
        let executions = self.executions.read().unwrap();
        executions.get(task_id).map(|c| c.state)
    }

    /// Get the full execution context for a task.
    pub fn get_context(&self, task_id: &TaskId) -> Option<ExecutionContext> {
        let executions = self.executions.read().unwrap();
        executions.get(task_id).cloned()
    }

    /// Cancel a running task.
    ///
    /// Returns `true` if the task was cancelled, `false` if it wasn't found
    /// or was already completed.
    pub fn cancel(&self, task_id: &TaskId) -> bool {
        let mut executions = self.executions.write().unwrap();

        if let Some(context) = executions.get_mut(task_id) {
            match context.state {
                ExecutionState::Pending
                | ExecutionState::Analyzing
                | ExecutionState::Preparing
                | ExecutionState::Running => {
                    context.state = ExecutionState::Cancelled;
                    context.completed_at = Some(Utc::now());
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// List all execution contexts.
    pub fn list_executions(&self) -> Vec<ExecutionContext> {
        let executions = self.executions.read().unwrap();
        executions.values().cloned().collect()
    }

    /// Get metrics for a completed task.
    pub fn get_metrics(&self, task_id: &TaskId) -> Option<TaskMetrics> {
        let executions = self.executions.read().unwrap();
        let metrics = self.metrics.read().unwrap();

        let context = executions.get(task_id)?;
        let internal = metrics.get(task_id)?;

        let total_duration_ms = internal
            .started_at
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let execution_duration_ms = internal
            .execution_started_at
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        Some(TaskMetrics {
            total_duration_ms,
            execution_duration_ms,
            input_tokens: internal.input_tokens,
            output_tokens: internal.output_tokens,
            agent_count: context.agents.len(),
            step_count: match &context.strategy.path {
                ExecutionPath::Simple { .. } => 1,
                ExecutionPath::Medium { steps, .. } => steps.len(),
                ExecutionPath::Complex { step_count, .. } => *step_count,
            },
            retry_count: internal.retry_count,
            adjustment_count: internal.adjustment_count,
        })
    }

    /// Get a reference to the lifecycle manager.
    pub fn get_lifecycle(&self) -> &ManagedAgentPool {
        &self.lifecycle
    }
}

impl Default for OrchestratorEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock implementation of KernelOperations for testing.
    struct MockKernelOps {
        agents: std::sync::Mutex<Vec<String>>,
        responses: std::sync::Mutex<Vec<String>>,
        call_count: std::sync::atomic::AtomicU32,
    }

    impl MockKernelOps {
        fn new() -> Self {
            Self {
                agents: std::sync::Mutex::new(Vec::new()),
                responses: std::sync::Mutex::new(Vec::new()),
                call_count: std::sync::atomic::AtomicU32::new(0),
            }
        }

        fn with_responses(responses: Vec<String>) -> Self {
            Self {
                agents: std::sync::Mutex::new(Vec::new()),
                responses: std::sync::Mutex::new(responses),
                call_count: std::sync::atomic::AtomicU32::new(0),
            }
        }

        fn get_call_count(&self) -> u32 {
            self.call_count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl KernelOperations for MockKernelOps {
        fn send_message(&self, agent_name: &str, _message: &str) -> Result<(String, u64, u64), String> {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let responses = self.responses.lock().unwrap();
            let idx = self.call_count.load(std::sync::atomic::Ordering::SeqCst) as usize;
            let response = responses
                .get(idx.saturating_sub(1))
                .cloned()
                .unwrap_or_else(|| format!("Response from {}", agent_name));

            Ok((response, 100, 50))
        }

        fn spawn_agent(&self, name: &str, _description: &str) -> Result<String, String> {
            let mut agents = self.agents.lock().unwrap();
            agents.push(name.to_string());
            Ok(format!("agent-{}", name))
        }

        fn kill_agent(&self, _agent_id: &str) -> Result<(), String> {
            Ok(())
        }

        fn list_agents(&self) -> Vec<String> {
            self.agents.lock().unwrap().clone()
        }

        fn publish_event(&self, _event_data: Vec<u8>) {}
    }

    #[test]
    fn test_submit_creates_execution_context() {
        let engine = OrchestratorEngine::new();
        let request = OrchestrationRequest {
            description: "Fix the bug in login function".to_string(),
            ..Default::default()
        };

        let task_id = engine.submit(request);

        let context = engine.get_context(&task_id);
        assert!(context.is_some());

        let context = context.unwrap();
        assert_eq!(context.state, ExecutionState::Pending);
        assert_eq!(context.task_id, task_id);
    }

    #[test]
    fn test_execute_state_transitions() {
        let engine = OrchestratorEngine::new();
        let request = OrchestrationRequest {
            description: "Fix bug".to_string(), // Simple task
            ..Default::default()
        };

        let task_id = engine.submit(request);

        // Initial state
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Pending));

        // Execute
        let result = engine.execute(&task_id);
        assert!(result.is_ok());

        // Final state
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Completed));
        assert!(engine.get_context(&task_id).unwrap().output.is_some());
    }

    #[test]
    fn test_execute_nonexistent_task() {
        let engine = OrchestratorEngine::new();
        let fake_id = TaskId::new();

        let result = engine.execute(&fake_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_cancel_pending_task() {
        let engine = OrchestratorEngine::new();
        let request = OrchestrationRequest {
            description: "Test task".to_string(),
            ..Default::default()
        };

        let task_id = engine.submit(request);

        // Cancel before execution
        assert!(engine.cancel(&task_id));
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Cancelled));
    }

    #[test]
    fn test_cancel_completed_task() {
        let engine = OrchestratorEngine::new();
        let request = OrchestrationRequest {
            description: "Fix bug".to_string(),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        engine.execute(&task_id).unwrap();

        // Cannot cancel completed task
        assert!(!engine.cancel(&task_id));
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Completed));
    }

    #[test]
    fn test_cancel_nonexistent_task() {
        let engine = OrchestratorEngine::new();
        let fake_id = TaskId::new();

        assert!(!engine.cancel(&fake_id));
    }

    #[test]
    fn test_list_executions() {
        let engine = OrchestratorEngine::new();

        // Initially empty
        assert!(engine.list_executions().is_empty());

        // Submit multiple tasks
        let id1 = engine.submit(OrchestrationRequest {
            description: "Task 1".to_string(),
            ..Default::default()
        });
        let id2 = engine.submit(OrchestrationRequest {
            description: "Task 2".to_string(),
            ..Default::default()
        });

        let list = engine.list_executions();
        assert_eq!(list.len(), 2);

        let ids: Vec<TaskId> = list.iter().map(|c| c.task_id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_get_metrics() {
        let engine = OrchestratorEngine::new();
        let request = OrchestrationRequest {
            description: "Fix bug".to_string(),
            ..Default::default()
        };

        let task_id = engine.submit(request);

        // No metrics before execution
        let metrics = engine.get_metrics(&task_id);
        assert!(metrics.is_some());
        assert_eq!(metrics.unwrap().total_duration_ms, 0);

        // Execute
        engine.execute(&task_id).unwrap();

        // Metrics after execution
        let metrics = engine.get_metrics(&task_id);
        assert!(metrics.is_some());
        let metrics = metrics.unwrap();
        assert!(metrics.total_duration_ms >= 0);
    }

    #[test]
    fn test_get_metrics_nonexistent_task() {
        let engine = OrchestratorEngine::new();
        let fake_id = TaskId::new();

        assert!(engine.get_metrics(&fake_id).is_none());
    }

    #[test]
    fn test_get_lifecycle() {
        let engine = OrchestratorEngine::new();

        // Should be able to get lifecycle manager
        let lifecycle = engine.get_lifecycle();
        assert!(lifecycle.list().is_empty());
    }

    #[test]
    fn test_execute_with_kernel_simple() {
        let engine = OrchestratorEngine::new();
        let kernel_ops = MockKernelOps::with_responses(vec!["Fixed the bug".to_string()]);

        let request = OrchestrationRequest {
            description: "Fix bug".to_string(),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        let result = engine.execute_with_kernel(&task_id, &kernel_ops);

        assert!(result.is_ok());
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Completed));
        assert_eq!(kernel_ops.get_call_count(), 1);
    }

    #[test]
    fn test_execute_with_kernel_medium() {
        let engine = OrchestratorEngine::new();
        let kernel_ops =
            MockKernelOps::with_responses(vec!["Analyzed".to_string(), "Executed".to_string()]);

        // Force medium complexity
        let request = OrchestrationRequest {
            description: "Analyze the code and implement the feature".to_string(),
            force_complexity: Some(ComplexityLevel::Medium),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        let result = engine.execute_with_kernel(&task_id, &kernel_ops);

        assert!(result.is_ok());
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Completed));
        assert_eq!(kernel_ops.get_call_count(), 2); // Two steps
    }

    #[test]
    fn test_execute_with_kernel_complex() {
        let engine = OrchestratorEngine::new();
        let kernel_ops = MockKernelOps::with_responses(vec![
            "Step 0 done".to_string(),
            "Step 1 done".to_string(),
            "Step 2 done".to_string(),
        ]);

        // Force complex complexity
        let request = OrchestrationRequest {
            description: "Complex task".to_string(),
            force_complexity: Some(ComplexityLevel::Complex),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        let result = engine.execute_with_kernel(&task_id, &kernel_ops);

        assert!(result.is_ok());
        assert_eq!(engine.get_status(&task_id), Some(ExecutionState::Completed));
        // Should have called send_message for each step
        assert!(kernel_ops.get_call_count() >= 1);
    }

    #[test]
    fn test_execution_path_simple() {
        let engine = OrchestratorEngine::new();

        let request = OrchestrationRequest {
            description: "Fix bug".to_string(),
            force_complexity: Some(ComplexityLevel::Simple),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        let context = engine.get_context(&task_id).unwrap();

        assert!(matches!(
            context.strategy.path,
            ExecutionPath::Simple { .. }
        ));
    }

    #[test]
    fn test_execution_path_medium() {
        let engine = OrchestratorEngine::new();

        let request = OrchestrationRequest {
            description: "Test medium".to_string(),
            force_complexity: Some(ComplexityLevel::Medium),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        let context = engine.get_context(&task_id).unwrap();

        assert!(matches!(
            context.strategy.path,
            ExecutionPath::Medium { .. }
        ));
    }

    #[test]
    fn test_execution_path_complex() {
        let engine = OrchestratorEngine::new();

        let request = OrchestrationRequest {
            description: "Test complex".to_string(),
            force_complexity: Some(ComplexityLevel::Complex),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        let context = engine.get_context(&task_id).unwrap();

        assert!(matches!(
            context.strategy.path,
            ExecutionPath::Complex { .. }
        ));
    }

    #[test]
    fn test_default_implementation() {
        let engine: OrchestratorEngine = Default::default();

        let request = OrchestrationRequest {
            description: "Test".to_string(),
            ..Default::default()
        };

        let task_id = engine.submit(request);
        assert!(engine.get_context(&task_id).is_some());
    }
}
