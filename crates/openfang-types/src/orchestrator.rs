//! Orchestrator types: task management, execution strategy, and agent lifecycle.
//!
//! The orchestrator is responsible for analyzing user requests, selecting
//! appropriate execution paths (single agent, workflow, or swarm), and
//! managing agent lifecycles based on usage patterns.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;

// =============================================================================
// UUID Wrapper Types
// =============================================================================

/// Unique identifier for an orchestrator task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generate a new random TaskId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TaskId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Unique identifier for a workflow definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub Uuid);

impl WorkflowId {
    /// Generate a new random WorkflowId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WorkflowId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Unique identifier for a running workflow instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowRunId(pub Uuid);

impl WorkflowRunId {
    /// Generate a new random WorkflowRunId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorkflowRunId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkflowRunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WorkflowRunId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

// =============================================================================
// Enums
// =============================================================================

/// Complexity level for task classification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityLevel {
    /// Simple task: single agent, minimal context.
    Simple,
    /// Medium complexity: multi-step workflow.
    #[default]
    Medium,
    /// Complex task: swarm with parallel execution.
    Complex,
}

/// Type of task being orchestrated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Code writing or modification.
    Coding,
    /// Information gathering and synthesis.
    Research,
    /// Content creation.
    Writing,
    /// Data analysis and insights.
    Analysis,
    /// Infrastructure and deployment.
    DevOps,
    /// Messaging and coordination.
    Communication,
    /// Mixed task type.
    #[default]
    Mixed,
    /// Other/unknown task type.
    Other,
}

/// Execution state of an orchestrated task.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    /// Task is waiting to be processed.
    #[default]
    Pending,
    /// Task is being analyzed for intent.
    Analyzing,
    /// Execution path is being prepared.
    Preparing,
    /// Task is actively running.
    Running,
    /// Monitoring progress and waiting for completion.
    Monitoring,
    /// Adjusting execution strategy.
    Adjusting,
    /// Task is finalizing results.
    Completing,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was cancelled.
    Cancelled,
}

/// Error handling mode for execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorHandlingMode {
    /// Stop immediately on first error.
    FailFast,
    /// Skip failing steps and continue.
    SkipAndContinue,
    /// Retry failed steps, then fail if still failing.
    #[default]
    RetryThenFail,
}

/// Status of a managed agent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedAgentStatus {
    /// Agent is actively processing tasks.
    Active,
    /// Agent is idle and available.
    #[default]
    Idle,
    /// Agent is temporarily suspended.
    Suspended,
    /// Agent is being destroyed.
    Destroying,
}

/// Retention status for agent lifecycle management.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionStatus {
    /// Agent should be permanently retained.
    Permanent,
    /// Agent is cached for potential reuse.
    #[default]
    Cached,
    /// Agent is dynamic and can be destroyed.
    Dynamic,
}

// =============================================================================
// Request and Analysis Types
// =============================================================================

/// A request to orchestrate a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationRequest {
    /// Unique task identifier.
    pub id: TaskId,
    /// User's task description.
    pub description: String,
    /// Channel to send responses to.
    pub channel: Option<String>,
    /// Optional deadline for completion.
    pub deadline: Option<DateTime<Utc>>,
    /// Force a specific complexity level.
    pub force_complexity: Option<ComplexityLevel>,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
}

impl Default for OrchestrationRequest {
    fn default() -> Self {
        Self {
            id: TaskId::new(),
            description: String::new(),
            channel: None,
            deadline: None,
            force_complexity: None,
            created_at: Utc::now(),
        }
    }
}

/// Result of intent analysis on a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAnalysis {
    /// Classified task type.
    pub task_type: TaskType,
    /// Estimated complexity level.
    pub complexity: ComplexityLevel,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Suggested agent templates for execution.
    pub suggested_agents: Vec<String>,
    /// Suggested hands for execution.
    pub suggested_hands: Vec<String>,
    /// Estimated duration in seconds.
    pub estimated_duration: u64,
    /// Extracted keywords from the task.
    pub keywords: Vec<String>,
}

impl Default for IntentAnalysis {
    fn default() -> Self {
        Self {
            task_type: TaskType::default(),
            complexity: ComplexityLevel::default(),
            confidence: 0.5,
            suggested_agents: Vec::new(),
            suggested_hands: Vec::new(),
            estimated_duration: 60,
            keywords: Vec::new(),
        }
    }
}

// =============================================================================
// Execution Strategy Types
// =============================================================================

/// Execution path selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPath {
    /// Simple: single hand/agent execution.
    Simple {
        /// Hand ID to use.
        hand_id: String,
    },
    /// Medium: workflow with sequential steps.
    Medium {
        /// Workflow name.
        workflow_name: String,
        /// Ordered step names.
        steps: Vec<String>,
    },
    /// Complex: swarm with parallel execution.
    Complex {
        /// Swarm definition ID.
        swarm_id: String,
        /// Number of steps in the swarm.
        step_count: usize,
    },
}

/// Retry policy for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Backoff delay in milliseconds.
    pub backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_ms: 1000,
        }
    }
}

/// Complete execution strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStrategy {
    /// Selected execution path.
    pub path: ExecutionPath,
    /// Maximum parallel execution slots.
    pub max_parallelism: usize,
    /// Timeout in seconds.
    pub timeout_seconds: u64,
    /// Retry policy.
    pub retry_policy: RetryPolicy,
    /// Error handling mode.
    pub error_mode: ErrorHandlingMode,
}

impl Default for ExecutionStrategy {
    fn default() -> Self {
        Self {
            path: ExecutionPath::Simple {
                hand_id: "coder".to_string(),
            },
            max_parallelism: 4,
            timeout_seconds: 300,
            retry_policy: RetryPolicy::default(),
            error_mode: ErrorHandlingMode::default(),
        }
    }
}

// =============================================================================
// Execution Context Types
// =============================================================================

/// Runtime execution context for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Task being executed.
    pub task_id: TaskId,
    /// Complexity level determined.
    pub complexity: ComplexityLevel,
    /// Execution strategy being used.
    pub strategy: ExecutionStrategy,
    /// Agents involved in execution.
    pub agents: Vec<AgentId>,
    /// Hands involved in execution.
    pub hands: Vec<String>,
    /// Active workflow run (if using workflow).
    pub workflow_run_id: Option<WorkflowRunId>,
    /// Active swarm execution (if using swarm).
    pub swarm_execution_id: Option<String>,
    /// Current execution state.
    pub state: ExecutionState,
    /// When execution started.
    pub started_at: DateTime<Utc>,
    /// When execution completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Output result.
    pub output: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
}

// =============================================================================
// Agent Management Types
// =============================================================================

/// Metrics for retention decision making.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetentionMetrics {
    /// How frequently the agent is used.
    pub frequency_score: u64,
    /// Historical success rate.
    pub success_rate_score: u64,
    /// Efficiency score.
    pub efficiency_score: u64,
    /// Response speed score.
    pub speed_score: u64,
}

/// Decision about agent retention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionDecision {
    /// Agent being evaluated.
    pub agent_id: AgentId,
    /// Retention decision.
    pub decision: RetentionStatus,
    /// Overall retention score.
    pub score: u64,
    /// Detailed metrics.
    pub metrics: RetentionMetrics,
}

/// Information about a managed agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgent {
    /// Agent's unique ID.
    pub agent_id: AgentId,
    /// Human-readable name.
    pub name: String,
    /// Template used to create this agent.
    pub template_id: Option<String>,
    /// Hand this agent belongs to.
    pub hand_id: Option<String>,
    /// When the agent was created.
    pub created_at: DateTime<Utc>,
    /// When the agent was last used.
    pub last_used_at: DateTime<Utc>,
    /// Number of times used.
    pub use_count: u64,
    /// Number of successful executions.
    pub success_count: u64,
    /// Total tokens consumed.
    pub total_tokens: u64,
    /// Average execution duration in milliseconds.
    pub avg_duration_ms: u64,
    /// Current status.
    pub status: ManagedAgentStatus,
    /// Retention status.
    pub retention: RetentionStatus,
}

// =============================================================================
// Task Metrics and Events
// =============================================================================

/// Metrics collected during task execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    /// Total wall-clock time in milliseconds.
    pub total_duration_ms: u64,
    /// Time spent in actual execution.
    pub execution_duration_ms: u64,
    /// Input tokens used.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Number of agents involved.
    pub agent_count: usize,
    /// Number of steps executed.
    pub step_count: usize,
    /// Number of retries performed.
    pub retry_count: u32,
    /// Number of strategy adjustments.
    pub adjustment_count: u32,
}

/// Action to adjust execution strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AdjustmentAction {
    /// Switch from one agent to another.
    SwitchAgent {
        /// Agent to switch from.
        from: String,
        /// Agent to switch to.
        to: String,
    },
    /// Split task into subtasks.
    SplitTask {
        /// Subtask descriptions.
        subtasks: Vec<String>,
    },
    /// Increase retry count for a step.
    IncreaseRetry {
        /// Step ID to modify.
        step_id: String,
        /// New max retries.
        max_retries: u32,
    },
    /// Change execution path entirely.
    ChangePath {
        /// New execution path.
        new_path: ExecutionPath,
    },
    /// Parallelize specific steps.
    Parallelize {
        /// Step IDs to parallelize.
        step_ids: Vec<String>,
    },
    /// Request human intervention.
    RequestHumanIntervention {
        /// Reason for intervention.
        reason: String,
    },
}

/// Event emitted during orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestrationEvent {
    /// Intent analysis started.
    AnalysisStarted {
        /// Task being analyzed.
        task_id: TaskId,
    },
    /// Intent analysis completed.
    AnalysisCompleted {
        /// Task analyzed.
        task_id: TaskId,
        /// Analysis results.
        analysis: IntentAnalysis,
    },
    /// Execution started.
    ExecutionStarted {
        /// Task being executed.
        task_id: TaskId,
        /// Execution path selected.
        path: ExecutionPath,
    },
    /// Progress update.
    ProgressUpdated {
        /// Task in progress.
        task_id: TaskId,
        /// Progress percentage (0.0 - 1.0).
        progress: f64,
        /// Human-readable message.
        message: String,
    },
    /// Milestone reached.
    MilestoneReached {
        /// Task with milestone.
        task_id: TaskId,
        /// Milestone description.
        milestone: String,
    },
    /// New agent created for task.
    AgentCreated {
        /// Task requiring agent.
        task_id: TaskId,
        /// Created agent ID.
        agent_id: AgentId,
        /// Reason for creation.
        reason: String,
    },
    /// Adjustment needed.
    AdjustmentNeeded {
        /// Task needing adjustment.
        task_id: TaskId,
        /// Why adjustment is needed.
        reason: String,
        /// Suggested action.
        suggested_action: AdjustmentAction,
    },
    /// Adjustment applied.
    AdjustmentApplied {
        /// Task adjusted.
        task_id: TaskId,
        /// Action taken.
        action: AdjustmentAction,
    },
    /// Task completed successfully.
    TaskCompleted {
        /// Task that completed.
        task_id: TaskId,
        /// Output result.
        output: String,
        /// Execution metrics.
        metrics: TaskMetrics,
    },
    /// Task failed.
    TaskFailed {
        /// Task that failed.
        task_id: TaskId,
        /// Error message.
        error: String,
        /// Whether recovery is possible.
        recoverable: bool,
    },
    /// Task cancelled.
    TaskCancelled {
        /// Task that was cancelled.
        task_id: TaskId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_uniqueness() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new();
        let display = format!("{}", id);
        assert_eq!(display.len(), 36); // UUID v4 string length
    }

    #[test]
    fn test_task_id_serialization() {
        let id = TaskId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_task_id_from_str() {
        let id = TaskId::new();
        let s = id.to_string();
        let parsed: TaskId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_workflow_id_uniqueness() {
        let id1 = WorkflowId::new();
        let id2 = WorkflowId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_workflow_run_id_uniqueness() {
        let id1 = WorkflowRunId::new();
        let id2 = WorkflowRunId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_complexity_level_default() {
        let level = ComplexityLevel::default();
        assert!(matches!(level, ComplexityLevel::Medium));
    }

    #[test]
    fn test_complexity_level_serde() {
        let level = ComplexityLevel::Complex;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"complex\"");
        let back: ComplexityLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ComplexityLevel::Complex);
    }

    #[test]
    fn test_task_type_default() {
        let t = TaskType::default();
        assert!(matches!(t, TaskType::Mixed));
    }

    #[test]
    fn test_execution_state_default() {
        let state = ExecutionState::default();
        assert!(matches!(state, ExecutionState::Pending));
    }

    #[test]
    fn test_error_handling_mode_default() {
        let mode = ErrorHandlingMode::default();
        assert!(matches!(mode, ErrorHandlingMode::RetryThenFail));
    }

    #[test]
    fn test_orchestration_request_default() {
        let req = OrchestrationRequest::default();
        assert!(req.description.is_empty());
        assert!(req.channel.is_none());
        assert!(req.deadline.is_none());
        assert!(req.force_complexity.is_none());
    }

    #[test]
    fn test_intent_analysis_default() {
        let analysis = IntentAnalysis::default();
        assert!(matches!(analysis.task_type, TaskType::Mixed));
        assert!(matches!(analysis.complexity, ComplexityLevel::Medium));
        assert_eq!(analysis.confidence, 0.5);
        assert!(analysis.suggested_agents.is_empty());
        assert!(analysis.suggested_hands.is_empty());
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.backoff_ms, 1000);
    }

    #[test]
    fn test_execution_strategy_default() {
        let strategy = ExecutionStrategy::default();
        assert_eq!(strategy.max_parallelism, 4);
        assert_eq!(strategy.timeout_seconds, 300);
    }

    #[test]
    fn test_execution_path_simple_serde() {
        let path = ExecutionPath::Simple {
            hand_id: "coder".to_string(),
        };
        let json = serde_json::to_string(&path).unwrap();
        assert!(json.contains("\"type\":\"simple\""));
        assert!(json.contains("\"hand_id\":\"coder\""));
        let back: ExecutionPath = serde_json::from_str(&json).unwrap();
        match back {
            ExecutionPath::Simple { hand_id } => assert_eq!(hand_id, "coder"),
            _ => panic!("Expected Simple variant"),
        }
    }

    #[test]
    fn test_execution_path_medium_serde() {
        let path = ExecutionPath::Medium {
            workflow_name: "review-flow".to_string(),
            steps: vec!["analyze".to_string(), "review".to_string()],
        };
        let json = serde_json::to_string(&path).unwrap();
        let back: ExecutionPath = serde_json::from_str(&json).unwrap();
        match back {
            ExecutionPath::Medium { workflow_name, steps } => {
                assert_eq!(workflow_name, "review-flow");
                assert_eq!(steps.len(), 2);
            }
            _ => panic!("Expected Medium variant"),
        }
    }

    #[test]
    fn test_retention_metrics_default() {
        let metrics = RetentionMetrics::default();
        assert_eq!(metrics.frequency_score, 0);
        assert_eq!(metrics.success_rate_score, 0);
    }

    #[test]
    fn test_task_metrics_default() {
        let metrics = TaskMetrics::default();
        assert_eq!(metrics.total_duration_ms, 0);
        assert_eq!(metrics.execution_duration_ms, 0);
        assert_eq!(metrics.retry_count, 0);
    }

    #[test]
    fn test_adjustment_action_switch_agent_serde() {
        let action = AdjustmentAction::SwitchAgent {
            from: "agent-a".to_string(),
            to: "agent-b".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: AdjustmentAction = serde_json::from_str(&json).unwrap();
        match back {
            AdjustmentAction::SwitchAgent { from, to } => {
                assert_eq!(from, "agent-a");
                assert_eq!(to, "agent-b");
            }
            _ => panic!("Expected SwitchAgent variant"),
        }
    }

    #[test]
    fn test_orchestration_event_analysis_started() {
        let task_id = TaskId::new();
        let event = OrchestrationEvent::AnalysisStarted { task_id };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"analysis_started\""));
        let back: OrchestrationEvent = serde_json::from_str(&json).unwrap();
        match back {
            OrchestrationEvent::AnalysisStarted { task_id: tid } => {
                assert_eq!(tid, task_id);
            }
            _ => panic!("Expected AnalysisStarted variant"),
        }
    }

    #[test]
    fn test_orchestration_event_progress_updated() {
        let task_id = TaskId::new();
        let event = OrchestrationEvent::ProgressUpdated {
            task_id,
            progress: 0.5,
            message: "Halfway done".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: OrchestrationEvent = serde_json::from_str(&json).unwrap();
        match back {
            OrchestrationEvent::ProgressUpdated {
                progress,
                message,
                ..
            } => {
                assert!((progress - 0.5).abs() < 0.001);
                assert_eq!(message, "Halfway done");
            }
            _ => panic!("Expected ProgressUpdated variant"),
        }
    }

    #[test]
    fn test_orchestration_event_task_completed() {
        let task_id = TaskId::new();
        let metrics = TaskMetrics {
            total_duration_ms: 1000,
            input_tokens: 100,
            output_tokens: 200,
            ..Default::default()
        };
        let event = OrchestrationEvent::TaskCompleted {
            task_id,
            output: "Done".to_string(),
            metrics: metrics.clone(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: OrchestrationEvent = serde_json::from_str(&json).unwrap();
        match back {
            OrchestrationEvent::TaskCompleted {
                output,
                metrics: m,
                ..
            } => {
                assert_eq!(output, "Done");
                assert_eq!(m.total_duration_ms, 1000);
                assert_eq!(m.input_tokens, 100);
                assert_eq!(m.output_tokens, 200);
            }
            _ => panic!("Expected TaskCompleted variant"),
        }
    }

    #[test]
    fn test_managed_agent_status_default() {
        let status = ManagedAgentStatus::default();
        assert!(matches!(status, ManagedAgentStatus::Idle));
    }

    #[test]
    fn test_retention_status_default() {
        let status = RetentionStatus::default();
        assert!(matches!(status, RetentionStatus::Cached));
    }
}
