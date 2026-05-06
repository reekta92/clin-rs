use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
}




pub fn parse(content: &str) -> (Frontmatter, &str) {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return (Frontmatter::default(), content);
    }

    let end_marker = "\n---";
    if let Some(end_idx) = content[3..].find(end_marker) {
        let frontmatter_str = &content[3..3 + end_idx];

        let remaining_start = 3 + end_idx + end_marker.len();
        
        let mut content_start = remaining_start;
        if content[remaining_start..].starts_with("\r\n") {
            content_start += 2;
        } else if content[remaining_start..].starts_with('\n') {
            content_start += 1;
        }

        let remaining_content = &content[content_start..];

        
        if let Ok(frontmatter) = serde_yml::from_str::<Frontmatter>(frontmatter_str) {
            return (frontmatter, remaining_content);
        }
    }

    (Frontmatter::default(), content)
}



pub fn serialize(frontmatter: &Frontmatter, content: &str) -> String {
    if frontmatter.tags.is_empty() && !frontmatter.pinned {
        return content.to_string();
    }

    match serde_yml::to_string(frontmatter) {
        Ok(yaml) => {
            
            let yaml = yaml.trim();
            let yaml = yaml.to_string();

            
            
            
            
            
            
            format!("---\n{}\n---\n{}", yaml, content)
        }
        Err(_) => content.to_string(), 
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just some text";
        let (fm, remaining) = parse(content);
        assert!(fm.tags.is_empty());
        assert_eq!(remaining, "Just some text");
    }

    #[test]
    fn test_parse_with_frontmatter() {
        let content = "---\ntags:\n  - work\n  - urgent\n---\nHere is the content.";
        let (fm, remaining) = parse(content);
        assert_eq!(fm.tags, vec!["work", "urgent"]);
        assert_eq!(remaining, "Here is the content.");
    }

    #[test]
    fn test_serialize() {
        let fm = Frontmatter {
            tags: vec!["work".to_string()],
            pinned: false,
        };
        let content = "My note";
        let serialized = serialize(&fm, content);
        assert!(serialized.starts_with("---\n"));
        assert!(serialized.contains("work"));
        assert!(serialized.ends_with("\n---\nMy note"));
    }
}
