use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::variables::TemplateVariables;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,

    #[serde(default)]
    pub title: TitleConfig,

    pub content: ContentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TitleConfig {
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentConfig {
    #[serde(default)]
    pub template: String,
}

#[derive(Debug, Clone)]
pub struct RenderedTemplate {
    pub title: Option<String>,
    pub content: String,
}

impl Template {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context("failed to read template file")?;
        toml::from_str::<Template>(&content).context("failed to parse template")
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize template")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create templates directory")?;
        }

        crate::fsutil::atomic_write_str(path, &content).context("failed to write template file")?;

        Ok(())
    }

    pub fn render(&self) -> RenderedTemplate {
        let vars = TemplateVariables::now();

        let title = self.title.template.as_ref().map(|t| vars.substitute(t));

        let content = vars.substitute(&self.content.template);

        RenderedTemplate { title, content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_toml_roundtrip() {
        let template = Template {
            name: "Test".to_string(),
            title: TitleConfig {
                template: Some("Title - {date}".to_string()),
            },
            content: ContentConfig {
                template: "Content here".to_string(),
            },
        };

        let toml_str = toml::to_string_pretty(&template).unwrap();
        let parsed: Template = toml::from_str(&toml_str).unwrap();

        assert_eq!(template.name, parsed.name);
        assert_eq!(template.title.template, parsed.title.template);
        assert_eq!(template.content.template, parsed.content.template);
    }
}
