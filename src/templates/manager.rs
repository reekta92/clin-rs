use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::model::{ContentConfig, Template, TitleConfig};
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

#[derive(Debug)]
pub struct TemplateManager {
    templates_dir: PathBuf,
}

impl TemplateManager {
    pub fn new(templates_dir: PathBuf) -> Self {
        Self { templates_dir }
    }

    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.templates_dir).context("failed to create templates directory")?;
        Ok(())
    }

    pub fn template_path(&self, name: &str) -> PathBuf {
        let filename = sanitize_filename(name);
        self.templates_dir.join(format!("{filename}.toml"))
    }

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

    pub fn load(&self, filename: &str) -> Result<Template> {
        let path = self.template_path(filename);
        Template::load(&path)
    }

    pub fn save(&self, filename: &str, template: &Template) -> Result<()> {
        self.ensure_dir()?;
        let path = self.template_path(filename);
        template.save(&path)
    }

    pub fn delete(&self, filename: &str) -> Result<()> {
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

    pub fn load_default(&self) -> Option<Template> {
        self.load("default").ok()
    }

    pub fn has_templates(&self) -> bool {
        self.list().map(|t| !t.is_empty()).unwrap_or(false)
    }

    pub fn create_examples(&self) -> Result<()> {
        if self.has_templates() {
            return Ok(());
        }

        self.ensure_dir()?;

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
        };
        self.save("meeting", &meeting)?;

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
        };
        self.save("todo", &todo)?;

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
        };
        self.save("journal", &journal)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let manager = TemplateManager::new(dir.path().to_path_buf());
        let template = Template {
            name: "Temp".to_string(),
            title: TitleConfig {
                template: Some("Temp {date}".to_string()),
            },
            content: ContentConfig {
                template: "Body".to_string(),
            },
        };

        manager.save("to-delete", &template).unwrap();
        let path = manager.template_path("to-delete");
        assert!(path.exists());

        manager.delete("to-delete").unwrap();
        assert!(!path.exists());
    }
}
