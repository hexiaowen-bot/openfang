//! Swarm configuration and execution state types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Swarm definition loaded from Swarm.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmDefinition {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input: SwarmInput,
    pub steps: Vec<SwarmStep>,
    #[serde(default)]
    pub error_handling: ErrorHandlingConfig,
    #[serde(default)]
    pub settings: SwarmSettings,
}

/// Input specification for a swarm.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwarmInput {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub optional: HashMap<String, serde_json::Value>,
}

/// Single step within a swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStep {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Hand/agent identifier or name.
    pub hand: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub input_mapping: HashMap<String, String>,
    #[serde(default)]
    pub output_mapping: HashMap<String, String>,
    pub condition: Option<String>,
    pub retry: Option<RetryConfig>,
}

/// Retry behaviour for a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_delay_seconds")]
    pub delay_seconds: u32,
    #[serde(default = "default_backoff")]
    pub backoff: String,
}

fn default_max_attempts() -> u32 {
    3
}

fn default_delay_seconds() -> u32 {
    5
}

fn default_backoff() -> String {
    "fixed".to_string()
}

/// Global error handling configuration for a swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    #[serde(default = "default_error_strategy")]
    pub default_strategy: String,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay_seconds")]
    pub retry_delay_seconds: u32,
}

impl Default for ErrorHandlingConfig {
    fn default() -> Self {
        Self {
            default_strategy: default_error_strategy(),
            max_retries: 0,
            retry_delay_seconds: default_retry_delay_seconds(),
        }
    }
}

fn default_error_strategy() -> String {
    "fail".to_string()
}

fn default_retry_delay_seconds() -> u32 {
    5
}

/// Global runtime settings for a swarm.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwarmSettings {
    pub timeout_minutes: Option<u32>,
    pub max_parallel_steps: Option<u32>,
    pub shared_knowledge_namespace: Option<String>,
    pub persist_intermediate_results: Option<bool>,
}

/// Runtime execution record for a swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmExecution {
    pub id: String,
    pub definition_id: String,
    pub input: HashMap<String, serde_json::Value>,
    pub status: SwarmStatus,
    pub step_results: HashMap<String, SwarmStepResult>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Overall swarm execution status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SwarmStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for SwarmStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SwarmStatus::Pending => "pending",
            SwarmStatus::Running => "running",
            SwarmStatus::Completed => "completed",
            SwarmStatus::Failed => "failed",
            SwarmStatus::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

/// Single step execution status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StepStatus::Pending => "pending",
            StepStatus::Running => "running",
            StepStatus::Completed => "completed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
        };
        write!(f, "{}", s)
    }
}

/// Result information for a single swarm step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub output: Option<serde_json::Value>,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub retry_count: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_status_default() {
        let status: SwarmStatus = Default::default();
        assert!(matches!(status, SwarmStatus::Pending));
    }

    #[test]
    fn test_swarm_status_display() {
        assert_eq!(format!("{}", SwarmStatus::Pending), "pending");
        assert_eq!(format!("{}", SwarmStatus::Running), "running");
        assert_eq!(format!("{}", SwarmStatus::Completed), "completed");
        assert_eq!(format!("{}", SwarmStatus::Failed), "failed");
        assert_eq!(format!("{}", SwarmStatus::Cancelled), "cancelled");
    }

    #[test]
    fn test_step_status_default() {
        let status: StepStatus = Default::default();
        assert!(matches!(status, StepStatus::Pending));
    }

    #[test]
    fn test_step_status_display() {
        assert_eq!(format!("{}", StepStatus::Pending), "pending");
        assert_eq!(format!("{}", StepStatus::Running), "running");
        assert_eq!(format!("{}", StepStatus::Completed), "completed");
        assert_eq!(format!("{}", StepStatus::Failed), "failed");
        assert_eq!(format!("{}", StepStatus::Skipped), "skipped");
    }

    #[test]
    fn test_swarm_definition_toml_roundtrip() {
        let toml_str = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"
description = "A test swarm definition"

[input]
required = ["query"]

[[steps]]
id = "step1"
name = "First Step"
hand = "coder"
depends_on = []

[[steps]]
id = "step2"
name = "Second Step"
hand = "reviewer"
depends_on = ["step1"]

[error_handling]
default_strategy = "fail"
max_retries = 3
retry_delay_seconds = 5

[settings]
timeout_minutes = 30
max_parallel_steps = 4
"#;

        let definition: SwarmDefinition = toml::from_str(toml_str).expect("Failed to parse TOML");

        assert_eq!(definition.id, "test-swarm");
        assert_eq!(definition.name, "Test Swarm");
        assert_eq!(definition.version, "1.0.0");
        assert_eq!(definition.description, Some("A test swarm definition".to_string()));
        assert_eq!(definition.input.required, vec!["query"]);
        assert_eq!(definition.steps.len(), 2);
        assert_eq!(definition.steps[0].id, "step1");
        assert_eq!(definition.steps[1].id, "step2");
        assert_eq!(definition.steps[1].depends_on, vec!["step1"]);
        assert_eq!(definition.error_handling.default_strategy, "fail");
        assert_eq!(definition.error_handling.max_retries, 3);
        assert_eq!(definition.settings.timeout_minutes, Some(30));
        assert_eq!(definition.settings.max_parallel_steps, Some(4));
    }

    #[test]
    fn test_swarm_definition_default_values() {
        let toml_str = r#"
id = "minimal-swarm"
name = "Minimal Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
"#;

        let definition: SwarmDefinition = toml::from_str(toml_str).expect("Failed to parse TOML");

        assert_eq!(definition.id, "minimal-swarm");
        assert!(definition.description.is_none());
        assert!(definition.input.required.is_empty());
        assert!(definition.input.optional.is_empty());
        assert!(definition.steps[0].depends_on.is_empty());
        assert!(definition.steps[0].input_mapping.is_empty());
        assert!(definition.steps[0].output_mapping.is_empty());
        assert!(definition.steps[0].condition.is_none());
        assert!(definition.steps[0].retry.is_none());
        assert_eq!(definition.error_handling.default_strategy, "fail");
        assert_eq!(definition.error_handling.max_retries, 0);
        assert_eq!(definition.error_handling.retry_delay_seconds, 5);
        assert!(definition.settings.timeout_minutes.is_none());
    }

    #[test]
    fn test_swarm_status_serialization() {
        let status = SwarmStatus::Running;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, "\"running\"");

        let deserialized: SwarmStatus = serde_json::from_str(&json).expect("Failed to deserialize");
        assert!(matches!(deserialized, SwarmStatus::Running));
    }

    #[test]
    fn test_step_status_serialization() {
        let status = StepStatus::Completed;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, "\"completed\"");

        let deserialized: StepStatus = serde_json::from_str(&json).expect("Failed to deserialize");
        assert!(matches!(deserialized, StepStatus::Completed));
    }

    #[test]
    fn test_retry_config_defaults() {
        let toml_str = r#"
max_attempts = 5
"#;

        let config: RetryConfig = toml::from_str(toml_str).expect("Failed to parse TOML");
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.delay_seconds, 5); // default
        assert_eq!(config.backoff, "fixed"); // default
    }

    #[test]
    fn test_swarm_execution_creation() {
        let execution = SwarmExecution {
            id: "exec-123".to_string(),
            definition_id: "def-456".to_string(),
            input: HashMap::new(),
            status: SwarmStatus::Pending,
            step_results: HashMap::new(),
            output: None,
            error: None,
            started_at: Utc::now(),
            completed_at: None,
        };

        assert_eq!(execution.id, "exec-123");
        assert_eq!(execution.definition_id, "def-456");
        assert!(matches!(execution.status, SwarmStatus::Pending));
        assert!(execution.completed_at.is_none());
    }

    #[test]
    fn test_swarm_step_result_creation() {
        let result = SwarmStepResult {
            step_id: "step1".to_string(),
            status: StepStatus::Completed,
            output: Some(serde_json::json!({"key": "value"})),
            duration_ms: 1000,
            error: None,
            retry_count: 2,
            input_tokens: 100,
            output_tokens: 200,
        };

        assert_eq!(result.step_id, "step1");
        assert!(matches!(result.status, StepStatus::Completed));
        assert_eq!(result.duration_ms, 1000);
        assert_eq!(result.retry_count, 2);
    }
}
