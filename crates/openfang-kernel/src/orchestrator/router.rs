//! Task Router — determines execution strategy based on intent analysis.
//!
//! The TaskRouter analyzes the complexity of a task and routes it to the
//! appropriate execution path: Simple (single hand), Medium (workflow),
//! or Complex (swarm).

use openfang_types::orchestrator::*;

/// Task router that determines execution strategy based on intent analysis.
pub struct TaskRouter;

impl TaskRouter {
    /// Create a new task router.
    pub fn new() -> Self {
        Self
    }

    /// Route a task based on intent analysis and request parameters.
    ///
    /// Determines the execution path (Simple, Medium, or Complex) and
    /// configures appropriate parameters like parallelism, timeout,
    /// retry policy, and error handling mode.
    pub fn route(&self, analysis: &IntentAnalysis, request: &OrchestrationRequest) -> ExecutionStrategy {
        // Use forced complexity if specified, otherwise use analysis result
        let complexity = request.force_complexity.unwrap_or(analysis.complexity);

        let path = match complexity {
            ComplexityLevel::Simple => self.build_simple_path(analysis),
            ComplexityLevel::Medium => self.build_medium_path(analysis, &request.id),
            ComplexityLevel::Complex => self.build_complex_path(analysis, &request.id),
        };

        ExecutionStrategy {
            path,
            max_parallelism: self.determine_parallelism(complexity),
            timeout_seconds: self.determine_timeout(complexity, analysis.estimated_duration),
            retry_policy: self.determine_retry_policy(complexity),
            error_mode: self.determine_error_mode(complexity),
        }
    }

    /// Build execution path for simple tasks (single hand execution).
    fn build_simple_path(&self, analysis: &IntentAnalysis) -> ExecutionPath {
        let hand_id = if let Some(first_hand) = analysis.suggested_hands.first() {
            first_hand.clone()
        } else {
            // Default hand selection based on task type
            match analysis.task_type {
                TaskType::Coding => "browser".to_string(),
                TaskType::Research => "researcher".to_string(),
                _ => {
                    // Use first suggested agent as fallback, or generic default
                    analysis.suggested_agents
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "assistant".to_string())
                }
            }
        };

        ExecutionPath::Simple { hand_id }
    }

    /// Build execution path for medium complexity tasks (workflow).
    fn build_medium_path(&self, analysis: &IntentAnalysis, task_id: &TaskId) -> ExecutionPath {
        let task_id_short = task_id.to_string().split('-').next().unwrap_or("unknown").to_string();
        let workflow_name = format!("auto_{:?}_{}", analysis.task_type, task_id_short);

        // Map suggested agents to step names
        let steps = if analysis.suggested_agents.is_empty() {
            vec!["execute".to_string()]
        } else {
            // Use standard step names based on agent count
            match analysis.suggested_agents.len() {
                1 => vec!["execute".to_string()],
                2 => vec!["analyze".to_string(), "execute".to_string()],
                _ => vec!["analyze".to_string(), "execute".to_string(), "review".to_string()],
            }
        };

        ExecutionPath::Medium {
            workflow_name,
            steps,
        }
    }

    /// Build execution path for complex tasks (swarm).
    fn build_complex_path(&self, analysis: &IntentAnalysis, task_id: &TaskId) -> ExecutionPath {
        let task_id_short = task_id.to_string().split('-').next().unwrap_or("unknown").to_string();
        let swarm_id = format!("swarm_{:?}_{}", analysis.task_type, task_id_short);
        let step_count = analysis.suggested_agents.len().max(3);

        ExecutionPath::Complex {
            swarm_id,
            step_count,
        }
    }

    /// Determine maximum parallelism based on complexity.
    fn determine_parallelism(&self, complexity: ComplexityLevel) -> usize {
        match complexity {
            ComplexityLevel::Simple => 1,
            ComplexityLevel::Medium => 2,
            ComplexityLevel::Complex => 4,
        }
    }

    /// Determine timeout based on complexity and estimated duration.
    fn determine_timeout(&self, complexity: ComplexityLevel, estimated_duration: u64) -> u64 {
        let calculated = estimated_duration.saturating_mul(2);
        let minimum = match complexity {
            ComplexityLevel::Simple => 120,
            ComplexityLevel::Medium => 300,
            ComplexityLevel::Complex => 600,
        };
        calculated.max(minimum)
    }

    /// Determine retry policy based on complexity.
    fn determine_retry_policy(&self, complexity: ComplexityLevel) -> RetryPolicy {
        match complexity {
            ComplexityLevel::Simple => RetryPolicy {
                max_retries: 1,
                backoff_ms: 1000,
            },
            ComplexityLevel::Medium => RetryPolicy {
                max_retries: 2,
                backoff_ms: 2000,
            },
            ComplexityLevel::Complex => RetryPolicy {
                max_retries: 3,
                backoff_ms: 3000,
            },
        }
    }

    /// Determine error handling mode based on complexity.
    fn determine_error_mode(&self, complexity: ComplexityLevel) -> ErrorHandlingMode {
        match complexity {
            ComplexityLevel::Simple => ErrorHandlingMode::FailFast,
            ComplexityLevel::Medium => ErrorHandlingMode::RetryThenFail,
            ComplexityLevel::Complex => ErrorHandlingMode::SkipAndContinue,
        }
    }
}

impl Default for TaskRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> OrchestrationRequest {
        OrchestrationRequest {
            id: TaskId::new(),
            description: "Test task".to_string(),
            channel: None,
            deadline: None,
            force_complexity: None,
            created_at: chrono::Utc::now(),
        }
    }

    fn create_test_analysis(
        task_type: TaskType,
        complexity: ComplexityLevel,
        suggested_agents: Vec<String>,
        suggested_hands: Vec<String>,
        estimated_duration: u64,
    ) -> IntentAnalysis {
        IntentAnalysis {
            task_type,
            complexity,
            confidence: 0.8,
            suggested_agents,
            suggested_hands,
            estimated_duration,
            keywords: vec!["test".to_string()],
        }
    }

    #[test]
    fn test_simple_path_with_suggested_hand() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Coding,
            ComplexityLevel::Simple,
            vec![],
            vec!["custom-hand".to_string()],
            60,
        );

        let strategy = router.route(&analysis, &request);

        assert_eq!(strategy.max_parallelism, 1);
        assert_eq!(strategy.timeout_seconds, 120); // min for simple
        assert_eq!(strategy.retry_policy.max_retries, 1);
        assert_eq!(strategy.retry_policy.backoff_ms, 1000);
        assert!(matches!(strategy.error_mode, ErrorHandlingMode::FailFast));

        match &strategy.path {
            ExecutionPath::Simple { hand_id } => assert_eq!(hand_id, "custom-hand"),
            _ => panic!("Expected Simple path"),
        }
    }

    #[test]
    fn test_simple_path_coding_default() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Coding,
            ComplexityLevel::Simple,
            vec![],
            vec![],
            60,
        );

        let strategy = router.route(&analysis, &request);

        match &strategy.path {
            ExecutionPath::Simple { hand_id } => assert_eq!(hand_id, "browser"),
            _ => panic!("Expected Simple path"),
        }
    }

    #[test]
    fn test_simple_path_research_default() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Research,
            ComplexityLevel::Simple,
            vec![],
            vec![],
            60,
        );

        let strategy = router.route(&analysis, &request);

        match &strategy.path {
            ExecutionPath::Simple { hand_id } => assert_eq!(hand_id, "researcher"),
            _ => panic!("Expected Simple path"),
        }
    }

    #[test]
    fn test_simple_path_fallback_to_agent() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Mixed,
            ComplexityLevel::Simple,
            vec!["custom-agent".to_string()],
            vec![],
            60,
        );

        let strategy = router.route(&analysis, &request);

        match &strategy.path {
            ExecutionPath::Simple { hand_id } => assert_eq!(hand_id, "custom-agent"),
            _ => panic!("Expected Simple path"),
        }
    }

    #[test]
    fn test_medium_path() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Analysis,
            ComplexityLevel::Medium,
            vec!["agent1".to_string(), "agent2".to_string()],
            vec![],
            120,
        );

        let strategy = router.route(&analysis, &request);

        assert_eq!(strategy.max_parallelism, 2);
        assert_eq!(strategy.timeout_seconds, 300); // max(120*2, 300)
        assert_eq!(strategy.retry_policy.max_retries, 2);
        assert_eq!(strategy.retry_policy.backoff_ms, 2000);
        assert!(matches!(strategy.error_mode, ErrorHandlingMode::RetryThenFail));

        match &strategy.path {
            ExecutionPath::Medium { workflow_name, steps } => {
                assert!(workflow_name.starts_with("auto_Analysis_"));
                assert_eq!(steps.len(), 2);
                assert_eq!(steps[0], "analyze");
                assert_eq!(steps[1], "execute");
            }
            _ => panic!("Expected Medium path"),
        }
    }

    #[test]
    fn test_medium_path_three_agents() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Writing,
            ComplexityLevel::Medium,
            vec!["agent1".to_string(), "agent2".to_string(), "agent3".to_string()],
            vec![],
            100,
        );

        let strategy = router.route(&analysis, &request);

        match &strategy.path {
            ExecutionPath::Medium { steps, .. } => {
                assert_eq!(steps.len(), 3);
                assert_eq!(steps[0], "analyze");
                assert_eq!(steps[1], "execute");
                assert_eq!(steps[2], "review");
            }
            _ => panic!("Expected Medium path"),
        }
    }

    #[test]
    fn test_medium_path_empty_agents() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::DevOps,
            ComplexityLevel::Medium,
            vec![],
            vec![],
            100,
        );

        let strategy = router.route(&analysis, &request);

        match &strategy.path {
            ExecutionPath::Medium { steps, .. } => {
                assert_eq!(steps.len(), 1);
                assert_eq!(steps[0], "execute");
            }
            _ => panic!("Expected Medium path"),
        }
    }

    #[test]
    fn test_complex_path() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::DevOps,
            ComplexityLevel::Complex,
            vec!["agent1".to_string(), "agent2".to_string(), "agent3".to_string(), "agent4".to_string()],
            vec![],
            400,
        );

        let strategy = router.route(&analysis, &request);

        assert_eq!(strategy.max_parallelism, 4);
        assert_eq!(strategy.timeout_seconds, 800); // 400 * 2
        assert_eq!(strategy.retry_policy.max_retries, 3);
        assert_eq!(strategy.retry_policy.backoff_ms, 3000);
        assert!(matches!(strategy.error_mode, ErrorHandlingMode::SkipAndContinue));

        match &strategy.path {
            ExecutionPath::Complex { swarm_id, step_count } => {
                assert!(swarm_id.starts_with("swarm_DevOps_"));
                assert_eq!(*step_count, 4);
            }
            _ => panic!("Expected Complex path"),
        }
    }

    #[test]
    fn test_complex_path_minimum_steps() {
        let router = TaskRouter::new();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Communication,
            ComplexityLevel::Complex,
            vec!["agent1".to_string()], // Only 1 agent
            vec![],
            100,
        );

        let strategy = router.route(&analysis, &request);

        match &strategy.path {
            ExecutionPath::Complex { step_count, .. } => {
                assert_eq!(*step_count, 3); // max(1, 3)
            }
            _ => panic!("Expected Complex path"),
        }
    }

    #[test]
    fn test_force_complexity_override() {
        let router = TaskRouter::new();
        let mut request = create_test_request();
        request.force_complexity = Some(ComplexityLevel::Complex);

        let analysis = create_test_analysis(
            TaskType::Coding,
            ComplexityLevel::Simple, // Analysis says simple
            vec![],
            vec![],
            60,
        );

        let strategy = router.route(&analysis, &request);

        // Should use Complex path due to force_complexity override
        assert_eq!(strategy.max_parallelism, 4);
        assert!(matches!(strategy.path, ExecutionPath::Complex { .. }));
    }

    #[test]
    fn test_timeout_calculation() {
        let router = TaskRouter::new();
        let request = create_test_request();

        // Simple with high estimated duration
        let analysis = create_test_analysis(
            TaskType::Coding,
            ComplexityLevel::Simple,
            vec![],
            vec!["hand".to_string()],
            200, // 200 * 2 = 400 > 120
        );
        let strategy = router.route(&analysis, &request);
        assert_eq!(strategy.timeout_seconds, 400);

        // Simple with low estimated duration
        let analysis2 = create_test_analysis(
            TaskType::Coding,
            ComplexityLevel::Simple,
            vec![],
            vec!["hand".to_string()],
            30, // 30 * 2 = 60 < 120
        );
        let strategy2 = router.route(&analysis2, &request);
        assert_eq!(strategy2.timeout_seconds, 120); // minimum
    }

    #[test]
    fn test_default_implementation() {
        let router: TaskRouter = Default::default();
        let request = create_test_request();
        let analysis = create_test_analysis(
            TaskType::Coding,
            ComplexityLevel::Simple,
            vec![],
            vec!["test-hand".to_string()],
            60,
        );

        let strategy = router.route(&analysis, &request);
        assert_eq!(strategy.max_parallelism, 1);
    }
}
