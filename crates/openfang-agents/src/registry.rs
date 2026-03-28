//! Agent template registry — manages template definitions.

use crate::bundled;
use crate::{
    AgentTemplate, AgentTemplateError, AgentTemplateResult, InstalledTemplate, RequirementStatus,
    RequirementType, TemplateReadiness,
};
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// The agent template registry — stores templates and tracks installations.
pub struct AgentTemplateRegistry {
    /// All known templates, keyed by template ID.
    templates: DashMap<String, InstalledTemplate>,
    /// Directory for user-installed templates.
    templates_dir: PathBuf,
}

impl AgentTemplateRegistry {
    /// Create a new registry with the given templates directory.
    pub fn new(templates_dir: PathBuf) -> Self {
        Self {
            templates: DashMap::new(),
            templates_dir,
        }
    }

    /// Load all bundled (compile-time embedded) templates.
    /// Returns the count of templates loaded.
    pub fn load_bundled(&self) -> usize {
        let bundled = bundled::bundled_templates();
        let mut count = 0;

        for (id, toml_content, soul_content, bootstrap_content) in bundled {
            match bundled::parse_bundled(id, toml_content, soul_content, bootstrap_content) {
                Ok(template) => {
                    info!(
                        template = %template.id,
                        name = %template.name,
                        "Loaded bundled agent template"
                    );
                    self.templates.insert(
                        template.id.clone(),
                        InstalledTemplate {
                            template,
                            path: PathBuf::from("<bundled>"),
                            enabled: true,
                            installed_at: None,
                        },
                    );
                    count += 1;
                }
                Err(e) => {
                    warn!(template = %id, error = %e, "Failed to parse bundled template");
                }
            }
        }

        count
    }

    /// Load user-installed templates from the templates directory.
    /// Returns the count of templates loaded.
    pub fn load_installed(&self) -> AgentTemplateResult<usize> {
        if !self.templates_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in std::fs::read_dir(&self.templates_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Look for AGENT.toml
            let toml_path = path.join("AGENT.toml");
            if !toml_path.exists() {
                continue;
            }

            match self.load_template_from_dir(&path) {
                Ok(template) => {
                    // Don't override bundled templates with same ID
                    if !self.templates.contains_key(&template.id) {
                        info!(
                            template = %template.id,
                            name = %template.name,
                            path = %path.display(),
                            "Loaded installed agent template"
                        );
                        self.templates.insert(
                            template.id.clone(),
                            InstalledTemplate {
                                template,
                                path,
                                enabled: true,
                                installed_at: Some(chrono::Utc::now()),
                            },
                        );
                        count += 1;
                    }
                }
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to load template");
                }
            }
        }

        Ok(count)
    }

    /// Load a template from a directory.
    fn load_template_from_dir(&self, path: &Path) -> AgentTemplateResult<AgentTemplate> {
        let toml_path = path.join("AGENT.toml");
        let toml_content = std::fs::read_to_string(&toml_path)?;

        let mut template: AgentTemplate =
            toml::from_str(&toml_content).map_err(|e| AgentTemplateError::TomlParse(e.to_string()))?;

        // Load SOUL.md if present
        let soul_path = path.join("SOUL.md");
        if soul_path.exists() {
            template.soul_content = Some(std::fs::read_to_string(soul_path)?);
        }

        // Load BOOTSTRAP.md if present
        let bootstrap_path = path.join("BOOTSTRAP.md");
        if bootstrap_path.exists() {
            template.bootstrap_content = Some(std::fs::read_to_string(bootstrap_path)?);
        }

        template.source = Some(crate::TemplateSource::UserInstalled);

        Ok(template)
    }

    /// List all available templates.
    pub fn list(&self) -> Vec<AgentTemplate> {
        let mut templates: Vec<AgentTemplate> = self
            .templates
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.template.clone())
            .collect();
        templates.sort_by(|a, b| a.name.cmp(&b.name));
        templates
    }

    /// List all installed templates (including disabled ones).
    pub fn list_all(&self) -> Vec<InstalledTemplate> {
        let mut templates: Vec<InstalledTemplate> =
            self.templates.iter().map(|t| t.value().clone()).collect();
        templates.sort_by(|a, b| a.template.name.cmp(&b.template.name));
        templates
    }

    /// Get a specific template by ID.
    pub fn get(&self, id: &str) -> Option<AgentTemplate> {
        self.templates.get(id).map(|t| t.template.clone())
    }

    /// Get an installed template by ID (with metadata).
    pub fn get_installed(&self, id: &str) -> Option<InstalledTemplate> {
        self.templates.get(id).map(|t| t.value().clone())
    }

    /// Check if a template exists.
    pub fn exists(&self, id: &str) -> bool {
        self.templates.contains_key(id)
    }

    /// Enable or disable a template.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> AgentTemplateResult<()> {
        let mut entry = self
            .templates
            .get_mut(id)
            .ok_or_else(|| AgentTemplateError::NotFound(id.to_string()))?;
        entry.enabled = enabled;
        Ok(())
    }

    /// Instantiate a template into an AgentManifest.
    pub fn instantiate(
        &self,
        id: &str,
        settings: HashMap<String, serde_json::Value>,
    ) -> AgentTemplateResult<openfang_types::agent::AgentManifest> {
        let installed = self
            .templates
            .get(id)
            .ok_or_else(|| AgentTemplateError::NotFound(id.to_string()))?;

        if !installed.enabled {
            return Err(AgentTemplateError::InstantiateFailed(format!(
                "Template '{}' is disabled",
                id
            )));
        }

        Ok(installed.template.instantiate(&settings))
    }

    /// Install a template from a directory.
    pub fn install(&self, path: &Path) -> AgentTemplateResult<AgentTemplate> {
        let template = self.load_template_from_dir(path)?;

        if self.templates.contains_key(&template.id) {
            return Err(AgentTemplateError::AlreadyExists(template.id.clone()));
        }

        // Copy to templates directory
        let dest_dir = self.templates_dir.join(&template.id);
        std::fs::create_dir_all(&dest_dir)?;

        // Copy AGENT.toml
        std::fs::copy(path.join("AGENT.toml"), dest_dir.join("AGENT.toml"))?;

        // Copy SOUL.md if present
        let soul_src = path.join("SOUL.md");
        if soul_src.exists() {
            std::fs::copy(soul_src, dest_dir.join("SOUL.md"))?;
        }

        // Copy BOOTSTRAP.md if present
        let bootstrap_src = path.join("BOOTSTRAP.md");
        if bootstrap_src.exists() {
            std::fs::copy(bootstrap_src, dest_dir.join("BOOTSTRAP.md"))?;
        }

        info!(
            template = %template.id,
            name = %template.name,
            "Installed agent template"
        );

        self.templates.insert(
            template.id.clone(),
            InstalledTemplate {
                template: template.clone(),
                path: dest_dir,
                enabled: true,
                installed_at: Some(chrono::Utc::now()),
            },
        );

        Ok(template)
    }

    /// Install a template from raw content.
    pub fn install_from_content(
        &self,
        toml_content: &str,
        soul_content: &str,
        bootstrap_content: Option<&str>,
    ) -> AgentTemplateResult<AgentTemplate> {
        let template = bundled::parse_bundled("custom", toml_content, Some(soul_content), bootstrap_content)?;

        if self.templates.contains_key(&template.id) {
            return Err(AgentTemplateError::AlreadyExists(template.id.clone()));
        }

        // Create directory and write files
        let dest_dir = self.templates_dir.join(&template.id);
        std::fs::create_dir_all(&dest_dir)?;

        std::fs::write(dest_dir.join("AGENT.toml"), toml_content)?;
        std::fs::write(dest_dir.join("SOUL.md"), soul_content)?;

        if let Some(bootstrap) = bootstrap_content {
            std::fs::write(dest_dir.join("BOOTSTRAP.md"), bootstrap)?;
        }

        info!(
            template = %template.id,
            name = %template.name,
            "Installed agent template from content"
        );

        self.templates.insert(
            template.id.clone(),
            InstalledTemplate {
                template: template.clone(),
                path: dest_dir,
                enabled: true,
                installed_at: Some(chrono::Utc::now()),
            },
        );

        Ok(template)
    }

    /// Uninstall a user-installed template.
    pub fn uninstall(&self, id: &str) -> AgentTemplateResult<()> {
        let installed = self
            .templates
            .get(id)
            .ok_or_else(|| AgentTemplateError::NotFound(id.to_string()))?;

        // Can't uninstall bundled templates
        if installed.template.source == Some(crate::TemplateSource::Bundled) {
            return Err(AgentTemplateError::InvalidTemplate(
                "Cannot uninstall bundled template".to_string(),
            ));
        }

        // Remove directory
        if installed.path.exists() && installed.path != std::path::Path::new("<bundled>") {
            std::fs::remove_dir_all(&installed.path)?;
        }

        // Remove from registry
        self.templates.remove(id);

        info!(template = %id, "Uninstalled agent template");

        Ok(())
    }

    /// Check requirements for a template.
    pub fn check_requirements(&self, id: &str) -> AgentTemplateResult<TemplateReadiness> {
        let template = self
            .templates
            .get(id)
            .ok_or_else(|| AgentTemplateError::NotFound(id.to_string()))?;

        let requirements: Vec<RequirementStatus> = template
            .template
            .requires
            .iter()
            .map(|req| {
                let satisfied = match req.requirement_type {
                    RequirementType::Binary => crate::check_binary(&req.check_value),
                    RequirementType::EnvVar | RequirementType::ApiKey => {
                        crate::check_env_var(&req.check_value)
                    }
                };
                RequirementStatus {
                    key: req.key.clone(),
                    label: req.label.clone(),
                    requirement_type: req.requirement_type.clone(),
                    satisfied,
                    optional: req.optional,
                }
            })
            .collect();

        let requirements_met = requirements
            .iter()
            .all(|r| r.satisfied || r.optional);

        Ok(TemplateReadiness {
            requirements_met,
            requirements,
        })
    }

    /// Get settings status for a template (with availability checks).
    pub fn get_settings_status(
        &self,
        id: &str,
    ) -> AgentTemplateResult<Vec<SettingStatus>> {
        let template = self
            .templates
            .get(id)
            .ok_or_else(|| AgentTemplateError::NotFound(id.to_string()))?;

        Ok(template
            .template
            .settings
            .iter()
            .map(|setting| {
                let options = setting
                    .options
                    .iter()
                    .map(|opt| {
                        let available = opt
                            .provider_env
                            .as_ref()
                            .map(|env| crate::check_env_var(env))
                            .unwrap_or(true);
                        SettingOptionStatus {
                            value: opt.value.clone(),
                            label: opt.label.clone(),
                            provider_env: opt.provider_env.clone(),
                            available,
                        }
                    })
                    .collect();

                SettingStatus {
                    key: setting.key.clone(),
                    label: setting.label.clone(),
                    setting_type: setting.setting_type.clone(),
                    default: setting.default.clone(),
                    options,
                    description: setting.description.clone(),
                }
            })
            .collect())
    }

    /// Get the count of templates.
    pub fn count(&self) -> usize {
        self.templates.len()
    }

    /// Get the count of enabled templates.
    pub fn count_enabled(&self) -> usize {
        self.templates.iter().filter(|t| t.enabled).count()
    }
}

impl Default for AgentTemplateRegistry {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

// ─── Settings status types (for API responses) ───────────────────────────────

/// Status of a single setting option.
#[derive(Debug, Clone, Serialize)]
pub struct SettingOptionStatus {
    pub value: String,
    pub label: String,
    pub provider_env: Option<String>,
    pub available: bool,
}

/// Setting with per-option availability info.
#[derive(Debug, Clone, Serialize)]
pub struct SettingStatus {
    pub key: String,
    pub label: String,
    pub setting_type: crate::SettingType,
    pub default: String,
    pub options: Vec<SettingOptionStatus>,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registry_is_empty() {
        let reg = AgentTemplateRegistry::new(PathBuf::from("/tmp"));
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn load_bundled_templates() {
        let reg = AgentTemplateRegistry::new(PathBuf::from("/tmp"));
        let count = reg.load_bundled();
        assert!(count > 0);
        assert!(!reg.list().is_empty());
    }

    #[test]
    fn get_nonexistent_template() {
        let reg = AgentTemplateRegistry::new(PathBuf::from("/tmp"));
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn instantiate_nonexistent_template() {
        let reg = AgentTemplateRegistry::new(PathBuf::from("/tmp"));
        let result = reg.instantiate("nonexistent", HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn check_requirements_nonexistent() {
        let reg = AgentTemplateRegistry::new(PathBuf::from("/tmp"));
        let result = reg.check_requirements("nonexistent");
        assert!(result.is_err());
    }
}
