//! Bundled agent templates — compile-time embedded templates.
//!
//! This module embeds all default expert templates into the binary using
//! `include_str!()`, enabling zero-dependency cold starts.

use crate::{AgentTemplate, AgentTemplateError, AgentTemplateResult, TemplateSource};

/// Get all bundled templates as (id, AGENT.toml, SOUL.md, BOOTSTRAP.md) tuples.
pub fn bundled_templates() -> Vec<(&'static str, &'static str, Option<&'static str>, Option<&'static str>)> {
    vec![
        // Programming expert
        (
            "oh-my-opencode",
            include_str!("../templates/oh-my-opencode/AGENT.toml"),
            Some(include_str!("../templates/oh-my-opencode/SOUL.md")),
            Some(include_str!("../templates/oh-my-opencode/BOOTSTRAP.md")),
        ),
        // Superpower (generalist)
        (
            "superpower",
            include_str!("../templates/superpower/AGENT.toml"),
            Some(include_str!("../templates/superpower/SOUL.md")),
            Some(include_str!("../templates/superpower/BOOTSTRAP.md")),
        ),
        // Security expert
        (
            "security-expert",
            include_str!("../templates/security-expert/AGENT.toml"),
            Some(include_str!("../templates/security-expert/SOUL.md")),
            None,
        ),
        // Data analyst
        (
            "data-analyst",
            include_str!("../templates/data-analyst/AGENT.toml"),
            Some(include_str!("../templates/data-analyst/SOUL.md")),
            None,
        ),
        // DevOps engineer
        (
            "devops-engineer",
            include_str!("../templates/devops-engineer/AGENT.toml"),
            Some(include_str!("../templates/devops-engineer/SOUL.md")),
            None,
        ),
        // Technical writer
        (
            "technical-writer",
            include_str!("../templates/technical-writer/AGENT.toml"),
            Some(include_str!("../templates/technical-writer/SOUL.md")),
            None,
        ),
    ]
}

/// Parse a bundled template from its TOML and optional content files.
pub fn parse_bundled(
    id: &str,
    toml_content: &str,
    soul_content: Option<&str>,
    bootstrap_content: Option<&str>,
) -> AgentTemplateResult<AgentTemplate> {
    let mut template: AgentTemplate =
        toml::from_str(toml_content).map_err(|e| AgentTemplateError::TomlParse(e.to_string()))?;

    // Override ID if not set in TOML
    if template.id.is_empty() {
        template.id = id.to_string();
    }

    // Set content
    template.soul_content = soul_content.map(|s| s.to_string());
    template.bootstrap_content = bootstrap_content.map(|s| s.to_string());
    template.source = Some(TemplateSource::Bundled);

    Ok(template)
}

/// Parse a template from raw strings (for API-based installation).
pub fn parse_from_content(
    toml_content: &str,
    soul_content: Option<&str>,
    bootstrap_content: Option<&str>,
) -> AgentTemplateResult<AgentTemplate> {
    let mut template: AgentTemplate =
        toml::from_str(toml_content).map_err(|e| AgentTemplateError::TomlParse(e.to_string()))?;

    template.soul_content = soul_content.map(|s| s.to_string());
    template.bootstrap_content = bootstrap_content.map(|s| s.to_string());
    template.source = Some(TemplateSource::UserInstalled);

    Ok(template)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_templates_count() {
        let templates = bundled_templates();
        assert!(templates.len() >= 6, "Should have at least 6 bundled templates");
    }

    #[test]
    fn test_parse_bundled_oh_my_opencode() {
        let templates = bundled_templates();
        let (id, toml, soul, bootstrap) = templates
            .iter()
            .find(|(id, _, _, _)| *id == "oh-my-opencode")
            .expect("oh-my-opencode template should exist");

        let template = parse_bundled(id, toml, *soul, *bootstrap).unwrap();

        assert_eq!(template.id, "oh-my-opencode");
        assert!(!template.name.is_empty());
        assert!(template.soul_content.is_some());
        assert!(template.bootstrap_content.is_some());
        assert_eq!(template.source, Some(TemplateSource::Bundled));
    }

    #[test]
    fn test_parse_bundled_all_templates() {
        let templates = bundled_templates();

        for (id, toml, soul, bootstrap) in templates {
            let template = parse_bundled(id, toml, soul, bootstrap);
            assert!(template.is_ok(), "Failed to parse template '{}': {:?}", id, template.err());

            let template = template.unwrap();
            assert_eq!(template.id, id);
            assert!(!template.name.is_empty());
        }
    }

    #[test]
    fn test_parse_invalid_toml() {
        let result = parse_bundled("test", "invalid toml [", None, None);
        assert!(result.is_err());
    }
}
