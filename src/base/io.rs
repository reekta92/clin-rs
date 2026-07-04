use super::model::{BaseFile, BaseView, ViewType};
use anyhow::{Context, Result};

pub fn parse_base(text: &str) -> Result<BaseFile> {
    serde_yaml_ng::from_str(text).context("failed to parse .base file")
}

pub fn serialize_base(base: &BaseFile) -> Result<String> {
    serde_yaml_ng::to_string(base).context("failed to serialize .base file")
}

pub fn default_base_file() -> BaseFile {
    BaseFile {
        views: vec![BaseView {
            r#type: ViewType::Table,
            name: Some("Table".into()),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::model::*;

    #[test]
    fn parse_full_obsidian_example() {
        let yaml = r#"
filters:
  or:
    - expr: 'status == "active"'
    - expr: "rating > 3"
formulas:
  formatted_price: '"$" + price'
  ppu: "price / qty"
properties:
  status:
    displayName: Status
  price:
    displayName: Price
views:
  - type: table
    name: Table View
    limit: 10
    filters:
      expr: "qty > 0"
    groupBy:
      property: status
      direction: ASC
    order:
      - file.name
      - file.ext
      - note.age
      - formula.ppu
      - formula.formatted_price
    summaries:
      formula.ppu: Average
"#;
        let base = parse_base(yaml).expect("failed to parse example base");
        assert!(matches!(base.filters, Some(FilterNode::Map(_))));
        assert_eq!(
            base.formulas.get("formatted_price").map(|s| s.as_str()),
            Some("\"$\" + price")
        );
        assert_eq!(
            base.formulas.get("ppu").map(|s| s.as_str()),
            Some("price / qty")
        );
        assert_eq!(base.views.len(), 1);
        let view = &base.views[0];
        assert_eq!(view.r#type, ViewType::Table);
        assert_eq!(view.name.as_deref(), Some("Table View"));
        assert_eq!(view.limit, Some(10));
        assert_eq!(
            view.order,
            vec![
                "file.name",
                "file.ext",
                "note.age",
                "formula.ppu",
                "formula.formatted_price"
            ]
        );
        assert_eq!(
            view.summaries
                .as_ref()
                .and_then(|m| m.get("formula.ppu"))
                .map(|s| s.as_str()),
            Some("Average")
        );

        let serialized = serialize_base(&base).expect("failed to serialize base");
        let base2 = parse_base(&serialized).expect("failed to re-parse serialized base");
        assert_eq!(base, base2);
    }
}
