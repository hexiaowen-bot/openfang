//! Swarm TOML configuration parser and validator.
//!
//! This module provides parsing and validation for Swarm.toml files,
//! including dependency checking, cycle detection, and topological sorting.

use openfang_types::swarm::SwarmDefinition;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Parser for Swarm TOML configuration files.
pub struct SwarmParser;

impl SwarmParser {
    /// Parse a SwarmDefinition from a TOML string.
    pub fn parse_toml(content: &str) -> Result<SwarmDefinition, String> {
        toml::from_str(content).map_err(|e| format!("Failed to parse TOML: {}", e))
    }

    /// Load and parse a SwarmDefinition from a file path.
    pub fn load_from_file(path: &Path) -> Result<SwarmDefinition, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;
        Self::parse_toml(&content)
    }

    /// Validate a SwarmDefinition for completeness and correctness.
    ///
    /// Performs the following validations:
    /// 1. Required fields are present (id, name, steps)
    /// 2. Step IDs are unique
    /// 3. All `depends_on` references point to existing steps
    /// 4. No circular dependencies (DAG validation)
    /// 5. Input mapping values follow valid format
    pub fn validate(swarm: &SwarmDefinition) -> Result<(), String> {
        // 1. Required fields validation
        if swarm.id.is_empty() {
            return Err("Swarm 'id' cannot be empty".to_string());
        }
        if swarm.name.is_empty() {
            return Err("Swarm 'name' cannot be empty".to_string());
        }
        if swarm.steps.is_empty() {
            return Err("Swarm must have at least one step".to_string());
        }

        // 2. Step ID uniqueness check
        let mut step_ids = HashSet::new();
        for step in &swarm.steps {
            if step.id.is_empty() {
                return Err("Step 'id' cannot be empty".to_string());
            }
            if !step_ids.insert(&step.id) {
                return Err(format!("Duplicate step id: '{}'", step.id));
            }
        }

        // 3. Dependency validation - all depends_on must reference existing steps
        for step in &swarm.steps {
            for dep in &step.depends_on {
                if !step_ids.contains(dep) {
                    return Err(format!(
                        "Step '{}' depends on non-existent step '{}'",
                        step.id, dep
                    ));
                }
            }
        }

        // 4. DAG cycle detection using DFS
        Self::check_for_cycles(swarm)?;

        // 5. Input mapping format validation
        for step in &swarm.steps {
            for (key, value) in &step.input_mapping {
                if !Self::is_valid_mapping_value(value) {
                    return Err(format!(
                        "Step '{}': invalid input_mapping value for key '{}': '{}'",
                        step.id, key, value
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check for circular dependencies using DFS.
    fn check_for_cycles(swarm: &SwarmDefinition) -> Result<(), String> {
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for step in &swarm.steps {
            adjacency.insert(&step.id, step.depends_on.iter().map(|s| s.as_str()).collect());
        }

        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();

        fn dfs<'a>(
            node: &'a str,
            adjacency: &HashMap<&'a str, Vec<&'a str>>,
            visited: &mut HashSet<&'a str>,
            recursion_stack: &mut HashSet<&'a str>,
            path: &mut Vec<&'a str>,
        ) -> Option<Vec<&'a str>> {
            visited.insert(node);
            recursion_stack.insert(node);
            path.push(node);

            if let Some(deps) = adjacency.get(node) {
                for &dep in deps {
                    if !visited.contains(dep) {
                        if let Some(cycle) = dfs(dep, adjacency, visited, recursion_stack, path) {
                            return Some(cycle);
                        }
                    } else if recursion_stack.contains(dep) {
                        // Found a cycle - extract the cycle from path
                        let cycle_start = path.iter().position(|&x| x == dep).unwrap();
                        let mut cycle = path[cycle_start..].to_vec();
                        cycle.push(dep);
                        return Some(cycle);
                    }
                }
            }

            path.pop();
            recursion_stack.remove(node);
            None
        }

        for step in &swarm.steps {
            if !visited.contains(step.id.as_str()) {
                let mut path = Vec::new();
                if let Some(cycle) = dfs(
                    &step.id,
                    &adjacency,
                    &mut visited,
                    &mut recursion_stack,
                    &mut path,
                ) {
                    let cycle_str = cycle.join(" -> ");
                    return Err(format!("Circular dependency detected: {}", cycle_str));
                }
            }
        }

        Ok(())
    }

    /// Validate that a mapping value follows the allowed format:
    /// - `input.*` - references input fields
    /// - `steps.*.*` - references output from other steps
    /// - `env.*` - references environment variables
    /// - quoted literal strings
    fn is_valid_mapping_value(value: &str) -> bool {
        // Check for valid reference patterns
        if value.starts_with("input.") {
            return value.len() > 6;
        }
        if value.starts_with("steps.") {
            // Format: steps.{step_id}.{field}
            let parts: Vec<&str> = value.split('.').collect();
            return parts.len() >= 3 && !parts[1].is_empty();
        }
        if value.starts_with("env.") {
            return value.len() > 4;
        }
        // Allow quoted strings (both single and double quotes)
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            return value.len() >= 2;
        }
        // Allow numeric literals
        if value.parse::<f64>().is_ok() {
            return true;
        }
        // Allow boolean literals
        if value == "true" || value == "false" {
            return true;
        }
        false
    }

    /// Perform topological sort on swarm steps, returning layers.
    ///
    /// Each inner vector represents a layer of steps that can be executed
    /// in parallel. Steps in the same layer have no dependencies between them.
    pub fn topological_sort(swarm: &SwarmDefinition) -> Result<Vec<Vec<String>>, String> {
        // Build dependency graph
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        // Initialize all steps with 0 in-degree
        for step in &swarm.steps {
            in_degree.insert(&step.id, 0);
            dependents.insert(&step.id, Vec::new());
        }

        // Build the graph
        for step in &swarm.steps {
            for dep in &step.depends_on {
                let degree = in_degree
                    .get_mut(step.id.as_str())
                    .ok_or_else(|| format!("Unknown step id '{}'", step.id))?;
                *degree += 1;

                dependents
                    .get_mut(dep.as_str())
                    .ok_or_else(|| format!("Unknown dependency '{}' for step '{}'", dep, step.id))?
                    .push(&step.id);
            }
        }

        // Kahn's algorithm
        let mut layers: Vec<Vec<String>> = Vec::new();
        let mut current_layer: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut processed = HashSet::new();

        while !current_layer.is_empty() {
            layers.push(current_layer.iter().map(|&s| s.to_string()).collect());

            for &step_id in &current_layer {
                processed.insert(step_id);
            }

            let mut next_layer: Vec<&str> = Vec::new();
            for &step_id in &current_layer {
                if let Some(deps) = dependents.get(step_id) {
                    for &dependent in deps {
                        if let Some(degree) = in_degree.get_mut(dependent) {
                            *degree -= 1;
                            if *degree == 0 && !processed.contains(dependent) {
                                next_layer.push(dependent);
                            }
                        }
                    }
                }
            }

            current_layer = next_layer;
        }

        // Check if all steps were processed (no cycles)
        if processed.len() != swarm.steps.len() {
            return Err("Cannot perform topological sort: circular dependency detected".to_string());
        }

        Ok(layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openfang_types::swarm::{SwarmInput, SwarmStep};
    use std::collections::HashMap;

    fn create_test_swarm() -> SwarmDefinition {
        SwarmDefinition {
            id: "test-swarm".to_string(),
            name: "Test Swarm".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test swarm".to_string()),
            input: SwarmInput::default(),
            steps: vec![
                SwarmStep {
                    id: "step1".to_string(),
                    name: "First Step".to_string(),
                    description: None,
                    hand: "coder".to_string(),
                    depends_on: vec![],
                    input_mapping: HashMap::new(),
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
                    input_mapping: HashMap::new(),
                    output_mapping: HashMap::new(),
                    condition: None,
                    retry: None,
                },
            ],
            error_handling: Default::default(),
            settings: Default::default(),
        }
    }

    #[test]
    fn test_parse_valid_toml() {
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
        let result = SwarmParser::parse_toml(toml);
        assert!(result.is_ok());
        let swarm = result.unwrap();
        assert_eq!(swarm.id, "my-swarm");
        assert_eq!(swarm.steps.len(), 2);
    }

    #[test]
    fn test_parse_invalid_toml() {
        let toml = "invalid toml content {{{";
        let result = SwarmParser::parse_toml(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_id() {
        let mut swarm = create_test_swarm();
        swarm.id = "".to_string();
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("id"));
    }

    #[test]
    fn test_validate_empty_name() {
        let mut swarm = create_test_swarm();
        swarm.name = "".to_string();
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("name"));
    }

    #[test]
    fn test_validate_empty_steps() {
        let mut swarm = create_test_swarm();
        swarm.steps = vec![];
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("step"));
    }

    #[test]
    fn test_validate_duplicate_step_id() {
        let mut swarm = create_test_swarm();
        swarm.steps[1].id = "step1".to_string();
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_validate_missing_dependency() {
        let mut swarm = create_test_swarm();
        swarm.steps[1].depends_on = vec!["nonexistent".to_string()];
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-existent"));
    }

    #[test]
    fn test_validate_circular_dependency() {
        let mut swarm = create_test_swarm();
        swarm.steps[0].depends_on = vec!["step2".to_string()];
        swarm.steps[1].depends_on = vec!["step0".to_string()];
        
        // Need to add step0 that step2 depends on
        swarm.steps.push(SwarmStep {
            id: "step0".to_string(),
            name: "Step 0".to_string(),
            description: None,
            hand: "coder".to_string(),
            depends_on: vec!["step1".to_string()], // Creates cycle
            input_mapping: HashMap::new(),
            output_mapping: HashMap::new(),
            condition: None,
            retry: None,
        });
        
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular"));
    }

    #[test]
    fn test_validate_valid_mapping() {
        let mut swarm = create_test_swarm();
        swarm.steps[0].input_mapping.insert("field1".to_string(), "input.query".to_string());
        swarm.steps[0].input_mapping.insert("field2".to_string(), "steps.step1.output".to_string());
        swarm.steps[0].input_mapping.insert("field3".to_string(), "env.API_KEY".to_string());
        swarm.steps[0].input_mapping.insert("field4".to_string(), "\"literal value\"".to_string());
        swarm.steps[0].input_mapping.insert("field5".to_string(), "42".to_string());
        swarm.steps[0].input_mapping.insert("field6".to_string(), "true".to_string());
        
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_mapping() {
        let mut swarm = create_test_swarm();
        swarm.steps[0].input_mapping.insert("field".to_string(), "invalid_format".to_string());
        
        let result = SwarmParser::validate(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid input_mapping"));
    }

    #[test]
    fn test_topological_sort_linear() {
        let swarm = create_test_swarm();
        let layers = SwarmParser::topological_sort(&swarm).unwrap();
        
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0], vec!["step1"]);
        assert_eq!(layers[1], vec!["step2"]);
    }

    #[test]
    fn test_topological_sort_parallel() {
        let mut swarm = create_test_swarm();
        // Add a step that depends only on step1 (same as step2)
        swarm.steps.push(SwarmStep {
            id: "step3".to_string(),
            name: "Third Step".to_string(),
            description: None,
            hand: "tester".to_string(),
            depends_on: vec!["step1".to_string()],
            input_mapping: HashMap::new(),
            output_mapping: HashMap::new(),
            condition: None,
            retry: None,
        });
        
        let layers = SwarmParser::topological_sort(&swarm).unwrap();
        
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0], vec!["step1"]);
        // step2 and step3 can run in parallel
        assert_eq!(layers[1].len(), 2);
        assert!(layers[1].contains(&"step2".to_string()));
        assert!(layers[1].contains(&"step3".to_string()));
    }

    #[test]
    fn test_topological_sort_cycle() {
        let mut swarm = create_test_swarm();
        swarm.steps[0].depends_on = vec!["step2".to_string()];
        
        let result = SwarmParser::topological_sort(&swarm);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("circular"));
    }

    #[test]
    fn test_load_from_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let toml = r#"
id = "file-swarm"
name = "File Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Step 1"
hand = "coder"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml.as_bytes()).unwrap();
        
        let result = SwarmParser::load_from_file(temp_file.path());
        assert!(result.is_ok());
        let swarm = result.unwrap();
        assert_eq!(swarm.id, "file-swarm");
    }
}
