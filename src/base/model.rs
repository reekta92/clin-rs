use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FilterNode {
    Map(FilterNodeMap),
    Expr(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FilterNodeMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub and: Option<Vec<FilterNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub or: Option<Vec<FilterNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not: Option<Vec<FilterNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupBy {
    pub property: String,
    pub direction: SortDirection,
}
impl Default for GroupBy {
    fn default() -> Self {
        Self {
            property: String::new(),
            direction: SortDirection::Asc,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewType {
    #[default]
    Table,
    List,
    Cards,
    Map,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BaseView {
    pub r#type: ViewType,
    pub name: Option<String>,
    pub limit: Option<usize>,
    pub filters: Option<FilterNode>,
    pub group_by: Option<GroupBy>,
    pub order: Vec<String>,
    pub summaries: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PropertyDisplay {
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BaseFile {
    pub filters: Option<FilterNode>,
    pub formulas: BTreeMap<String, String>,
    pub properties: BTreeMap<String, PropertyDisplay>,
    pub summaries: BTreeMap<String, String>,
    pub views: Vec<BaseView>,
}
