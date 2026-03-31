//! Swarm execution engine — multi-step agent pipeline with dependency management.
//!
//! The swarm engine executes workflows defined in Swarm.toml format, where steps
//! can have dependencies on other steps. Steps are organized into execution layers
//! using topological sort, allowing parallel execution of independent steps.
//!
//! Features:
//! - Dependency-based step ordering
//! - Condition evaluation for conditional step execution
//! - Input/output mapping between steps
//! - Retry logic with configurable strategies
//! - Error handling (fail, skip, retry)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use openfang_types::swarm::{
    ErrorHandlingConfig, StepStatus, SwarmDefinition, SwarmExecution, SwarmStatus,
    SwarmStep, SwarmStepResult,
};

use super::swarm_condition::{ConditionContext, ConditionEvaluator};
use super::swarm_mapping::{MappingContext, MappingEngine};
use super::swarm_parser::SwarmParser;

/// The swarm execution engine — manages definitions and executes swarm runs.
pub struct SwarmEngine {
    definitions: Arc<RwLock<HashMap<String, SwarmDefinition>>>,
    executions: Arc<RwLock<HashMap<String, SwarmExecution>>>,
}

impl SwarmEngine {
    /// Create a new swarm engine.
    pub fn new() -> Self {
        Self {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load a swarm definition from TOML content.
    ///
    /// Parses the TOML, validates the definition, and stores it.
    /// Returns the definition ID on success.
    pub async fn load_definition(&self, toml_content: &str) -> Result<String, String> {
        let definition = SwarmParser::parse_toml(toml_content)?;

        // Validate the definition
        SwarmParser::validate(&definition)?;

        let def_id = definition.id.clone();

        // Store the definition
        self.definitions
            .write()
            .await
            .insert(def_id.clone(), definition);

        info!(swarm_id = %def_id, "Swarm definition loaded");

        Ok(def_id)
    }

    /// List all loaded swarm definitions.
    pub async fn list_definitions(&self) -> Vec<SwarmDefinition> {
        self.definitions.read().await.values().cloned().collect()
    }

    /// Get a specific swarm definition by ID.
    pub async fn get_definition(&self, def_id: &str) -> Option<SwarmDefinition> {
        self.definitions.read().await.get(def_id).cloned()
    }

    /// Create a new swarm execution instance.
    ///
    /// Validates that all required inputs are provided, then creates
    /// an execution record with Pending status.
    pub async fn create_execution(
        &self,
        def_id: &str,
        input: HashMap<String, Value>,
    ) -> Result<String, String> {
        // Get the definition
        let definition = self
            .get_definition(def_id)
            .await
            .ok_or_else(|| format!("Swarm definition '{}' not found", def_id))?;

        // Validate required inputs
        for required_key in &definition.input.required {
            if !input.contains_key(required_key) {
                return Err(format!(
                    "Missing required input parameter: '{}'",
                    required_key
                ));
            }
        }

        // Create execution record
        let execution_id = Uuid::new_v4().to_string();
        let execution = SwarmExecution {
            id: execution_id.clone(),
            definition_id: def_id.to_string(),
            input,
            status: SwarmStatus::Pending,
            step_results: HashMap::new(),
            output: None,
            error: None,
            started_at: Utc::now(),
            completed_at: None,
        };

        self.executions
            .write()
            .await
            .insert(execution_id.clone(), execution);

        info!(execution_id = %execution_id, swarm_id = %def_id, "Swarm execution created");

        Ok(execution_id)
    }

    /// Execute a swarm run.
    ///
    /// This is the core execution method that:
    /// 1. Performs topological sort to determine execution order
    /// 2. Executes steps layer by layer
    /// 3. Applies input/output mappings
    /// 4. Evaluates conditions for conditional steps
    /// 5. Handles errors according to the configured strategy
    ///
    /// The `send_message` callback is used to invoke agents:
    /// `(agent_name: String, prompt: String) -> Result<(output: String, input_tokens: u64, output_tokens: u64), String>`
    pub async fn execute<F, Fut>(
        &self,
        execution_id: &str,
        send_message: F,
    ) -> Result<Value, String>
    where
        F: Fn(String, String) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(String, u64, u64), String>> + Send,
    {
        // Get execution and update status to Running
        let definition = {
            let mut executions = self.executions.write().await;
            let execution = executions
                .get_mut(execution_id)
                .ok_or_else(|| format!("Execution '{}' not found", execution_id))?;

            execution.status = SwarmStatus::Running;

            // Get the definition
            self.definitions
                .read()
                .await
                .get(&execution.definition_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "Definition '{}' not found",
                        execution.definition_id
                    )
                })?
        };

        info!(
            execution_id = %execution_id,
            swarm_name = %definition.name,
            steps = definition.steps.len(),
            "Starting swarm execution"
        );

        // Perform topological sort to get execution layers
        let layers = SwarmParser::topological_sort(&definition)?;

        debug!(
            execution_id = %execution_id,
            layers = layers.len(),
            "Topological sort completed"
        );

        // Execute layers
        let mut final_output: Option<Value> = None;

        for (layer_idx, layer) in layers.iter().enumerate() {
            debug!(
                execution_id = %execution_id,
                layer = layer_idx + 1,
                steps_in_layer = layer.len(),
                "Executing layer"
            );

            // Execute steps in this layer sequentially (MVP)
            // TODO: In future, parallel execution using tokio::join_all
            for step_id in layer {
                match self
                    .execute_step(
                        execution_id,
                        &definition,
                        step_id,
                        &send_message,
                    )
                    .await
                {
                    Ok(step_output) => {
                        // Store the last completed step's output as potential final output
                        if let Some(output) = step_output {
                            final_output = Some(output);
                        }
                    }
                    Err(e) => {
                        error!(
                            execution_id = %execution_id,
                            step_id = %step_id,
                            error = %e,
                            "Step execution failed with fatal error"
                        );

                        // Update execution with error
                        let mut executions = self.executions.write().await;
                        if let Some(execution) = executions.get_mut(execution_id) {
                            execution.status = SwarmStatus::Failed;
                            execution.error = Some(format!("Step '{}' failed: {}", step_id, e));
                            execution.completed_at = Some(Utc::now());
                        }

                        return Err(e);
                    }
                }
            }
        }

        // All steps completed successfully
        let mut executions = self.executions.write().await;
        if let Some(execution) = executions.get_mut(execution_id) {
            execution.status = SwarmStatus::Completed;
            execution.output = final_output.clone();
            execution.completed_at = Some(Utc::now());
        }

        info!(
            execution_id = %execution_id,
            status = "completed",
            "Swarm execution finished"
        );

        Ok(final_output.unwrap_or(Value::Null))
    }

    /// Execute a single step.
    ///
    /// Handles:
    /// - Condition evaluation
    /// - Input mapping
    /// - Agent invocation with retry logic
    /// - Output mapping
    /// - Error handling
    async fn execute_step<F, Fut>(
        &self,
        execution_id: &str,
        definition: &SwarmDefinition,
        step_id: &str,
        send_message: &F,
    ) -> Result<Option<Value>, String>
    where
        F: Fn(String, String) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(String, u64, u64), String>> + Send,
    {
        // Find the step definition
        let step = definition
            .steps
            .iter()
            .find(|s| s.id == step_id)
            .ok_or_else(|| format!("Step '{}' not found in definition", step_id))?;

        // Initialize step result
        {
            let mut executions = self.executions.write().await;
            let execution = executions
                .get_mut(execution_id)
                .ok_or_else(|| format!("Execution '{}' not found", execution_id))?;

            execution.step_results.insert(
                step_id.to_string(),
                SwarmStepResult {
                    step_id: step_id.to_string(),
                    status: StepStatus::Pending,
                    output: None,
                    duration_ms: 0,
                    error: None,
                    retry_count: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                },
            );
        }

        // Check condition if present
        if let Some(condition_expr) = &step.condition {
            // Read execution data and clone what we need, then release the lock
            let (input, step_outputs) = {
                let executions = self.executions.read().await;
                let execution = executions
                    .get(execution_id)
                    .ok_or_else(|| format!("Execution '{}' not found", execution_id))?;

                let step_outputs: HashMap<String, Value> = execution
                    .step_results
                    .iter()
                    .filter(|(_, r)| matches!(r.status, StepStatus::Completed))
                    .filter_map(|(id, r)| r.output.clone().map(|o| (id.clone(), o)))
                    .collect();

                (execution.input.clone(), step_outputs)
            };
            // Lock is released here

            // Construct ConditionContext in the same scope where cloned data lives
            let condition_ctx = ConditionContext {
                input: &input,
                step_outputs: &step_outputs,
            };

            match ConditionEvaluator::evaluate(condition_expr, &condition_ctx) {
                Ok(true) => {
                    debug!(step_id = %step_id, "Condition evaluated to true, executing step");
                }
                Ok(false) => {
                    info!(step_id = %step_id, "Condition evaluated to false, skipping step");

                    // Mark step as skipped
                    let mut executions = self.executions.write().await;
                    if let Some(execution) = executions.get_mut(execution_id) {
                        if let Some(result) = execution.step_results.get_mut(step_id) {
                            result.status = StepStatus::Skipped;
                        }
                    }

                    return Ok(None);
                }
                Err(e) => {
                    return Err(format!(
                        "Step '{}': condition evaluation failed: {}",
                        step_id, e
                    ));
                }
            }
        }

        // Build mapping context
        let mapping_ctx = self.build_mapping_context(execution_id).await?;

        // Apply input mappings
        let mapped_inputs = MappingEngine::apply_mappings(&step.input_mapping, &mapping_ctx)
            .map_err(|e| format!("Step '{}': input mapping failed: {}", step_id, e))?;

        // Serialize mapped inputs to JSON prompt
        let prompt = serde_json::to_string(&mapped_inputs)
            .map_err(|e| format!("Step '{}': failed to serialize prompt: {}", step_id, e))?;

        // Update step status to Running
        let start_time = Instant::now();
        {
            let mut executions = self.executions.write().await;
            if let Some(execution) = executions.get_mut(execution_id) {
                if let Some(result) = execution.step_results.get_mut(step_id) {
                    result.status = StepStatus::Running;
                }
            }
        }

        debug!(step_id = %step_id, hand = %step.hand, "Calling agent");

        // Execute step with retry logic
        let (output_str, input_tokens, output_tokens, actual_retries) = match self
            .execute_step_with_retry(
                step,
                &definition.error_handling,
                send_message,
                &prompt,
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                // Handle error according to error_handling strategy
                let error_strategy = step
                    .retry
                    .as_ref()
                    .map(|_| "retry".to_string())
                    .unwrap_or_else(|| definition.error_handling.default_strategy.clone());

                match error_strategy.as_str() {
                    "fail" => {
                        // Update step result with failure
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        let mut executions = self.executions.write().await;
                        if let Some(execution) = executions.get_mut(execution_id) {
                            if let Some(result) = execution.step_results.get_mut(step_id) {
                                result.status = StepStatus::Failed;
                                result.error = Some(e.clone());
                                result.duration_ms = duration_ms;
                            }
                        }
                        return Err(format!("Step '{}' failed: {}", step_id, e));
                    }
                    "skip" => {
                        warn!(step_id = %step_id, error = %e, "Step failed, skipping");
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        let mut executions = self.executions.write().await;
                        if let Some(execution) = executions.get_mut(execution_id) {
                            if let Some(result) = execution.step_results.get_mut(step_id) {
                                result.status = StepStatus::Failed;
                                result.error = Some(e);
                                result.duration_ms = duration_ms;
                            }
                        }
                        return Ok(None);
                    }
                    _ => {
                        // Should not reach here as retry is handled in execute_step_with_retry
                        return Err(format!("Step '{}' failed: {}", step_id, e));
                    }
                }
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Parse agent output as JSON
        let output_value = match serde_json::from_str::<Value>(&output_str) {
            Ok(value) => value,
            Err(_) => {
                // If not valid JSON, wrap as string
                Value::String(output_str)
            }
        };

        // Apply output mappings if any
        let final_output = if !step.output_mapping.is_empty() {
            let output_ctx = MappingContext {
                input: {
                    let mut map = HashMap::new();
                    map.insert("_raw".to_string(), output_value.clone());
                    map
                },
                step_outputs: {
                    let mut map = HashMap::new();
                    map.insert(step_id.to_string(), output_value.clone());
                    map
                },
            };

            match MappingEngine::apply_mappings(&step.output_mapping, &output_ctx) {
                Ok(mapped) => {
                    // Convert mapped outputs to a single JSON value
                    if mapped.len() == 1 && mapped.contains_key("_raw") {
                        mapped.get("_raw").cloned().unwrap_or(output_value)
                    } else {
                        Value::Object(mapped.into_iter().collect::<serde_json::Map<String, Value>>())
                    }
                }
                Err(e) => {
                    warn!(step_id = %step_id, error = %e, "Output mapping failed, using raw output");
                    output_value
                }
            }
        } else {
            output_value
        };

        // Update step result
        {
            let mut executions = self.executions.write().await;
            if let Some(execution) = executions.get_mut(execution_id) {
                if let Some(result) = execution.step_results.get_mut(step_id) {
                    result.status = StepStatus::Completed;
                    result.output = Some(final_output.clone());
                    result.duration_ms = duration_ms;
                    result.input_tokens = input_tokens;
                    result.output_tokens = output_tokens;
                    result.retry_count = actual_retries;
                }
            }
        }

        info!(
            step_id = %step_id,
            duration_ms = duration_ms,
            input_tokens = input_tokens,
            output_tokens = output_tokens,
            "Step completed"
        );

        Ok(Some(final_output))
    }

    /// Execute a step with retry logic.
    /// Returns (output, input_tokens, output_tokens, actual_retry_count).
    async fn execute_step_with_retry<F, Fut>(
        &self,
        step: &SwarmStep,
        error_handling: &ErrorHandlingConfig,
        send_message: &F,
        prompt: &str,
    ) -> Result<(String, u64, u64, u32), String>
    where
        F: Fn(String, String) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(String, u64, u64), String>> + Send,
    {
        let max_attempts = step
            .retry
            .as_ref()
            .map(|r| r.max_attempts)
            .unwrap_or_else(|| error_handling.max_retries + 1); // +1 because first attempt is not a retry

        let delay_secs = step
            .retry
            .as_ref()
            .map(|r| r.delay_seconds)
            .unwrap_or(error_handling.retry_delay_seconds);

        let mut last_error = String::new();

        for attempt in 0..max_attempts {
            match send_message(step.hand.clone(), prompt.to_string()).await {
                Ok((output, in_tok, out_tok)) => {
                    // actual_retry_count is the number of retries (failed attempts)
                    return Ok((output, in_tok, out_tok, attempt));
                }
                Err(e) => {
                    last_error = e;
                    if attempt + 1 < max_attempts {
                        warn!(
                            step_id = %step.id,
                            attempt = attempt + 1,
                            max_attempts = max_attempts,
                            error = %last_error,
                            "Step failed, retrying after {}s",
                            delay_secs
                        );
                        tokio::time::sleep(Duration::from_secs(delay_secs as u64)).await;
                    }
                }
            }
        }

        Err(format!(
            "Step '{}' failed after {} attempts: {}",
            step.id, max_attempts, last_error
        ))
    }

    /// Build a MappingContext for applying input mappings.
    async fn build_mapping_context(
        &self,
        execution_id: &str,
    ) -> Result<MappingContext, String> {
        let executions = self.executions.read().await;
        let execution = executions
            .get(execution_id)
            .ok_or_else(|| format!("Execution '{}' not found", execution_id))?;

        // Collect step outputs from completed steps
        let step_outputs: HashMap<String, Value> = execution
            .step_results
            .iter()
            .filter(|(_, r)| matches!(r.status, StepStatus::Completed))
            .filter_map(|(id, r)| r.output.clone().map(|o| (id.clone(), o)))
            .collect();

        Ok(MappingContext {
            input: execution.input.clone(),
            step_outputs,
        })
    }

    /// Get a specific execution by ID.
    pub async fn get_execution(&self, execution_id: &str) -> Option<SwarmExecution> {
        self.executions.read().await.get(execution_id).cloned()
    }

    /// List all executions.
    pub async fn list_executions(&self) -> Vec<SwarmExecution> {
        self.executions.read().await.values().cloned().collect()
    }
}

impl Default for SwarmEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openfang_types::swarm::{SwarmInput, SwarmStep};

    fn create_test_swarm_definition() -> SwarmDefinition {
        SwarmDefinition {
            id: "test-swarm".to_string(),
            name: "Test Swarm".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test swarm".to_string()),
            input: SwarmInput {
                required: vec!["query".to_string()],
                optional: HashMap::new(),
            },
            steps: vec![
                SwarmStep {
                    id: "step1".to_string(),
                    name: "First Step".to_string(),
                    description: None,
                    hand: "coder".to_string(),
                    depends_on: vec![],
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert("query".to_string(), "input.query".to_string());
                        map
                    },
                    output_mapping: HashMap::new(),
                    condition: None,
                    retry: None,
                },
                SwarmStep {
                    id: "step2".to_string(),
                    name: "Second Step".to_string(),
                    description: None,
                    hand: "reviewer".to_string(),
                    depends_on: vec!["step1".to_string()],
                    input_mapping: {
                        let mut map = HashMap::new();
                        map.insert("prev_result".to_string(), "steps.step1._raw".to_string());
                        map
                    },
                    output_mapping: HashMap::new(),
                    condition: None,
                    retry: None,
                },
            ],
            error_handling: ErrorHandlingConfig::default(),
            settings: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_swarm_engine_new() {
        let engine = SwarmEngine::new();
        let definitions = engine.list_definitions().await;
        assert!(definitions.is_empty());
    }

    #[tokio::test]
    async fn test_load_definition() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "my-swarm"
name = "My Swarm"
version = "1.0.0"
description = "A test swarm"

[[steps]]
id = "analyze"
name = "Analyze Code"
hand = "analyst"

[[steps]]
id = "review"
name = "Review Code"
hand = "reviewer"
depends_on = ["analyze"]
"#;

        let result = engine.load_definition(toml).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-swarm");

        let definitions = engine.list_definitions().await;
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "My Swarm");
    }

    #[tokio::test]
    async fn test_create_execution() {
        let engine = SwarmEngine::new();

        // First load a definition
        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[input]
required = ["query"]

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
"#;

        engine.load_definition(toml).await.unwrap();

        // Create execution with required input
        let mut input = HashMap::new();
        input.insert("query".to_string(), Value::String("hello".to_string()));

        let result = engine.create_execution("test-swarm", input).await;
        assert!(result.is_ok());

        let execution_id = result.unwrap();
        let execution = engine.get_execution(&execution_id).await;
        assert!(execution.is_some());
        assert!(matches!(execution.unwrap().status, SwarmStatus::Pending));
    }

    #[tokio::test]
    async fn test_create_execution_missing_required_input() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[input]
required = ["query"]

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
"#;

        engine.load_definition(toml).await.unwrap();

        // Create execution without required input
        let input = HashMap::new();
        let result = engine.create_execution("test-swarm", input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required input"));
    }

    #[tokio::test]
    async fn test_execute_simple_swarm() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
"#;

        engine.load_definition(toml).await.unwrap();

        let input = HashMap::new();
        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        // Mock send_message function
        let send_message = |_agent: String, _prompt: String| async move {
            Ok((r#"{"result": "success"}"#.to_string(), 10u64, 20u64))
        };

        let result = engine.execute(&execution_id, send_message).await;
        assert!(result.is_ok());

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Completed));
    }

    #[tokio::test]
    async fn test_execute_with_condition() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[input]
required = ["should_run"]

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
condition = "input.should_run == true"
"#;

        engine.load_definition(toml).await.unwrap();

        // Test with condition true
        let mut input = HashMap::new();
        input.insert("should_run".to_string(), Value::Bool(true));

        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        let send_message = |_agent: String, _prompt: String| async move {
            Ok((r#"{"result": "success"}"#.to_string(), 10u64, 20u64))
        };

        let result = engine.execute(&execution_id, send_message).await;
        assert!(result.is_ok());

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Completed));
        assert!(matches!(
            execution.step_results.get("step1").unwrap().status,
            StepStatus::Completed
        ));
    }

    #[tokio::test]
    async fn test_execute_with_skipped_condition() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[input]
required = ["should_run"]

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
condition = "input.should_run == true"
"#;

        engine.load_definition(toml).await.unwrap();

        // Test with condition false
        let mut input = HashMap::new();
        input.insert("should_run".to_string(), Value::Bool(false));

        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        let send_message = |_agent: String, _prompt: String| async move {
            Ok((r#"{"result": "success"}"#.to_string(), 10u64, 20u64))
        };

        let result = engine.execute(&execution_id, send_message).await;
        assert!(result.is_ok());

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Completed));
        assert!(matches!(
            execution.step_results.get("step1").unwrap().status,
            StepStatus::Skipped
        ));
    }

    #[tokio::test]
    async fn test_execute_with_retry_success() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"

[steps.retry]
max_attempts = 3
delay_seconds = 0
"#;

        engine.load_definition(toml).await.unwrap();

        let input = HashMap::new();
        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        // Mock that succeeds on first attempt (retry configured but not needed)
        let send_message = |_agent: String, _prompt: String| async move {
            Ok((r#"{"result": "success"}"#.to_string(), 10u64, 20u64))
        };

        let result = engine.execute(&execution_id, send_message).await;
        assert!(result.is_ok());

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Completed));
    }

    #[tokio::test]
    async fn test_execute_with_retry_exhausted() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"

[steps.retry]
max_attempts = 2
delay_seconds = 0
"#;

        engine.load_definition(toml).await.unwrap();

        let input = HashMap::new();
        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        // Mock that always fails
        let send_message = |_agent: String, _prompt: String| async move {
            Err("Persistent error".to_string())
        };

        let result = engine.execute(&execution_id, send_message).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed after 2 attempts"));

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Failed));
    }

    #[tokio::test]
    async fn test_execute_multi_layer_dag() {
        let engine = SwarmEngine::new();

        // A 3-layer DAG:
        // Layer 1: step1
        // Layer 2: step2, step3 (both depend on step1)
        // Layer 3: step4 (depends on step2 and step3)
        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"

[[steps]]
id = "step2"
name = "Step 2"
hand = "reviewer"
depends_on = ["step1"]

[[steps]]
id = "step3"
name = "Step 3"
hand = "tester"
depends_on = ["step1"]

[[steps]]
id = "step4"
name = "Step 4"
hand = "deployer"
depends_on = ["step2", "step3"]
"#;

        engine.load_definition(toml).await.unwrap();

        let input = HashMap::new();
        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        async fn mock_send_message(_agent: String, _prompt: String) -> Result<(String, u64, u64), String> {
            Ok((r#"{"result": "success"}"#.to_string(), 10u64, 20u64))
        }

        let result = engine.execute(&execution_id, mock_send_message).await;
        assert!(result.is_ok());

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Completed));
        assert_eq!(execution.step_results.len(), 4);

        // Verify all steps completed
        for step_id in ["step1", "step2", "step3", "step4"] {
            assert!(
                matches!(
                    execution.step_results.get(step_id).unwrap().status,
                    StepStatus::Completed
                ),
                "Step {} should be completed",
                step_id
            );
        }
    }

    #[tokio::test]
    async fn test_execute_with_input_output_mapping() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[input]
required = ["query"]

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
input_mapping = { query = "input.query" }

[[steps]]
id = "step2"
name = "Step 2"
hand = "reviewer"
depends_on = ["step1"]
input_mapping = { prev_result = "steps.step1._raw" }
"#;

        engine.load_definition(toml).await.unwrap();

        let mut input = HashMap::new();
        input.insert("query".to_string(), Value::String("test query".to_string()));
        let execution_id = engine.create_execution("test-swarm", input).await.unwrap();

        let send_message = |_agent: String, prompt: String| async move {
            // Return the prompt as result so we can verify mapping worked
            Ok((prompt, 10u64, 20u64))
        };

        let result = engine.execute(&execution_id, send_message).await;
        assert!(result.is_ok());

        let execution = engine.get_execution(&execution_id).await.unwrap();
        assert!(matches!(execution.status, SwarmStatus::Completed));

        // Verify step2 received step1's output
        let step2_result = execution.step_results.get("step2").unwrap();
        assert!(step2_result.output.is_some());
    }

    #[tokio::test]
    async fn test_list_executions() {
        let engine = SwarmEngine::new();

        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
"#;

        engine.load_definition(toml).await.unwrap();

        // Create multiple executions
        let input = HashMap::new();
        let exec1 = engine.create_execution("test-swarm", input.clone()).await.unwrap();
        let exec2 = engine.create_execution("test-swarm", input).await.unwrap();

        let executions = engine.list_executions().await;
        assert_eq!(executions.len(), 2);

        let ids: Vec<_> = executions.iter().map(|e| e.id.clone()).collect();
        assert!(ids.contains(&exec1));
        assert!(ids.contains(&exec2));
    }

    #[tokio::test]
    async fn test_get_nonexistent_execution() {
        let engine = SwarmEngine::new();

        let result = engine.get_execution("nonexistent-id").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_execute_nonexistent_execution() {
        let engine = SwarmEngine::new();

        let send_message = |_agent: String, _prompt: String| async move {
            Ok((r#"{}"#.to_string(), 10u64, 20u64))
        };

        let result = engine.execute("nonexistent-id", send_message).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
