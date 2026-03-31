//! Swarm data mapping engine — resolves input/output mappings for swarm steps.
//!
//! The mapping engine provides a way to extract values from various sources
//! (workflow input, step outputs, environment variables) and map them to
//! step input parameters.

use openfang_types::swarm::SwarmExecution;
use serde_json::Value;
use std::collections::HashMap;

/// Mapping execution context, containing data that can be referenced during execution.
#[derive(Debug, Clone)]
pub struct MappingContext {
    /// Workflow input parameters.
    pub input: HashMap<String, Value>,
    /// Outputs from completed steps.
    pub step_outputs: HashMap<String, Value>,
}

/// Data mapping engine for swarm step input resolution.
pub struct MappingEngine;

impl MappingEngine {
    /// Parse a single mapping source path and extract the value from context.
    ///
    /// Supported formats:
    /// - `input.<key>` — reference workflow input parameter
    /// - `steps.<step_id>.<key>` — reference field from a previous step's output
    /// - `env.<VAR>` — reference environment variable
    /// - `"literal"` — quoted literal string
    ///
    /// For `steps.<step_id>._raw`, the entire step output value is returned.
    pub fn resolve(source: &str, ctx: &MappingContext) -> Result<Value, String> {
        if source.is_empty() {
            return Err("Empty mapping source".to_string());
        }

        // Check for quoted literal string
        if (source.starts_with('"') && source.ends_with('"'))
            || (source.starts_with('\'') && source.ends_with('\''))
        {
            let literal = &source[1..source.len() - 1];
            return Ok(Value::String(literal.to_string()));
        }

        // Parse the source path
        let parts: Vec<&str> = source.split('.').collect();
        if parts.is_empty() {
            return Err(format!("Invalid mapping source: {}", source));
        }

        match parts[0] {
            "input" => {
                if parts.len() < 2 {
                    return Err(format!(
                        "Invalid input reference '{}', expected 'input.<key>'",
                        source
                    ));
                }
                let key = parts[1];
                ctx.input
                    .get(key)
                    .cloned()
                    .ok_or_else(|| format!("Input key '{}' not found", key))
            }
            "steps" => {
                if parts.len() < 3 {
                    return Err(format!(
                        "Invalid steps reference '{}', expected 'steps.<step_id>.<key>'",
                        source
                    ));
                }
                let step_id = parts[1];
                let key = parts[2];

                let step_output = ctx.step_outputs.get(step_id).ok_or_else(|| {
                    format!("Step '{}' not found in step outputs", step_id)
                })?;

                // Special key `_raw` returns the entire value
                if key == "_raw" {
                    return Ok(step_output.clone());
                }

                // Otherwise, try to extract the field from the object
                match step_output {
                    Value::Object(map) => map
                        .get(key)
                        .cloned()
                        .ok_or_else(|| format!("Key '{}' not found in step '{}' output", key, step_id)),
                    _ => Err(format!(
                        "Step '{}' output is not an object, cannot access key '{}'",
                        step_id, key
                    )),
                }
            }
            "env" => {
                if parts.len() < 2 {
                    return Err(format!(
                        "Invalid env reference '{}', expected 'env.<VAR>'",
                        source
                    ));
                }
                let var_name = parts[1];
                std::env::var(var_name)
                    .map(Value::String)
                    .map_err(|_| format!("Environment variable '{}' not set", var_name))
            }
            _ => Err(format!(
                "Unknown mapping source prefix '{}', expected 'input', 'steps', 'env', or a quoted literal",
                parts[0]
            )),
        }
    }

    /// Apply a set of mapping rules and return the target parameters as a HashMap.
    ///
    /// The `mappings` parameter is a map from target key to source path.
    /// Each source path is resolved using [`resolve`] and the result is
    /// inserted into the output HashMap with the target key.
    pub fn apply_mappings(
        mappings: &HashMap<String, String>,
        ctx: &MappingContext,
    ) -> Result<HashMap<String, Value>, String> {
        let mut result = HashMap::with_capacity(mappings.len());

        for (target_key, source_path) in mappings {
            match Self::resolve(source_path, ctx) {
                Ok(value) => {
                    result.insert(target_key.clone(), value);
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to resolve mapping for '{}': {}",
                        target_key, e
                    ));
                }
            }
        }

        Ok(result)
    }

    /// Build a MappingContext from a SwarmExecution.
    ///
    /// Extracts the `output` field from each completed step's result
    /// and builds the step_outputs map.
    pub fn build_context_from_execution(execution: &SwarmExecution) -> MappingContext {
        let step_outputs: HashMap<String, Value> = execution
            .step_results
            .iter()
            .filter_map(|(step_id, result)| {
                result.output.as_ref().map(|output| (step_id.clone(), output.clone()))
            })
            .collect();

        MappingContext {
            input: execution.input.clone(),
            step_outputs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openfang_types::swarm::SwarmStepResult;
    use serde_json::json;

    fn create_test_context() -> MappingContext {
        let input = {
            let mut map = HashMap::new();
            map.insert("name".to_string(), json!("Alice"));
            map.insert("age".to_string(), json!(30));
            map
        };

        let step_outputs = {
            let mut map = HashMap::new();
            map.insert(
                "step1".to_string(),
                json!({
                    "result": "success",
                    "data": { "value": 42 }
                }),
            );
            map.insert("step2".to_string(), json!("raw string output"));
            map
        };

        MappingContext {
            input,
            step_outputs,
        }
    }

    #[test]
    fn test_resolve_input() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("input.name", &ctx);
        assert_eq!(result.unwrap(), json!("Alice"));

        let result = MappingEngine::resolve("input.age", &ctx);
        assert_eq!(result.unwrap(), json!(30));
    }

    #[test]
    fn test_resolve_input_not_found() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("input.unknown", &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_resolve_steps_object_field() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("steps.step1.result", &ctx);
        assert_eq!(result.unwrap(), json!("success"));
    }

    #[test]
    fn test_resolve_steps_nested_field() {
        let ctx = create_test_context();

        // "steps.step1.data" extracts the "data" field from step1's output
        // The path format is steps.<step_id>.<key>, where key is a top-level field
        let result = MappingEngine::resolve("steps.step1.data", &ctx);
        assert_eq!(result.unwrap(), json!({ "value": 42 }));
    }

    #[test]
    fn test_resolve_steps_raw() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("steps.step1._raw", &ctx);
        assert_eq!(
            result.unwrap(),
            json!({
                "result": "success",
                "data": { "value": 42 }
            })
        );
    }

    #[test]
    fn test_resolve_steps_not_object() {
        let ctx = create_test_context();

        // step2 output is a string, not an object
        let result = MappingEngine::resolve("steps.step2.some_key", &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not an object"));
    }

    #[test]
    fn test_resolve_steps_not_found() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("steps.unknown.key", &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_resolve_literal_double_quoted() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("\"hello world\"", &ctx);
        assert_eq!(result.unwrap(), json!("hello world"));
    }

    #[test]
    fn test_resolve_literal_single_quoted() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("'hello world'", &ctx);
        assert_eq!(result.unwrap(), json!("hello world"));
    }

    #[test]
    fn test_resolve_empty_source() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("", &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty"));
    }

    #[test]
    fn test_resolve_unknown_prefix() {
        let ctx = create_test_context();

        let result = MappingEngine::resolve("unknown.key", &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown mapping source prefix"));
    }

    #[test]
    fn test_apply_mappings() {
        let ctx = create_test_context();

        let mut mappings = HashMap::new();
        mappings.insert("target_name".to_string(), "input.name".to_string());
        mappings.insert("target_result".to_string(), "steps.step1.result".to_string());
        mappings.insert("target_literal".to_string(), "\"literal_value\"".to_string());

        let result = MappingEngine::apply_mappings(&mappings, &ctx);
        assert!(result.is_ok());

        let map = result.unwrap();
        assert_eq!(map.get("target_name").unwrap(), &json!("Alice"));
        assert_eq!(map.get("target_result").unwrap(), &json!("success"));
        assert_eq!(map.get("target_literal").unwrap(), &json!("literal_value"));
    }

    #[test]
    fn test_apply_mappings_error() {
        let ctx = create_test_context();

        let mut mappings = HashMap::new();
        mappings.insert("target_name".to_string(), "input.name".to_string());
        mappings.insert("target_bad".to_string(), "input.unknown".to_string());

        let result = MappingEngine::apply_mappings(&mappings, &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("target_bad"));
    }

    #[test]
    fn test_build_context_from_execution() {
        let execution = SwarmExecution {
            id: "test-id".to_string(),
            definition_id: "def-id".to_string(),
            input: {
                let mut map = HashMap::new();
                map.insert("key".to_string(), json!("value"));
                map
            },
            status: openfang_types::swarm::SwarmStatus::Running,
            step_results: {
                let mut map = HashMap::new();
                map.insert(
                    "step1".to_string(),
                    SwarmStepResult {
                        step_id: "step1".to_string(),
                        status: openfang_types::swarm::StepStatus::Completed,
                        output: Some(json!({"result": "ok"})),
                        duration_ms: 100,
                        error: None,
                        retry_count: 0,
                        input_tokens: 10,
                        output_tokens: 20,
                    },
                );
                map.insert(
                    "step2".to_string(),
                    SwarmStepResult {
                        step_id: "step2".to_string(),
                        status: openfang_types::swarm::StepStatus::Completed,
                        output: None, // No output
                        duration_ms: 50,
                        error: None,
                        retry_count: 0,
                        input_tokens: 5,
                        output_tokens: 10,
                    },
                );
                map
            },
            output: None,
            error: None,
            started_at: chrono::Utc::now(),
            completed_at: None,
        };

        let ctx = MappingEngine::build_context_from_execution(&execution);

        assert_eq!(ctx.input.get("key").unwrap(), &json!("value"));
        assert_eq!(ctx.step_outputs.len(), 1); // Only step1 has output
        assert!(ctx.step_outputs.contains_key("step1"));
        assert!(!ctx.step_outputs.contains_key("step2"));
    }
}
