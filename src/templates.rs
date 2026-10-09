use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,

    #[serde(default)]
    pub title: TitleConfig,

    pub content: ContentConfig,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub properties: std::collections::BTreeMap<String, toml::Value>,
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
    pub properties: Vec<crate::frontmatter::FrontmatterEdit>,
    pub header: Option<String>,
    pub tags: Vec<String>,
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

    pub fn render(
        &self,
        definitions: &crate::property_model::PropertyDefinitions,
        overrides: &[crate::frontmatter::FrontmatterEdit],
    ) -> Result<RenderedTemplate> {
        let vars = TemplateVariables::now();
        let source = vars.substitute(&self.content.template);
        let (source_header, body) = crate::frontmatter::split_header(source.as_bytes())?;
        let body = std::str::from_utf8(body)?;
        let source_mapping = source_header
            .map(crate::frontmatter::validate_header)
            .transpose()?
            .unwrap_or_default();
        let mut edits = definitions.defaults()?;
        for edit in &mut edits {
            if let Some(source) = &edit.value_yaml {
                let mut value = serde_yaml_ng::from_str(source)?;
                substitute_property_dates(&mut value, &vars);
                edit.value_yaml = Some(serde_yaml_ng::to_string(&value)?);
            }
        }
        edits.retain(|edit| {
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&edit.key_yaml)
                .is_ok_and(|key| !source_mapping.contains_key(&key))
        });
        for (key, value) in &self.properties {
            let mut value = crate::property_model::toml_to_yaml(value)?;
            substitute_property_dates(&mut value, &vars);
            anyhow::ensure!(
                !serde_yaml_ng::to_string(&value)?.contains("{prop:"),
                "Property placeholders belong in title/body, not property values"
            );
            let edit = crate::property_model::property_edit(key, Some(&value))?;
            edits.retain(|old| old.key_yaml != edit.key_yaml);
            edits.push(edit);
        }
        for edit in overrides {
            let key: serde_yaml_ng::Value = serde_yaml_ng::from_str(&edit.key_yaml)?;
            crate::property_model::validate_key(
                key.as_str()
                    .context("Creation properties require string keys")?,
            )?;
            edits.retain(|old| old.key_yaml != edit.key_yaml);
            edits.push(edit.clone());
        }
        let header =
            crate::frontmatter::apply_edits(source_header.unwrap_or("---\n---\n"), &edits)?;
        let frontmatter = crate::frontmatter::checked_parse(&header)?;
        for (key, value) in &frontmatter.extra {
            anyhow::ensure!(
                !serde_yaml_ng::to_string(value)?.contains("{prop:"),
                "Property placeholders belong in title/body, not property values ({})",
                key.as_str().unwrap_or("YAML key")
            );
            if let Some(key) = key.as_str() {
                definitions.validate(key, value)?;
            }
        }
        let values = crate::property_model::properties_from_mapping(&frontmatter.extra);
        let title = self
            .title
            .template
            .as_ref()
            .map(|title| vars.substitute(title))
            .or(frontmatter.title)
            .map(|title| substitute_properties(&title, &values))
            .transpose()?;
        let content = substitute_properties(body, &values)?;
        Ok(RenderedTemplate {
            title,
            content,
            properties: edits,
            header: (source_header.is_some() || !values.is_empty()).then_some(header),
            tags: frontmatter.tags,
        })
    }
}
fn substitute_property_dates(value: &mut serde_yaml_ng::Value, vars: &TemplateVariables) {
    match value {
        serde_yaml_ng::Value::String(value) => *value = vars.substitute(value),
        serde_yaml_ng::Value::Sequence(values) => {
            for value in values {
                substitute_property_dates(value, vars);
            }
        }
        serde_yaml_ng::Value::Mapping(values) => {
            for (_, value) in values {
                substitute_property_dates(value, vars);
            }
        }
        _ => {}
    }
}
pub fn substitute_properties(
    text: &str,
    values: &crate::property_model::PropertyMap,
) -> Result<String> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("{prop:") {
        output.push_str(&rest[..index]);
        rest = &rest[index + 6..];
        let end = rest.find('}').context("Unclosed property placeholder")?;
        let key = &rest[..end];
        let value = values
            .get(key)
            .with_context(|| format!("Missing template property {key}"))?;
        anyhow::ensure!(
            *value != crate::property_model::PropertyValue::Unsupported,
            "Complex property {key} cannot be interpolated"
        );
        output.push_str(&value.display());
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

#[derive(Debug, Clone)]
struct TemplateVariables {
    pub date: String,
    pub datetime: String,
    pub iso_datetime: String,
    pub time: String,
    pub weekday: String,
    pub year: String,
    pub month: String,
    pub day: String,
}

impl TemplateVariables {
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            date: now.format("%Y-%m-%d").to_string(),
            datetime: now.format("%Y-%m-%d %H:%M").to_string(),
            iso_datetime: now.to_rfc3339(),
            time: now.format("%H:%M").to_string(),
            weekday: now.format("%A").to_string(),
            year: now.format("%Y").to_string(),
            month: now.format("%m").to_string(),
            day: now.format("%d").to_string(),
        }
    }

    pub fn substitute(&self, template: &str) -> String {
        template
            .replace("{date}", &self.date)
            .replace("{datetime}", &self.datetime)
            .replace("{iso_datetime}", &self.iso_datetime)
            .replace("{time}", &self.time)
            .replace("{weekday}", &self.weekday)
            .replace("{year}", &self.year)
            .replace("{month}", &self.month)
            .replace("{day}", &self.day)
    }
}

fn sanitize_filename(name: &str) -> String {
    let mut result = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            result.push(c.to_ascii_lowercase());
        } else if c == ' ' {
            result.push('_');
        }
    }
    if result.is_empty() {
        result = "template".to_string();
    }
    result
}

#[derive(Debug, Clone)]
pub struct TemplateSummary {
    pub filename: String,
    pub name: String,
}

impl crate::storage::Storage {
    pub fn ensure_templates_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.templates_dir).context("failed to create templates directory")?;
        Ok(())
    }

    pub fn template_path(&self, name: &str) -> PathBuf {
        let filename = sanitize_filename(name);
        self.templates_dir.join(format!("{filename}.toml"))
    }

    pub fn list_templates(&self) -> Result<Vec<TemplateSummary>> {
        let mut templates = Vec::new();

        if !self.templates_dir.exists() {
            return Ok(templates);
        }

        for entry in
            fs::read_dir(&self.templates_dir).context("failed to read templates directory")?
        {
            let entry = entry.context("failed to read template entry")?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            let filename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            match Template::load(&path) {
                Ok(template) => {
                    templates.push(TemplateSummary {
                        filename,
                        name: template.name,
                    });
                }
                Err(_) => {
                    continue;
                }
            }
        }

        templates.sort_by_key(|a| a.name.to_lowercase());

        Ok(templates)
    }

    pub fn load_template(&self, filename: &str) -> Result<Template> {
        let path = self.template_path(filename);
        Template::load(&path)
    }

    pub fn save_template(&self, filename: &str, template: &Template) -> Result<()> {
        self.ensure_templates_dir()?;
        let path = self.template_path(filename);
        template.save(&path)
    }

    pub fn delete_template(&self, filename: &str) -> Result<()> {
        let path = self.template_path(filename);
        fs::remove_file(&path).with_context(|| {
            format!(
                "failed to delete template file '{}'",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
            )
        })?;
        Ok(())
    }

    pub fn load_default_template(&self) -> Option<Template> {
        self.load_template("default").ok()
    }

    pub fn has_templates(&self) -> bool {
        fs::read_dir(&self.templates_dir)
            .map(|mut e| e.next().is_some())
            .unwrap_or(false)
    }

    pub fn create_example_templates(&self) -> Result<()> {
        if self.has_templates() {
            return Ok(());
        }

        self.ensure_templates_dir()?;

        let meeting = Template {
            name: "Meeting Notes".to_string(),
            title: TitleConfig {
                template: Some("Meeting - {date}".to_string()),
            },
            content: ContentConfig {
                template: r"# Meeting Notes
        
        **Date:** {date}
        **Time:** {time}
        
        ## Attendees
        
        - 
        
        ## Agenda
        
        1. 
        
        ## Discussion
        
        ## Action Items
        
        - [ ] 
        
        ## Next Meeting
        
        "
                .to_string(),
            },
            properties: std::collections::BTreeMap::default(),
        };
        self.save_template("meeting", &meeting)?;

        let todo = Template {
            name: "Todo List".to_string(),
            title: TitleConfig {
                template: Some("Tasks - {date}".to_string()),
            },
            content: ContentConfig {
                template: r"# Tasks for {weekday}, {date}
        
        ## High Priority
        
        - [ ] 
        
        ## Normal Priority
        
        - [ ] 
        
        ## Low Priority
        
        - [ ] 
        
        ## Notes
        
        "
                .to_string(),
            },
            properties: std::collections::BTreeMap::default(),
        };
        self.save_template("todo", &todo)?;

        let journal = Template {
            name: "Journal Entry".to_string(),
            title: TitleConfig {
                template: Some("Journal - {date}".to_string()),
            },
            content: ContentConfig {
                template: r"# {weekday}, {date}
        
        ## How I'm feeling
        
        ## What happened today
        
        ## Grateful for
        
        1. 
        2. 
        3. 
        
        ## Tomorrow's focus
        
        "
                .to_string(),
            },
            properties: std::collections::BTreeMap::default(),
        };
        self.save_template("journal", &journal)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_storage(dir: &std::path::Path) -> crate::storage::Storage {
        crate::storage::Storage {
            data_dir: dir.to_path_buf(),
            config_dir: dir.to_path_buf(),
            notes_dir: dir.to_path_buf(),
            templates_dir: dir.to_path_buf(),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: true,
        }
    }

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
            properties: std::collections::BTreeMap::default(),
        };

        let toml_str = toml::to_string_pretty(&template).unwrap();
        let parsed: Template = toml::from_str(&toml_str).unwrap();

        assert_eq!(template.name, parsed.name);
        assert_eq!(template.title.template, parsed.title.template);
        assert_eq!(template.content.template, parsed.content.template);
    }

    #[test]
    fn test_template_variables_substitution() {
        let vars = TemplateVariables {
            date: "2026-03-28".to_string(),
            datetime: "2026-03-28 14:30".to_string(),
            iso_datetime: String::new(),
            time: "14:30".to_string(),
            weekday: "Saturday".to_string(),
            year: "2026".to_string(),
            month: "03".to_string(),
            day: "28".to_string(),
        };

        let template = "Meeting on {date} at {time}";
        let result = vars.substitute(template);
        assert_eq!(result, "Meeting on 2026-03-28 at 14:30");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Meeting Notes"), "meeting_notes");
        assert_eq!(sanitize_filename("todo-list"), "todo-list");
        assert_eq!(sanitize_filename("My Template!"), "my_template");
        assert_eq!(sanitize_filename(""), "template");
    }

    #[test]
    fn delete_removes_template_file() {
        let dir = tempfile::tempdir().unwrap();
        let storage = test_storage(dir.path());
        let template = Template {
            name: "Temp".to_string(),
            title: TitleConfig {
                template: Some("Temp {date}".to_string()),
            },
            content: ContentConfig {
                template: "Body".to_string(),
            },
            properties: std::collections::BTreeMap::default(),
        };

        storage.save_template("to-delete", &template).unwrap();
        let path = storage.template_path("to-delete");
        assert!(path.exists());

        storage.delete_template("to-delete").unwrap();
        assert!(!path.exists());
    }
}
