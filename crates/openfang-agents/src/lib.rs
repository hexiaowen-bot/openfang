//! OpenFang Agents — expert agent template marketplace.
//!
//! This crate provides a template system for creating specialized agents.
//! Templates are blueprints that can be instantiated into full AgentManifests.
//!
//! # Example
//! ```ignore
//! use openfang_agents::AgentTemplateRegistry;
//!
//! let registry = AgentTemplateRegistry::new(templates_dir);
//! registry.load_bundled();
//!
//! // List available templates
//! let templates = registry.list();
//!
//! // Instantiate a template into an AgentManifest
//! let manifest = registry.instantiate("oh-my-opencode", Default::default())?;
//! ```

pub mod bundled;
pub mod registry;

use chrono::{DateTime, Utc};
use openfang_types::agent::{
    AgentManifest, AutonomousConfig, ManifestCapabilities, ModelConfig, ResourceQuota,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ─── Error types ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AgentTemplateError {
    #[error("Template not found: {0}")]
    NotFound(String),
    #[error("Template already exists: {0}")]
    AlreadyExists(String),
    #[error("TOML parse error: {0}")]
    TomlParse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid template: {0}")]
    InvalidTemplate(String),
    #[error("Missing required file: {0}")]
    MissingFile(String),
    #[error("Failed to instantiate: {0}")]
    InstantiateFailed(String),
}

pub type AgentTemplateResult<T> = Result<T, AgentTemplateError>;

// ─── Core types ──────────────────────────────────────────────────────────────

/// Category of an agent template.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentCategory {
    #[default]
    General,
    Coding,
    Research,
    Analysis,
    Writing,
    DevOps,
    Security,
    Communication,
    Finance,
    #[serde(other)]
    Other,
}

impl std::fmt::Display for AgentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::General => write!(f, "General"),
            Self::Coding => write!(f, "Coding"),
            Self::Research => write!(f, "Research"),
            Self::Analysis => write!(f, "Analysis"),
            Self::Writing => write!(f, "Writing"),
            Self::DevOps => write!(f, "DevOps"),
            Self::Security => write!(f, "Security"),
            Self::Communication => write!(f, "Communication"),
            Self::Finance => write!(f, "Finance"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Source of a template.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    /// Compiled into the binary.
    #[default]
    Bundled,
    /// Installed by the user.
    UserInstalled,
    /// Downloaded from a marketplace.
    Marketplace,
}

/// Type of a template requirement check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementType {
    /// A binary must exist on PATH.
    Binary,
    /// An environment variable must be set.
    EnvVar,
    /// An API key environment variable must be set.
    ApiKey,
}

/// A single requirement declared by a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRequirement {
    /// Unique key for this requirement.
    pub key: String,
    /// Human-readable label.
    pub label: String,
    /// What kind of check to perform.
    #[serde(rename = "type")]
    pub requirement_type: RequirementType,
    /// The value to check (binary name, env var name, etc.).
    pub check_value: String,
    /// Whether this requirement is optional.
    #[serde(default)]
    pub optional: bool,
    /// Description of why this is needed.
    #[serde(default)]
    pub description: Option<String>,
}

/// Type of a template setting control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingType {
    Select,
    Text,
    Toggle,
}

/// A single option within a Select-type setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
    /// Env var to check for availability badge.
    #[serde(default)]
    pub provider_env: Option<String>,
}

/// A configurable setting declared in a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSetting {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub setting_type: SettingType,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub options: Vec<SettingOption>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Model configuration within a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateModelConfig {
    /// LLM provider name.
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Model identifier.
    #[serde(default = "default_model")]
    pub model: String,
    /// Maximum tokens for completion.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Sampling temperature.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// File to read system prompt from (default: SOUL.md).
    #[serde(default = "default_prompt_file")]
    pub prompt_file: String,
}

fn default_provider() -> String {
    "default".to_string()
}

fn default_model() -> String {
    "default".to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_temperature() -> f32 {
    0.5
}

fn default_prompt_file() -> String {
    "SOUL.md".to_string()
}

impl Default for TemplateModelConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            prompt_file: default_prompt_file(),
        }
    }
}

/// Agent configuration embedded in a template.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfigTemplate {
    /// Model configuration.
    #[serde(default)]
    pub model: Option<TemplateModelConfig>,
    /// Resource quotas.
    #[serde(default)]
    pub resources: Option<ResourceQuota>,
    /// Capability grants.
    #[serde(default)]
    pub capabilities: Option<ManifestCapabilities>,
    /// Autonomous configuration for 24/7 agents.
    #[serde(default)]
    pub autonomous: Option<AutonomousConfig>,
    /// Tags to apply to the created agent.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Skills to enable (empty = all).
    #[serde(default)]
    pub skills: Vec<String>,
    /// MCP servers to enable (empty = all).
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

/// Complete agent template — a blueprint for creating agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplate {
    /// Unique template identifier (kebab-case).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Detailed description (Markdown).
    #[serde(default)]
    pub long_description: Option<String>,
    /// Author.
    #[serde(default)]
    pub author: String,
    /// Version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Category.
    #[serde(default)]
    pub category: AgentCategory,
    /// Emoji icon.
    #[serde(default)]
    pub icon: Option<String>,
    /// Tags for discovery.
    #[serde(default)]
    pub tags: Vec<String>,
    /// System requirements.
    #[serde(default)]
    pub requires: Vec<TemplateRequirement>,
    /// Configurable settings.
    #[serde(default)]
    pub settings: Vec<TemplateSetting>,
    /// Agent configuration template.
    #[serde(default)]
    pub agent: AgentConfigTemplate,
    /// Source of this template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<TemplateSource>,
    /// SOUL.md content (loaded from file).
    #[serde(skip_serializing)]
    pub soul_content: Option<String>,
    /// BOOTSTRAP.md content (loaded from file).
    #[serde(skip_serializing)]
    pub bootstrap_content: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

impl Default for AgentTemplate {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Unnamed Template".to_string(),
            description: String::new(),
            long_description: None,
            author: String::new(),
            version: default_version(),
            category: AgentCategory::default(),
            icon: None,
            tags: Vec::new(),
            requires: Vec::new(),
            settings: Vec::new(),
            agent: AgentConfigTemplate::default(),
            source: None,
            soul_content: None,
            bootstrap_content: None,
        }
    }
}

impl AgentTemplate {
    /// Instantiate this template into an AgentManifest.
    #[allow(clippy::field_reassign_with_default)]
    pub fn instantiate(&self, settings: &HashMap<String, serde_json::Value>) -> AgentManifest {
        let mut manifest = AgentManifest::default();

        // Basic fields
        manifest.name = self.name.clone();
        manifest.version = self.version.clone();
        manifest.description = self.description.clone();
        manifest.author = self.author.clone();
        manifest.tags = self.tags.clone();

        // Agent config
        if let Some(ref model_template) = self.agent.model {
            manifest.model = ModelConfig {
                provider: model_template.provider.clone(),
                model: model_template.model.clone(),
                max_tokens: model_template.max_tokens,
                temperature: model_template.temperature,
                system_prompt: self.soul_content.clone().unwrap_or_default(),
                api_key_env: None,
                base_url: None,
            };
        }

        if let Some(ref resources) = self.agent.resources {
            manifest.resources = resources.clone();
        }

        if let Some(ref capabilities) = self.agent.capabilities {
            manifest.capabilities = capabilities.clone();
        }

        if let Some(ref autonomous) = self.agent.autonomous {
            manifest.autonomous = Some(autonomous.clone());
        }

        manifest.skills = self.agent.skills.clone();
        manifest.mcp_servers = self.agent.mcp_servers.clone();

        // Apply user settings to manifest
        self.apply_settings(&mut manifest, settings);

        manifest
    }

    /// Apply user settings to the manifest.
    fn apply_settings(
        &self,
        manifest: &mut AgentManifest,
        settings: &HashMap<String, serde_json::Value>,
    ) {
        for (key, value) in settings {
            // Find matching setting definition
            if let Some(setting) = self.settings.iter().find(|s| s.key == *key) {
                match setting.setting_type {
                    SettingType::Select => {
                        // For select settings, we might want to modify behavior
                        // For now, store in metadata
                        manifest.metadata.insert(key.clone(), value.clone());
                    }
                    SettingType::Text => {
                        manifest.metadata.insert(key.clone(), value.clone());
                    }
                    SettingType::Toggle => {
                        if let Some(enabled) = value.as_bool() {
                            manifest.metadata.insert(key.clone(), enabled.into());
                        }
                    }
                }
            }
        }
    }
}

/// Installed template with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledTemplate {
    /// The template definition.
    pub template: AgentTemplate,
    /// Path where the template is installed.
    pub path: PathBuf,
    /// Whether the template is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// When the template was installed.
    pub installed_at: Option<DateTime<Utc>>,
}

fn default_true() -> bool {
    true
}

/// Status of a single requirement.
#[derive(Debug, Clone, Serialize)]
pub struct RequirementStatus {
    pub key: String,
    pub label: String,
    pub requirement_type: RequirementType,
    pub satisfied: bool,
    pub optional: bool,
}

/// Readiness status for a template.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateReadiness {
    /// Whether all requirements are satisfied.
    pub requirements_met: bool,
    /// List of individual requirement statuses.
    pub requirements: Vec<RequirementStatus>,
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Check if a binary exists on PATH.
pub fn check_binary(name: &str) -> bool {
    which_binary(name)
}

/// Check if an environment variable is set and non-empty.
pub fn check_env_var(name: &str) -> bool {
    std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false)
}

/// Check if a binary is on PATH (cross-platform).
fn which_binary(name: &str) -> bool {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let separator = if cfg!(windows) { ';' } else { ':' };
    let extensions: Vec<&str> = if cfg!(windows) {
        vec!["", ".exe", ".cmd", ".bat"]
    } else {
        vec![""]
    };

    for dir in path_var.split(separator) {
        for ext in &extensions {
            let candidate = std::path::Path::new(dir).join(format!("{name}{ext}"));
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_instantiate() {
        let template = AgentTemplate {
            id: "test-template".to_string(),
            name: "Test Template".to_string(),
            description: "A test template".to_string(),
            author: "test".to_string(),
            version: "1.0.0".to_string(),
            category: AgentCategory::Coding,
            tags: vec!["test".to_string()],
            agent: AgentConfigTemplate {
                model: Some(TemplateModelConfig {
                    provider: "groq".to_string(),
                    model: "llama-3.3-70b-versatile".to_string(),
                    max_tokens: 4096,
                    temperature: 0.3,
                    prompt_file: "SOUL.md".to_string(),
                }),
                ..Default::default()
            },
            soul_content: Some("You are a test agent.".to_string()),
            ..Default::default()
        };

        let manifest = template.instantiate(&HashMap::new());

        assert_eq!(manifest.name, "Test Template");
        assert_eq!(manifest.model.provider, "groq");
        assert_eq!(manifest.model.system_prompt, "You are a test agent.");
        assert!(manifest.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_template_with_settings() {
        let template = AgentTemplate {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            settings: vec![TemplateSetting {
                key: "style".to_string(),
                label: "Style".to_string(),
                setting_type: SettingType::Select,
                default: "concise".to_string(),
                options: vec![
                    SettingOption {
                        value: "concise".to_string(),
                        label: "Concise".to_string(),
                        provider_env: None,
                    },
                    SettingOption {
                        value: "verbose".to_string(),
                        label: "Verbose".to_string(),
                        provider_env: None,
                    },
                ],
                description: None,
            }],
            ..Default::default()
        };

        let mut settings = HashMap::new();
        settings.insert("style".to_string(), serde_json::json!("verbose"));

        let manifest = template.instantiate(&settings);
        assert_eq!(manifest.metadata.get("style"), Some(&serde_json::json!("verbose")));
    }

    #[test]
    fn test_category_display() {
        assert_eq!(AgentCategory::Coding.to_string(), "Coding");
        assert_eq!(AgentCategory::DevOps.to_string(), "DevOps");
        assert_eq!(AgentCategory::General.to_string(), "General");
    }

    #[test]
    fn test_default_values() {
        let model = TemplateModelConfig::default();
        assert_eq!(model.provider, "default");
        assert_eq!(model.model, "default");
        assert_eq!(model.max_tokens, 8192);
        assert_eq!(model.temperature, 0.5);
        assert_eq!(model.prompt_file, "SOUL.md");
    }
}
