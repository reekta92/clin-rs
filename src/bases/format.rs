//! Bases configuration is standalone YAML, not a clin note/frontmatter wrapper.

use crate::frontmatter::properties::{parse_mapping, replacement_node};
use anyhow::{Context, Result, bail};
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;
use std::str::FromStr;
use yaml_edit::AsYaml;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    Expression(String),
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Vec<Filter>),
}

impl Default for Filter {
    fn default() -> Self {
        Self::And(Vec::new())
    }
}

impl Filter {
    fn parse(value: &Value) -> Result<Self> {
        match value {
            Value::String(expression) => Ok(Self::Expression(expression.clone())),
            Value::Mapping(mapping) if mapping.len() == 1 => {
                let (operator, operands) =
                    mapping.iter().next().context("missing filter operator")?;
                let operator = operator
                    .as_str()
                    .context("filter operator must be a string")?;
                let operands = operands
                    .as_sequence()
                    .context("filter operands must be a list")?;
                let filters = operands
                    .iter()
                    .map(Self::parse)
                    .collect::<Result<Vec<_>>>()?;
                match operator {
                    "and" => Ok(Self::And(filters)),
                    "or" => Ok(Self::Or(filters)),
                    "not" => Ok(Self::Not(filters)),
                    _ => bail!("unknown filter operator {operator:?}"),
                }
            }
            _ => bail!("filter must be an expression or exactly one and/or/not list"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    Table,
    List,
    Cards,
    Kanban,
    Unsupported(String),
}

impl Layout {
    pub fn from_name(name: &str) -> Self {
        match name {
            "table" => Self::Table,
            "list" => Self::List,
            "cards" => Self::Cards,
            "kanban" => Self::Kanban,
            other => Self::Unsupported(other.to_owned()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Table => "table",
            Self::List => "list",
            Self::Cards => "cards",
            Self::Kanban => "kanban",
            Self::Unsupported(name) => name,
        }
    }

    pub fn supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertySort {
    pub property: String,
    pub direction: Direction,
}

impl PropertySort {
    fn parse(value: &Value) -> Result<Self> {
        let mapping = value
            .as_mapping()
            .context("sort/group configuration must be a mapping")?;
        let property = required_string(mapping, "property")?.to_owned();
        let direction = match optional_string(mapping, "direction")?.unwrap_or("ASC") {
            "ASC" => Direction::Ascending,
            "DESC" => Direction::Descending,
            direction => bail!("unknown sort direction {direction:?}"),
        };
        Ok(Self {
            property,
            direction,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ViewConfig {
    pub name: String,
    pub layout: Layout,
    pub filter: Filter,
    /// Displayed properties, never row sort order.
    pub order: Vec<String>,
    pub sort: Vec<PropertySort>,
    pub group_by: Option<PropertySort>,
    /// None is automatic; Some([]) explicitly hides every group.
    pub group_order: Option<Vec<Value>>,
    pub limit: Option<usize>,
    pub summaries: BTreeMap<String, String>,
    pub options: Mapping,
}

impl ViewConfig {
    fn parse(value: &Value) -> Result<Self> {
        let mapping = value.as_mapping().context("view must be a mapping")?;
        let layout = Layout::from_name(required_string(mapping, "type")?);
        let name = optional_string(mapping, "name")?
            .unwrap_or(layout.name())
            .to_owned();
        let mut view = Self {
            name,
            layout,
            filter: Filter::default(),
            order: Vec::new(),
            sort: Vec::new(),
            group_by: None,
            group_order: None,
            limit: None,
            summaries: BTreeMap::new(),
            options: mapping.clone(),
        };
        // Plugin-specific configuration is retained, not interpreted as core layout settings.
        if !view.layout.supported() {
            return Ok(view);
        }
        view.filter = optional_filter(mapping, "filters")?;
        view.order = optional_sequence(mapping, "order")?
            .unwrap_or_default()
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .context("display order must contain property strings")
            })
            .collect::<Result<_>>()?;
        view.sort = optional_sequence(mapping, "sort")?
            .unwrap_or_default()
            .iter()
            .map(PropertySort::parse)
            .collect::<Result<_>>()?;
        view.group_by = mapping
            .get("groupBy")
            .map(PropertySort::parse)
            .transpose()?;
        view.group_order = optional_sequence(mapping, "groupOrder")?.map(<[Value]>::to_vec);
        view.limit = mapping
            .get("limit")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .context("view limit must be a non-negative integer")
            })
            .transpose()?;
        view.summaries = string_mapping(mapping.get("summaries"), "view summaries")?;
        Ok(view)
    }
}

#[derive(Clone, Debug)]
pub struct BaseConfig {
    pub filter: Filter,
    pub formulas: BTreeMap<String, String>,
    pub summaries: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub views: Vec<ViewConfig>,
}

#[derive(Clone, Debug)]
pub struct BaseDocument {
    source: String,
    pub config: BaseConfig,
}

impl BaseDocument {
    pub fn parse(source: &str) -> Result<Self> {
        let mapping = parse_mapping(source).context("invalid Bases YAML")?;
        let filter = optional_filter(&mapping, "filters")?;
        let formulas = string_mapping(mapping.get("formulas"), "formulas")?;
        let summaries = string_mapping(mapping.get("summaries"), "custom summaries")?;
        let mut labels = BTreeMap::new();
        if let Some(properties) = mapping.get("properties") {
            for (key, value) in properties
                .as_mapping()
                .context("property settings must be a mapping")?
            {
                let key = key
                    .as_str()
                    .context("property setting key must be a string")?;
                let settings = value
                    .as_mapping()
                    .context("property setting must be a mapping")?;
                if let Some(label) = optional_string(settings, "displayName")? {
                    labels.insert(key.to_owned(), label.to_owned());
                }
            }
        }
        let views = optional_sequence(&mapping, "views")?
            .context("base must define a views list")?
            .iter()
            .map(ViewConfig::parse)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            source: source.to_owned(),
            config: BaseConfig {
                filter,
                formulas,
                summaries,
                labels,
                views,
            },
        })
    }

    pub fn new_table() -> Self {
        Self::parse("views:\n  - type: table\n    name: Table\n    order:\n      - file.name\n")
            .expect("built-in base definition")
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn view(&self, name: Option<&str>) -> Result<(usize, &ViewConfig)> {
        let Some(name) = name else {
            return self
                .config
                .views
                .first()
                .map(|view| (0, view))
                .context("base has no views");
        };
        let mut matches = self
            .config
            .views
            .iter()
            .enumerate()
            .filter(|(_, view)| view.name == name);
        let view = matches.next().context("named base view does not exist")?;
        if matches.next().is_some() {
            bail!("base view name is ambiguous");
        }
        Ok(view)
    }

    pub fn set_global_field(&self, key: &str, value: Option<&Value>) -> Result<Self> {
        let yaml = yaml_edit::YamlFile::from_str(&self.source)?;
        let root = yaml.ensure_document();
        if let Some(value) = value {
            root.set(key, replacement_node(value)?);
        } else {
            root.remove(key);
        }
        Self::parse(&preserve_line_endings(&self.source, &yaml.to_string()))
    }

    pub fn set_view_field(&self, index: usize, key: &str, value: Option<&Value>) -> Result<Self> {
        let yaml = yaml_edit::YamlFile::from_str(&self.source)?;
        let root = yaml.document().context("missing base document")?;
        let view = root
            .get_sequence("views")
            .and_then(|views| views.get(index))
            .context("view index out of bounds")?;
        let mapping = view.as_mapping().context("view must be a mapping")?;
        if let Some(value) = value {
            mapping.set(key, replacement_node(value)?);
        } else {
            mapping.remove(key);
        }
        Self::parse(&preserve_line_endings(&self.source, &yaml.to_string()))
    }

    pub fn set_formula(&self, name: &str, expression: Option<&str>) -> Result<Self> {
        if expression.is_none() && !self.config.formulas.contains_key(name) {
            return Ok(self.clone());
        }
        let yaml = yaml_edit::YamlFile::from_str(&self.source)?;
        let root = yaml.document().context("missing base document")?;
        let formulas = if let Some(formulas) = root.get_mapping("formulas") {
            formulas
        } else {
            if !self.config.formulas.is_empty() {
                bail!("formulas mapping cannot be edited losslessly");
            }
            root.set("formulas", yaml_edit::Mapping::new());
            root.get_mapping("formulas")
                .context("could not create formulas mapping")?
        };
        if let Some(expression) = expression {
            formulas.set(
                replacement_node(&Value::String(name.into()))?,
                replacement_node(&Value::String(expression.into()))?,
            );
        } else {
            formulas.remove(name);
        }
        Self::parse(&preserve_line_endings(&self.source, &yaml.to_string()))
    }

    pub fn add_view(&self, layout: &Layout, name: &str) -> Result<Self> {
        if !layout.supported() {
            bail!("plugin views cannot be created in clin");
        }
        if self.config.views.iter().any(|view| view.name == name) {
            bail!("view name already exists");
        }
        let yaml = yaml_edit::YamlFile::from_str(&self.source)?;
        let root = yaml.document().context("missing base document")?;
        let views = root.get_sequence("views").context("missing views list")?;
        let mapping = yaml_edit::Mapping::new_flow();
        mapping.set("type", layout.name());
        mapping.set("name", replacement_node(&Value::String(name.into()))?);
        let order = yaml_edit::Sequence::new_flow();
        order.push("file.name");
        mapping.set("order", order);
        views.push(mapping);
        let mut source = yaml.to_string();
        let end = usize::from(
            views
                .as_node()
                .context("missing views syntax node")?
                .text_range()
                .end(),
        );
        // yaml-edit can omit the separator after an appended flow item before another field.
        if end < source.len()
            && !source[..end].ends_with('\n')
            && !source[end..].starts_with('\n')
            && !source[end..].starts_with('\r')
        {
            source.insert(end, '\n');
        }
        Self::parse(&preserve_line_endings(&self.source, &source))
    }

    pub fn remove_view(&self, index: usize) -> Result<Self> {
        if self.config.views.len() <= 1 {
            bail!("cannot remove the last base view");
        }
        let yaml = yaml_edit::YamlFile::from_str(&self.source)?;
        let views = yaml
            .document()
            .and_then(|root| root.get_sequence("views"))
            .context("missing views list")?;
        if views.remove(index).is_none() {
            bail!("view index out of bounds");
        }
        Self::parse(&preserve_line_endings(&self.source, &yaml.to_string()))
    }
}

fn preserve_line_endings(original: &str, updated: &str) -> String {
    if original.contains("\r\n") && !original.replace("\r\n", "").contains('\n') {
        updated.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        updated.to_owned()
    }
}

fn required_string<'a>(mapping: &'a Mapping, key: &str) -> Result<&'a str> {
    optional_string(mapping, key)?.with_context(|| format!("missing {key:?}"))
}

fn optional_string<'a>(mapping: &'a Mapping, key: &str) -> Result<Option<&'a str>> {
    mapping
        .get(key)
        .map(|value| {
            value
                .as_str()
                .with_context(|| format!("{key:?} must be a string"))
        })
        .transpose()
}

fn optional_sequence<'a>(mapping: &'a Mapping, key: &str) -> Result<Option<&'a [Value]>> {
    mapping
        .get(key)
        .map(|value| {
            value
                .as_sequence()
                .map(Vec::as_slice)
                .with_context(|| format!("{key:?} must be a list"))
        })
        .transpose()
}

fn optional_filter(mapping: &Mapping, key: &str) -> Result<Filter> {
    mapping
        .get(key)
        .map(Filter::parse)
        .transpose()
        .map(Option::unwrap_or_default)
}

fn string_mapping(value: Option<&Value>, label: &str) -> Result<BTreeMap<String, String>> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    value
        .as_mapping()
        .with_context(|| format!("{label} must be a mapping"))?
        .iter()
        .map(|(key, value)| {
            let key = key
                .as_str()
                .context("expression property name must be a string")?;
            let value = value
                .as_str()
                .context("expression/summary name must be a string")?;
            Ok((key.to_owned(), value.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shared_filters_formula_labels_and_manual_group_order() {
        let source = "filters:\n  not: ['done', 'file.inFolder(\"archive\")']\nformulas:\n  total: price * amount\nproperties:\n  formula.total: {displayName: Total, unknown: keep}\nsummaries:\n  custom: values.mean().round(3)\nviews:\n  - type: table\n    name: Main\n    order: [file.name, formula.total]\n    sort: [{property: formula.total, direction: DESC}]\n    groupBy: {property: note.status, direction: ASC}\n    groupOrder: [Open, null]\n    limit: 10\n    summaries: {formula.total: custom}\n";
        let doc = BaseDocument::parse(source).unwrap();
        assert_eq!(doc.source(), source);
        assert!(matches!(doc.config.filter, Filter::Not(_)));
        let view = &doc.config.views[0];
        assert_eq!(view.order, ["file.name", "formula.total"]);
        assert_eq!(view.sort[0].direction, Direction::Descending);
        assert_eq!(view.limit, Some(10));
        assert_eq!(view.group_order.as_ref().unwrap()[1], Value::Null);
        assert_eq!(doc.config.labels["formula.total"], "Total");
    }

    #[test]
    fn empty_manual_order_differs_from_automatic() {
        let doc = BaseDocument::parse("views: [{type: kanban, groupOrder: []}, {type: table}]\n")
            .unwrap();
        assert_eq!(doc.config.views[0].group_order, Some(Vec::new()));
        assert_eq!(doc.config.views[1].group_order, None);
    }

    #[test]
    fn unsupported_views_do_not_prevent_supported_views_or_destroy_source() {
        let source = "# keep\nfuture: {x: 1}\nviews:\n  - type: map\n    name: Map\n    coordinates: location\n    groupBy: plugin-specific\n  - type: table\n    name: Table\n";
        let doc = BaseDocument::parse(source).unwrap();
        assert!(!doc.config.views[0].layout.supported());
        assert!(doc.view(Some("Table")).unwrap().1.layout.supported());
        assert_eq!(doc.source(), source);
        let updated = doc
            .set_view_field(1, "name", Some(&Value::String("Other".into())))
            .unwrap();
        assert!(updated.source().starts_with("# keep\nfuture: {x: 1}\n"));
        assert!(
            updated
                .source()
                .contains("coordinates: location\n    groupBy: plugin-specific\n")
        );
    }

    #[test]
    fn named_views_and_default_view_are_explicit() {
        let doc = BaseDocument::parse("views: [{type: table, name: A}, {type: cards, name: B}]\n")
            .unwrap();
        assert_eq!(doc.view(None).unwrap().0, 0);
        assert_eq!(doc.view(Some("B")).unwrap().0, 1);
        assert!(doc.view(Some("missing")).is_err());
        let doc = BaseDocument::parse("views: [{type: table, name: A}, {type: table, name: A}]\n")
            .unwrap();
        assert!(doc.view(Some("A")).is_err());
    }

    #[test]
    fn mutation_retains_comments_unknown_fields_and_line_endings() {
        for newline in ["\n", "\r\n"] {
            let source = "# header\nfuture: 'exact'\nformulas:\n  original: 'x * 2' # keep\nviews:\n  - type: table\n    name: Old # name comment\n    columns: {x: 100} # untouched\n".replace('\n', newline);
            let doc = BaseDocument::parse(&source).unwrap();
            let doc = doc
                .set_view_field(0, "name", Some(&Value::String("New".into())))
                .unwrap();
            let doc = doc.set_formula("new", Some("x + 3")).unwrap();
            assert!(doc.source().contains(&format!("future: 'exact'{newline}")));
            assert!(
                doc.source()
                    .contains(&format!("original: 'x * 2' # keep{newline}"))
            );
            assert!(
                doc.source()
                    .contains(&format!("columns: {{x: 100}} # untouched{newline}"))
            );
            assert!(doc.source().contains("# name comment"));
            if newline == "\r\n" {
                assert!(!doc.source().replace("\r\n", "").contains('\n'));
            }
        }
    }

    #[test]
    fn authoring_adds_removes_views_and_updates_global_filters() {
        let doc = BaseDocument::new_table();
        let doc = doc.set_formula("total", Some("price * amount")).unwrap();
        let doc = doc
            .add_view(&Layout::List, "List")
            .unwrap()
            .add_view(&Layout::Cards, "Cards")
            .unwrap()
            .add_view(&Layout::Kanban, "Board")
            .unwrap();
        assert_eq!(doc.config.views.len(), 4);
        assert!(doc.add_view(&Layout::Table, "Table").is_err());
        let doc = doc
            .set_global_field("filters", Some(&Value::String("file.ext == 'md'".into())))
            .unwrap();
        assert_eq!(
            doc.config.filter,
            Filter::Expression("file.ext == 'md'".into())
        );
        let doc = doc
            .remove_view(3)
            .unwrap()
            .set_formula("total", None)
            .unwrap();
        assert_eq!(doc.config.views.len(), 3);
        assert!(!doc.config.formulas.contains_key("total"));
        assert!(BaseDocument::new_table().remove_view(0).is_err());
    }

    #[test]
    fn invalid_known_fields_are_errors_not_silent_defaults() {
        for source in [
            "views: table",
            "views: [{type: table, limit: -1}]",
            "views: [{type: table, order: 1}]",
            "views: [{type: table, sort: [{property: x, direction: SIDEWAYS}]}]",
            "filters: {and: [], or: []}\nviews: []",
            "formulas: {x: 123}\nviews: []",
            "views: [{type: table, groupOrder: false}]",
            "views: []\nviews: []",
        ] {
            assert!(BaseDocument::parse(source).is_err(), "{source}");
        }
    }
}
