//! Template management module
//!
//! This module handles user-defined note templates stored in <storage_path>/templates/
//! Templates are TOML files that define boilerplate content for new notes.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};

/// A note template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Display name for the template
    pub name: String,

    /// Title configuration
    #[serde(default)]
    pub title: TitleConfig,

    /// Content configuration
    pub content: ContentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TitleConfig {
    /// Template string for the title (supports variables)
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfig {
    /// Template string for the content (supports variables)
    pub template: String,
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            template: String::new(),
        }
    }
}

impl Template {
    /// Create a new empty template
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            title: TitleConfig::default(),
            content: ContentConfig::default(),
        }
    }

    /// Load a template from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context("failed to read template file")?;
        let template: Template = toml::from_str(&content).context("failed to parse template")?;
        Ok(template)
    }

    /// Save the template to a TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize template")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create templates directory")?;
        }

        let mut file = fs::File::create(path).context("failed to create template file")?;
        file.write_all(content.as_bytes())
            .context("failed to write template file")?;

        Ok(())
    }

    /// Render the template with variable substitution
    pub fn render(&self) -> RenderedTemplate {
        let vars = TemplateVariables::now();

        let title = self.title.template.as_ref().map(|t| vars.substitute(t));

        let content = vars.substitute(&self.content.template);

        RenderedTemplate { title, content }
    }
}

/// Result of rendering a template
#[derive(Debug, Clone)]
pub struct RenderedTemplate {
    pub title: Option<String>,
    pub content: String,
}

/// Variables available for template substitution
#[derive(Debug, Clone)]
pub struct TemplateVariables {
    pub date: String,     // YYYY-MM-DD
    pub datetime: String, // YYYY-MM-DD HH:MM
    pub time: String,     // HH:MM
    pub weekday: String,  // Monday, Tuesday, etc.
    pub year: String,     // YYYY
    pub month: String,    // MM
    pub day: String,      // DD
}

impl TemplateVariables {
    /// Create variables for the current time
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            date: now.format("%Y-%m-%d").to_string(),
            datetime: now.format("%Y-%m-%d %H:%M").to_string(),
            time: now.format("%H:%M").to_string(),
            weekday: now.format("%A").to_string(),
            year: now.format("%Y").to_string(),
            month: now.format("%m").to_string(),
            day: now.format("%d").to_string(),
        }
    }

    /// Substitute variables in a template string
    pub fn substitute(&self, template: &str) -> String {
        template
            .replace("{date}", &self.date)
            .replace("{datetime}", &self.datetime)
            .replace("{time}", &self.time)
            .replace("{weekday}", &self.weekday)
            .replace("{year}", &self.year)
            .replace("{month}", &self.month)
            .replace("{day}", &self.day)
    }
}

/// Template manager for CRUD operations
#[derive(Debug)]
pub struct TemplateManager {
    templates_dir: PathBuf,
}

impl TemplateManager {
    /// Create a new template manager for the given templates directory
    pub fn new(templates_dir: PathBuf) -> Self {
        Self { templates_dir }
    }

    /// Ensure the templates directory exists
    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.templates_dir).context("failed to create templates directory")?;
        Ok(())
    }

    /// Get the path for a template by name
    pub fn template_path(&self, name: &str) -> PathBuf {
        let filename = sanitize_filename(name);
        self.templates_dir.join(format!("{}.toml", filename))
    }

    /// List all available templates
    pub fn list(&self) -> Result<Vec<TemplateSummary>> {
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
                        path,
                    });
                }
                Err(_) => {
                    // Skip invalid templates
                    continue;
                }
            }
        }

        // Sort by name
        templates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(templates)
    }

    /// Load a template by filename (without extension)
    pub fn load(&self, filename: &str) -> Result<Template> {
        let path = self.template_path(filename);
        Template::load(&path)
    }

    /// Save a template
    pub fn save(&self, filename: &str, template: &Template) -> Result<()> {
        self.ensure_dir()?;
        let path = self.template_path(filename);
        template.save(&path)
    }

    /// Delete a template
    pub fn delete(&self, filename: &str) -> Result<()> {
        let path = self.template_path(filename);
        if path.exists() {
            fs::remove_file(&path).context("failed to delete template")?;
        }
        Ok(())
    }

    /// Check if the default template exists
    pub fn has_default(&self) -> bool {
        self.template_path("default").exists()
    }

    /// Load the default template if it exists
    pub fn load_default(&self) -> Option<Template> {
        self.load("default").ok()
    }

    /// Check if any templates exist
    pub fn has_templates(&self) -> bool {
        self.list().map(|t| !t.is_empty()).unwrap_or(false)
    }

    /// Create example templates if none exist
    pub fn create_examples(&self) -> Result<()> {
        if self.has_templates() {
            return Ok(());
        }

        self.ensure_dir()?;

        // Meeting notes template
        let meeting = Template {
            name: "Meeting Notes".to_string(),
            title: TitleConfig {
                template: Some("Meeting - {date}".to_string()),
            },
            content: ContentConfig {
                template: r#"# Meeting Notes

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

"#
                .to_string(),
            },
        };
        self.save("meeting", &meeting)?;

        // Todo list template
        let todo = Template {
            name: "Todo List".to_string(),
            title: TitleConfig {
                template: Some("Tasks - {date}".to_string()),
            },
            content: ContentConfig {
                template: r#"# Tasks for {weekday}, {date}

## High Priority

- [ ] 

## Normal Priority

- [ ] 

## Low Priority

- [ ] 

## Notes

"#
                .to_string(),
            },
        };
        self.save("todo", &todo)?;

        // Journal entry template
        let journal = Template {
            name: "Journal Entry".to_string(),
            title: TitleConfig {
                template: Some("Journal - {date}".to_string()),
            },
            content: ContentConfig {
                template: r#"# {weekday}, {date}

## How I'm feeling



## What happened today



## Grateful for

1. 
2. 
3. 

## Tomorrow's focus

"#
                .to_string(),
            },
        };
        self.save("journal", &journal)?;

        Ok(())
    }
}

/// Summary of a template for listing
#[derive(Debug, Clone)]
pub struct TemplateSummary {
    /// Filename without extension
    pub filename: String,
    /// Display name from the template
    pub name: String,
    /// Full path to the template file
    pub path: PathBuf,
}

/// Sanitize a string for use as a filename
fn sanitize_filename(name: &str) -> String {
    let mut result = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_variables_substitution() {
        let vars = TemplateVariables {
            date: "2026-03-28".to_string(),
            datetime: "2026-03-28 14:30".to_string(),
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
