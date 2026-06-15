use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::OnceLock;

static CONFIG_PATH_OVERRIDE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Set once at startup from the parsed `--config` value. Panics-free: later calls are no-ops.
pub fn set_config_path_override(path: PathBuf) {
    let _ = CONFIG_PATH_OVERRIDE.set(Some(path));
}

#[path = "graf/themes.rs"]
pub mod themes;
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListDensity {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotesLayout {
    Tree,
    #[default]
    Grid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Default,
    TokyoNight,
    CatppuccinMocha,
    Onedark,
    Gruvbox,
    Dracula,
    Nord,
    RosePine,
    Everforest,
    Kanagawa,
    Solarized,
}

impl FromStr for Theme {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Theme::Default),
            "tokyo_night" | "tokyonight" => Ok(Theme::TokyoNight),
            "catppuccin_mocha" | "catppuccinmocha" => Ok(Theme::CatppuccinMocha),
            "onedark" => Ok(Theme::Onedark),
            "gruvbox" => Ok(Theme::Gruvbox),
            "dracula" => Ok(Theme::Dracula),
            "nord" => Ok(Theme::Nord),
            "rose_pine" | "rosepine" => Ok(Theme::RosePine),
            "everforest" => Ok(Theme::Everforest),
            "kanagawa" => Ok(Theme::Kanagawa),
            "solarized" | "solarized_dark" | "solarizeddark" => Ok(Theme::Solarized),
            _ => Err(format!("Unknown theme: {s}")),
        }
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Default => write!(f, "default"),
            Theme::TokyoNight => write!(f, "tokyo_night"),
            Theme::CatppuccinMocha => write!(f, "catppuccin_mocha"),
            Theme::Onedark => write!(f, "onedark"),
            Theme::Gruvbox => write!(f, "gruvbox"),
            Theme::Dracula => write!(f, "dracula"),
            Theme::Nord => write!(f, "nord"),
            Theme::RosePine => write!(f, "rose_pine"),
            Theme::Everforest => write!(f, "everforest"),
            Theme::Kanagawa => write!(f, "kanagawa"),
            Theme::Solarized => write!(f, "solarized"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Background {
    #[default]
    Transparent,
    Solid,
}

impl FromStr for Background {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "transparent" => Ok(Background::Transparent),
            "solid" => Ok(Background::Solid),
            _ => Err(format!("Unknown background: {s}")),
        }
    }
}

impl std::fmt::Display for Background {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Background::Transparent => write!(f, "transparent"),
            Background::Solid => write!(f, "solid"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeColorMode {
    Tag,
    #[default]
    Folder,
    LinkCount,
    Uniform,
}

impl FromStr for NodeColorMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tag" => Ok(NodeColorMode::Tag),
            "folder" => Ok(NodeColorMode::Folder),
            "link_count" | "linkcount" => Ok(NodeColorMode::LinkCount),
            "uniform" => Ok(NodeColorMode::Uniform),
            _ => Err(format!("Unknown node_color_mode: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeColorMode {
    Source,
    Target,
    #[default]
    Uniform,
}

impl FromStr for EdgeColorMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "source" => Ok(EdgeColorMode::Source),
            "target" => Ok(EdgeColorMode::Target),
            "uniform" => Ok(EdgeColorMode::Uniform),
            _ => Err(format!("Unknown edge_color_mode: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LabelMode {
    #[default]
    Selected,
    Neighbors,
    All,
    None,
}

impl FromStr for LabelMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "selected" => Ok(LabelMode::Selected),
            "neighbors" => Ok(LabelMode::Neighbors),
            "all" => Ok(LabelMode::All),
            "none" => Ok(LabelMode::None),
            _ => Err(format!("Unknown label_mode: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeSizeMode {
    #[default]
    Fixed,
    LinkCount,
}

impl FromStr for NodeSizeMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fixed" => Ok(NodeSizeMode::Fixed),
            "link_count" | "linkcount" => Ok(NodeSizeMode::LinkCount),
            _ => Err(format!("Unknown node_size_mode: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CanvasMarker {
    #[default]
    Braille,
    HalfBlock,
    Dot,
}

impl FromStr for CanvasMarker {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "braille" => Ok(CanvasMarker::Braille),
            "half_block" | "halfblock" => Ok(CanvasMarker::HalfBlock),
            "dot" => Ok(CanvasMarker::Dot),
            _ => Err(format!("Unknown canvas_marker: {s}")),
        }
    }
}

impl From<CanvasMarker> for ratatui::symbols::Marker {
    fn from(m: CanvasMarker) -> Self {
        match m {
            CanvasMarker::Braille => ratatui::symbols::Marker::Braille,
            CanvasMarker::HalfBlock => ratatui::symbols::Marker::HalfBlock,
            CanvasMarker::Dot => ratatui::symbols::Marker::Dot,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeShape {
    #[default]
    Circle,
    Square,
    Diamond,
}

impl FromStr for NodeShape {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "circle" => Ok(NodeShape::Circle),
            "square" => Ok(NodeShape::Square),
            "diamond" => Ok(NodeShape::Diamond),
            _ => Err(format!("Unknown node_shape: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    Plain,
    #[default]
    Rounded,
    Double,
    None,
}

impl BorderStyle {
    pub fn to_border_type(&self) -> ratatui::widgets::BorderType {
        match self {
            BorderStyle::Plain => ratatui::widgets::BorderType::Plain,
            BorderStyle::Rounded => ratatui::widgets::BorderType::Rounded,
            BorderStyle::Double => ratatui::widgets::BorderType::Double,
            BorderStyle::None => ratatui::widgets::BorderType::Plain,
        }
    }
}

impl FromStr for BorderStyle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "plain" => Ok(BorderStyle::Plain),
            "rounded" => Ok(BorderStyle::Rounded),
            "double" => Ok(BorderStyle::Double),
            "none" => Ok(BorderStyle::None),
            _ => Err(format!("Unknown border_style: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegendPosition {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl FromStr for LegendPosition {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "top_right" | "topright" => Ok(LegendPosition::TopRight),
            "top_left" | "topleft" => Ok(LegendPosition::TopLeft),
            "bottom_right" | "bottomright" => Ok(LegendPosition::BottomRight),
            "bottom_left" | "bottomleft" => Ok(LegendPosition::BottomLeft),
            _ => Err(format!("Unknown legend position: {s}")),
        }
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#')?;
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    } else {
        None
    }
}

fn deserialize_optional_color<'de, D>(deserializer: D) -> Result<Option<Color>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => parse_hex_color(&s)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid hex color: {s}"))),
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColorOverrides {
    pub node_color: Option<Color>,
    pub edge_color: Option<Color>,
    pub label_color: Option<Color>,
    pub selection_ring_color: Option<Color>,
    pub border_color: Option<Color>,
    pub title_color: Option<Color>,
    pub grid_color: Option<Color>,
    pub legend_text_color: Option<Color>,
    pub status_bar_color: Option<Color>,
    pub background_color: Option<Color>,
}

impl serde::Serialize for ColorOverrides {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ColorOverrides", 10)?;
        fn fmt_color(c: &Color) -> String {
            if let Color::Rgb(r, g, b) = c {
                format!("#{r:02x}{g:02x}{b:02x}")
            } else {
                format!("{c:?}")
            }
        }
        if let Some(ref v) = self.node_color {
            s.serialize_field("node_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.edge_color {
            s.serialize_field("edge_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.label_color {
            s.serialize_field("label_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.selection_ring_color {
            s.serialize_field("selection_ring_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.border_color {
            s.serialize_field("border_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.title_color {
            s.serialize_field("title_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.grid_color {
            s.serialize_field("grid_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.legend_text_color {
            s.serialize_field("legend_text_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.status_bar_color {
            s.serialize_field("status_bar_color", &fmt_color(v))?;
        }
        if let Some(ref v) = self.background_color {
            s.serialize_field("background_color", &fmt_color(v))?;
        }
        s.end()
    }
}

impl<'de> serde::Deserialize<'de> for ColorOverrides {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct ColorOverridesRaw {
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            node_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            edge_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            label_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            selection_ring_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            border_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            title_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            grid_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            legend_text_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            status_bar_color: Option<Color>,
            #[serde(default, deserialize_with = "deserialize_optional_color")]
            background_color: Option<Color>,
        }
        let raw = ColorOverridesRaw::deserialize(deserializer)?;
        Ok(ColorOverrides {
            node_color: raw.node_color,
            edge_color: raw.edge_color,
            label_color: raw.label_color,
            selection_ring_color: raw.selection_ring_color,
            border_color: raw.border_color,
            title_color: raw.title_color,
            grid_color: raw.grid_color,
            legend_text_color: raw.legend_text_color,
            status_bar_color: raw.status_bar_color,
            background_color: raw.background_color,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualConfig {
    #[serde(default = "default_graph_background")]
    pub graph_background: Background,
    #[serde(default)]
    pub node_color_mode: NodeColorMode,
    #[serde(default)]
    pub edge_color_mode: EdgeColorMode,
    #[serde(default)]
    pub label_mode: LabelMode,
    #[serde(default = "default_label_max")]
    pub label_max_length: usize,
    #[serde(default = "default_node_size")]
    pub node_size: f64,
    #[serde(default)]
    pub node_size_mode: NodeSizeMode,
    #[serde(default = "default_edge_thickness")]
    pub edge_thickness: u16,
    #[serde(default = "default_true")]
    pub show_legend: bool,
    #[serde(default)]
    pub show_grid: bool,
    #[serde(default)]
    pub show_minimap: bool,
    #[serde(default)]
    pub minimap_position: LegendPosition,
    #[serde(default = "default_minimap_width")]
    pub minimap_width: u16,
    #[serde(default = "default_minimap_height")]
    pub minimap_height: u16,
    #[serde(default)]
    pub canvas_marker: CanvasMarker,
    #[serde(default)]
    pub node_shape: NodeShape,
    #[serde(default = "default_label_offset")]
    pub label_offset: f64,
    #[serde(default = "default_grid_divisions")]
    pub grid_divisions: usize,
    #[serde(default)]
    pub colors: ColorOverrides,
}

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            graph_background: Background::Solid,
            node_color_mode: NodeColorMode::Folder,
            edge_color_mode: EdgeColorMode::Uniform,
            label_mode: LabelMode::default(),
            label_max_length: default_label_max(),
            node_size: default_node_size(),
            node_size_mode: NodeSizeMode::default(),
            edge_thickness: default_edge_thickness(),
            show_legend: default_true(),
            show_grid: false,
            show_minimap: false,
            minimap_position: LegendPosition::TopRight,
            minimap_width: default_minimap_width(),
            minimap_height: default_minimap_height(),
            canvas_marker: CanvasMarker::Braille,
            node_shape: NodeShape::default(),
            label_offset: default_label_offset(),
            grid_divisions: default_grid_divisions(),
            colors: ColorOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhysicsConfig {
    #[serde(default = "default_ideal_distance")]
    pub ideal_distance: f64,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            ideal_distance: default_ideal_distance(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractionConfig {
    #[serde(default = "default_zoom_factor")]
    pub zoom_factor: f64,
    #[serde(default = "default_drag_sensitivity")]
    pub drag_sensitivity: f64,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self {
            zoom_factor: default_zoom_factor(),
            drag_sensitivity: default_drag_sensitivity(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayConfig {
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    #[serde(default)]
    pub status_format: Option<String>,
    #[serde(default)]
    pub border_style: BorderStyle,
    #[serde(default = "default_border_title")]
    pub border_title: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_status_bar: default_true(),
            status_format: None,
            border_style: BorderStyle::default(),
            border_title: default_border_title(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreviewPosition {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FilterConfig {
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub min_links: usize,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchConfig {
    #[serde(default = "default_search_max_results")]
    pub max_results: usize,
    #[serde(default = "default_search_max_visible")]
    pub max_visible: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: default_search_max_results(),
            max_visible: default_search_max_visible(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ThemeConfig {
    #[serde(
        default = "default_theme",
        serialize_with = "serialize_theme",
        deserialize_with = "deserialize_theme"
    )]
    pub theme: Theme,
    #[serde(
        default,
        serialize_with = "serialize_background",
        deserialize_with = "deserialize_background"
    )]
    pub background: Background,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BackupConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub backup_on_save: bool,
    #[serde(default = "default_true")]
    pub backup_on_quit: bool,
    #[serde(default)]
    pub auto_push: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_name: Option<String>,
    #[serde(default)]
    pub commit_message_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_backup_interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ListConfig {
    #[serde(default = "default_preview_enabled")]
    pub preview_enabled: bool,
    #[serde(default)]
    pub preview_position: PreviewPosition,
    #[serde(default)]
    pub preview_encryption: bool,
    #[serde(default = "default_true")]
    pub show_date_in_list: bool,
    #[serde(default)]
    pub show_file_size: bool,
    #[serde(default = "default_date_format")]
    pub date_format: String,
    #[serde(default)]
    pub density: ListDensity,
    #[serde(default)]
    pub default_view: NotesLayout,
    #[serde(default)]
    pub default_sort_field: Option<crate::app::SortField>,
    #[serde(default)]
    pub default_sort_order: Option<crate::app::SortOrder>,
    #[serde(default)]
    pub pinned_on_top: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EditorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_command: Option<String>,
    #[serde(default)]
    pub external_enabled: bool,
    #[serde(default)]
    pub preview_enabled: bool,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct GrafConfig {
    #[serde(default)]
    pub visual: VisualConfig,
    #[serde(default)]
    pub physics: PhysicsConfig,
    #[serde(default)]
    pub interaction: InteractionConfig,
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub preview_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ClinConfig {
    pub storage_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_storage_path: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub mouse_enabled: bool,
    #[serde(default)]
    pub default_folder: Option<String>,
    #[serde(default = "default_true")]
    pub confirm_on_delete: bool,
    #[serde(default)]
    pub confirm_on_quit: bool,

    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub backup: BackupConfig,

    #[serde(default)]
    pub list: ListConfig,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub graf: GrafConfig,
}

fn default_preview_enabled() -> bool {
    true
}
fn default_border_title() -> String {
    "graf".to_string()
}
fn default_label_max() -> usize {
    20
}
fn default_node_size() -> f64 {
    2.0
}
fn default_edge_thickness() -> u16 {
    1
}
fn default_true() -> bool {
    true
}
fn default_ideal_distance() -> f64 {
    80.0
}
fn default_zoom_factor() -> f64 {
    1.15
}
fn default_drag_sensitivity() -> f64 {
    1.0
}
fn default_minimap_width() -> u16 {
    24
}
fn default_minimap_height() -> u16 {
    12
}
fn default_label_offset() -> f64 {
    4.0
}
fn default_grid_divisions() -> usize {
    10
}
fn default_search_max_results() -> usize {
    20
}
fn default_search_max_visible() -> usize {
    10
}

fn default_graph_background() -> Background {
    Background::Solid
}

fn default_theme() -> Theme {
    Theme::Default
}

fn default_date_format() -> String {
    "%Y-%m-%d".to_string()
}

fn serialize_theme<S>(theme: &Theme, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&theme.to_string())
}

fn deserialize_theme<'de, D>(deserializer: D) -> Result<Theme, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<Theme>().map_err(serde::de::Error::custom)
}

fn serialize_background<S>(bg: &Background, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&bg.to_string())
}

fn deserialize_background<'de, D>(deserializer: D) -> Result<Background, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<Background>().map_err(serde::de::Error::custom)
}

pub struct ThemeColors {
    pub node_colors: Vec<Color>,
    pub edge_color: Color,
    pub border_color: Color,
    pub title_color: Color,
    pub label_color: Color,
    pub legend_text_color: Color,
    pub legend_border_color: Color,
    pub selected_indicator_color: Color,
    pub grid_color: Color,
    pub background_color: Option<Color>,
    pub status_bar_color: Color,
    pub minimap_border_color: Color,
    pub minimap_viewport_color: Color,
    pub minimap_bg_color: Option<Color>,
}

impl ClinConfig {
    pub fn config_path() -> Result<PathBuf> {
        if let Some(p) = CONFIG_PATH_OVERRIDE.get().and_then(|opt| opt.as_ref()) {
            return Ok(p.clone());
        }
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine config directory")?;
        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    pub fn default_storage_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine data directory")?;
        Ok(proj_dirs.data_local_dir().to_path_buf())
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent).context("failed to create config directory")?;
            }

            let proj_dirs = ProjectDirs::from("com", "clin", "clin")
                .ok_or_else(|| anyhow::anyhow!("no home dir"))?;
            let graf_path = proj_dirs.config_dir().join("graf.toml");
            let mut config = Self::default();
            config.graf = GrafConfig::default();

            if graf_path.exists() {
                if let Ok(content) = fs::read_to_string(&graf_path)
                    && let Ok(graf_config) = toml::from_str::<GrafConfigOnly>(&content)
                {
                    config.graf.visual = graf_config.visual;
                    config.graf.physics = graf_config.physics;
                    config.graf.interaction = graf_config.interaction;
                    config.display = graf_config.display;
                    config.graf.filter = graf_config.filter;
                    config.graf.search = graf_config.search;
                }
                let _ = fs::rename(&graf_path, graf_path.with_extension("toml.migrated"));
            }

            let content = r#"# Clin Configuration File

# Custom path for notes storage (e.g., "/home/user/vault").
# If not set, defaults to the standard data directory for your OS.
# storage_path = "/path/to/your/notes"

# Enable mouse support (clicking, scrolling, panning).
mouse_enabled = true

# Default folder for new notes (relative to vault root).
# default_folder = "inbox"

# Confirm before moving a note or folder to the trash.
confirm_on_delete = true

# Confirm before quitting.
confirm_on_quit = false

# ── List View ─────────────────────────────────────────────────────────────────

[list]
# Show the preview pane in the notes list by default.
preview_enabled = true

# Preview position ("left", "right", "top", "bottom").
preview_position = "right"

# Hide previews of encrypted notes.
preview_encryption = false

# Show modification date in the notes list.
show_date_in_list = true

# Show file size in the notes list.
show_file_size = false

# Date format for the notes list (chrono format, e.g., "%Y-%m-%d").
date_format = "%Y-%m-%d"

# Density of the notes list ("comfortable" or "compact").
density = "comfortable"

# Default view mode for the notes list ("grid" or "tree").
default_view = "grid"

# Default sorting field for the notes list ("title" or "modified").
default_sort_field = "title"

# Default sorting order ("ascending" or "descending").
default_sort_order = "ascending"

# Keep pinned notes at the top of the list.
pinned_on_top = true

# ── Editor ────────────────────────────────────────────────────────────────────

[editor]
# External editor command (e.g., "nvim", "code", "nano").
# external_command = "nvim"

# Enable external editor mode by default.
external_enabled = false

# Show the markdown preview in the editor view by default.
preview_enabled = false

# Show line numbers in the editor.
show_line_numbers = true
# ── Backup ────────────────────────────────────────────────────────────────────

[backup]
# Enable auto-backups via git.
enabled = true
# Perform a backup commit whenever a note is saved.
backup_on_save = true
# Perform a backup commit when the app exits.
backup_on_quit = true
# Interval in minutes for automatic background backups.
# auto_backup_interval = 30

# ── Theme ─────────────────────────────────────────────────────────────────────

[theme]
# Theme to use ("default", "tokyo_night", "catppuccin_mocha", "onedark", "gruvbox", etc.)
theme = "default"
# Background style ("transparent" or "solid")
background = "transparent"


# ── Graph View (Graf) ─────────────────────────────────────────────────────────

[graf.visual]
# Graph background style ("solid", "transparent")
graph_background = "solid"
# Node color mode ("folder", "tag", "uniform")
node_color_mode = "folder"
# Edge color mode ("uniform", "gradient")
edge_color_mode = "uniform"
# Label display mode ("selected", "neighbors", "all", "none")
label_mode = "selected"
# Show legend in graph view
show_legend = true
# Show minimap in graph view
show_minimap = false

[graf.physics]
# Ideal distance between nodes
ideal_distance = 80.0

[graf.interaction]
# Zoom sensitivity factor
zoom_factor = 1.15
# Drag sensitivity factor
drag_sensitivity = 1.0
"#;
            let mut file =
                fs::File::create(&config_path).context("failed to create config file")?;
            file.write_all(content.as_bytes())
                .context("failed to write config file")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600));
            }

            return Ok(config);
        }

        let content = fs::read_to_string(&config_path).context("failed to read config")?;

        // Phase E: Migration (visual.notes_layout -> default_view, and flat graf/list/editor keys to nested namespaces)
        let mut value: toml::Value = toml::from_str(&content).context("failed to parse config for migration")?;
        let mut changed = false;

        if let Some(visual) = value.get_mut("visual").and_then(|v| v.as_table_mut()) {
            if let Some(notes_layout) = visual.remove("notes_layout") {
                if value.get("default_view").is_none() {
                    if let Some(root) = value.as_table_mut() {
                        root.insert("default_view".to_string(), notes_layout);
                        changed = true;
                    }
                }
            }
        }

        let mut editor_table = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(v) = root.remove("external_editor") {
                editor_table.insert("external_command".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("external_editor_enabled") {
                editor_table.insert("external_enabled".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("editor_preview_enabled") {
                editor_table.insert("preview_enabled".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("show_line_numbers") {
                editor_table.insert("show_line_numbers".to_string(), v);
                changed = true;
            }
        }
        if !editor_table.is_empty() {
            if let Some(root) = value.as_table_mut() {
                if let Some(existing_editor) = root.get_mut("editor").and_then(|e| e.as_table_mut()) {
                    for (k, v) in editor_table {
                        existing_editor.insert(k, v);
                    }
                } else {
                    root.insert("editor".to_string(), toml::Value::Table(editor_table));
                }
            }
        }

        let mut list_table = toml::value::Table::new();
        let list_legacy_keys = [
            ("preview_enabled", "preview_enabled"),
            ("preview_position", "preview_position"),
            ("preview_encryption", "preview_encryption"),
            ("date_format", "date_format"),
            ("list_density", "density"),
            ("show_file_size", "show_file_size"),
            ("show_date_in_list", "show_date_in_list"),
            ("default_view", "default_view"),
            ("default_sort_field", "default_sort_field"),
            ("default_sort_order", "default_sort_order"),
            ("pinned_on_top", "pinned_on_top"),
        ];
        if let Some(root) = value.as_table_mut() {
            for (old_key, new_key) in &list_legacy_keys {
                if let Some(v) = root.remove(*old_key) {
                    list_table.insert(new_key.to_string(), v);
                    changed = true;
                }
            }
        }
        if !list_table.is_empty() {
            if let Some(root) = value.as_table_mut() {
                if let Some(existing_list) = root.get_mut("list").and_then(|l| l.as_table_mut()) {
                    for (k, v) in list_table {
                        existing_list.insert(k, v);
                    }
                } else {
                    root.insert("list".to_string(), toml::Value::Table(list_table));
                }
            }
        }

        let mut graf_addons = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(v) = root.remove("search") {
                graf_addons.insert("search".to_string(), v);
                changed = true;
            }
            if let Some(v) = root.remove("graph_preview_enabled") {
                graf_addons.insert("preview_enabled".to_string(), v);
                changed = true;
            }
        }
        
        let graf_keys = ["visual", "physics", "interaction", "filter"];
        for key in &graf_keys {
            if let Some(val) = value.as_table_mut().and_then(|t| t.remove(*key)) {
                graf_addons.insert(key.to_string(), val);
                changed = true;
            }
        }
        if !graf_addons.is_empty() {
            if let Some(root) = value.as_table_mut() {
                if let Some(existing_graf) = root.get_mut("graf").and_then(|g| g.as_table_mut()) {
                    for (k, v) in graf_addons {
                        existing_graf.insert(k, v);
                    }
                } else {
                    root.insert("graf".to_string(), toml::Value::Table(graf_addons));
                }
            }
        }
        if changed {
            let migrated_content = toml::to_string_pretty(&value).context("failed to serialize migrated config")?;
            let _ = crate::fsutil::atomic_write(&config_path, migrated_content.as_bytes());
            return Ok(toml::from_str(&migrated_content).context("failed to parse migrated config")?);
        }

        let config: ClinConfig = toml::from_str(&content).context("failed to parse config")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        crate::fsutil::atomic_write(&config_path, content.as_bytes())?;
        Ok(())
    }

    pub fn effective_storage_path(&self) -> Result<PathBuf> {
        match &self.storage_path {
            Some(path) => Ok(path.clone()),
            None => Self::default_storage_path(),
        }
    }

    pub fn set_storage_path(&mut self, path: PathBuf) {
        self.storage_path = Some(path);
    }

    pub fn reset_storage_path(&mut self) {
        self.storage_path = None;
    }

    pub fn has_custom_storage_path(&self) -> bool {
        self.storage_path.is_some()
    }

    pub fn set_previous_storage_path(&mut self, path: PathBuf) {
        self.previous_storage_path = Some(path);
    }

    pub fn clear_previous_storage_path(&mut self) {
        self.previous_storage_path = None;
    }

    pub fn theme_colors(&self) -> ThemeColors {
        let mut colors =
            themes::theme_colors(&self.theme.theme, self.graf.visual.graph_background.clone());

        if let Some(ref c) = self.graf.visual.colors.node_color {
            colors.node_colors = vec![*c];
        }
        if let Some(c) = self.graf.visual.colors.edge_color {
            colors.edge_color = c;
        }
        if let Some(c) = self.graf.visual.colors.label_color {
            colors.label_color = c;
        }
        if let Some(c) = self.graf.visual.colors.selection_ring_color {
            colors.selected_indicator_color = c;
        }
        if let Some(c) = self.graf.visual.colors.border_color {
            colors.border_color = c;
            colors.legend_border_color = c;
            colors.minimap_border_color = c;
        }
        if let Some(c) = self.graf.visual.colors.title_color {
            colors.title_color = c;
        }
        if let Some(c) = self.graf.visual.colors.grid_color {
            colors.grid_color = c;
        }
        if let Some(c) = self.graf.visual.colors.legend_text_color {
            colors.legend_text_color = c;
        }
        if let Some(c) = self.graf.visual.colors.status_bar_color {
            colors.status_bar_color = c;
        }
        if let Some(c) = self.graf.visual.colors.background_color {
            colors.background_color = Some(c);
            colors.minimap_bg_color = Some(c);
        }

        colors
    }

    pub fn expand_border_title(&self) -> String {
        let mut title = self.display.border_title.clone();
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_default();
        title = title.replace("{cwd}", &cwd);
        title
    }

    pub fn expand_status(
        &self,
        files: usize,
        links: usize,
        selected: Option<&str>,
        viewport_size_pct: Option<f64>,
        viewport_ratio: Option<f64>,
    ) -> String {
        let fmt = self
            .display
            .status_format
            .as_deref()
            .unwrap_or("Files: {files} | Links: {links} | Selected: {selected}");
        let fmt = fmt.replace("{files}", &files.to_string());
        let fmt = fmt.replace("{links}", &links.to_string());
        let fmt = fmt.replace("{selected}", selected.unwrap_or("none"));
        let fmt = fmt.replace(
            "{date}",
            &chrono::Local::now().format("%Y-%m-%d").to_string(),
        );
        let fmt = fmt.replace(
            "{time}",
            &chrono::Local::now().format("%H:%M:%S").to_string(),
        );
        let fmt = fmt.replace(
            "{size}",
            &format!("{:.0}%", viewport_size_pct.unwrap_or(0.0).clamp(0.0, 100.0)),
        );
        fmt.replace("{ratio}", &format!("{:.1}x", viewport_ratio.unwrap_or(1.0)))
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        if self.graf.visual.label_max_length < 1 || self.graf.visual.label_max_length > 60 {
            errs.push(format!(
                "graf.visual.label_max_length must be 1-60, got {}",
                self.graf.visual.label_max_length
            ));
        }
        if self.graf.visual.node_size < 1.0 || self.graf.visual.node_size > 5.0 {
            errs.push(format!(
                "graf.visual.node_size must be 1.0-5.0, got {}",
                self.graf.visual.node_size
            ));
        }
        if self.graf.visual.edge_thickness < 1 || self.graf.visual.edge_thickness > 3 {
            errs.push(format!(
                "graf.visual.edge_thickness must be 1-3, got {}",
                self.graf.visual.edge_thickness
            ));
        }
        if self.graf.interaction.zoom_factor <= 0.0 {
            errs.push(format!(
                "graf.interaction.zoom_factor must be > 0, got {}",
                self.graf.interaction.zoom_factor
            ));
        }
        if self.graf.visual.show_legend && self.graf.visual.show_minimap {
            // Legend position is now hardcoded to BottomRight. Minimap position is still configurable.
            if self.graf.visual.minimap_position == LegendPosition::BottomRight {
                errs.push("graf.visual.minimap_position cannot be bottom_right when show_legend is true".to_string());
            }
        }
        errs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GrafConfigOnly {
    #[serde(default)]
    visual: VisualConfig,
    #[serde(default)]
    physics: PhysicsConfig,
    #[serde(default)]
    interaction: InteractionConfig,
    #[serde(default)]
    display: DisplayConfig,
    #[serde(default)]
    filter: FilterConfig,
    #[serde(default)]
    search: SearchConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClinConfig::default();
        assert!(config.storage_path.is_none());
        assert!(!config.has_custom_storage_path());
    }

    #[test]
    fn test_set_storage_path() {
        let mut config = ClinConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));
        assert!(config.has_custom_storage_path());
        assert_eq!(config.storage_path, Some(PathBuf::from("/custom/path")));
    }

    #[test]
    fn test_reset_storage_path() {
        let mut config = ClinConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));
        config.reset_storage_path();
        assert!(!config.has_custom_storage_path());
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut config = ClinConfig::default();
        config.set_storage_path(PathBuf::from("/custom/path"));

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ClinConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.storage_path, parsed.storage_path);
    }

    #[test]
    fn test_serde_defaults() {
        let toml_str = r#"
[graf.visual]
# all fields omitted
"#;
        let config: ClinConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.graf.visual.show_minimap, false);
        assert_eq!(config.graf.visual.node_color_mode, NodeColorMode::Folder);
        assert_eq!(config.graf.visual.edge_color_mode, EdgeColorMode::Uniform);
        assert_eq!(config.graf.visual.graph_background, Background::Solid);
    }

    #[test]
    fn test_unknown_field_tolerance() {
        let toml_str = r#"
[graf.physics]
damping = 0.5
unknown_field = "ignore me"
"#;
        let config: ClinConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.graf.physics.ideal_distance, 80.0);
    }

    #[test]
    fn test_new_fields_roundtrip() {
        let mut config = ClinConfig::default();
        config.mouse_enabled = false;
        config.list.date_format = "%d/%m/%Y".to_string();
        config.list.density = ListDensity::Compact;
        config.list.show_file_size = true;
        config.list.show_date_in_list = false;
        config.list.default_view = NotesLayout::Tree;
        config.backup.auto_backup_interval = Some(60);

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ClinConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.mouse_enabled, false);
        assert_eq!(parsed.list.date_format, "%d/%m/%Y");
        assert_eq!(parsed.list.density, ListDensity::Compact);
        assert_eq!(parsed.list.show_file_size, true);
        assert_eq!(parsed.list.show_date_in_list, false);
        assert_eq!(parsed.list.default_view, NotesLayout::Tree);
        assert_eq!(parsed.backup.auto_backup_interval, Some(60));
    }

    #[test]
    fn test_migration_logic() {
        let toml_str = r#"
external_editor = "nvim"
preview_position = "left"

[visual]
notes_layout = "tree"

[physics]
ideal_distance = 120.0
"#;
        let mut value: toml::Value = toml::from_str(toml_str).unwrap();

        // 1. Move notes_layout to default_view
        if let Some(visual) = value.get_mut("visual").and_then(|v| v.as_table_mut()) {
            if let Some(notes_layout) = visual.remove("notes_layout") {
                if value.get("default_view").is_none() {
                    if let Some(root) = value.as_table_mut() {
                        root.insert("default_view".to_string(), notes_layout);
                    }
                }
            }
        }

        // 2. Map legacy editor keys to editor namespace
        let mut editor_table = toml::value::Table::new();
        if let Some(root) = value.as_table_mut() {
            if let Some(v) = root.remove("external_editor") {
                editor_table.insert("external_command".to_string(), v);
            }
        }
        if !editor_table.is_empty() {
            if let Some(root) = value.as_table_mut() {
                if let Some(existing_editor) = root.get_mut("editor").and_then(|e| e.as_table_mut()) {
                    for (k, v) in editor_table {
                        existing_editor.insert(k, v);
                    }
                } else {
                    root.insert("editor".to_string(), toml::Value::Table(editor_table));
                }
            }
        }

        // 3. Map legacy list keys to list namespace
        let mut list_table = toml::value::Table::new();
        let list_legacy_keys = [
            ("preview_position", "preview_position"),
            ("default_view", "default_view"),
        ];
        if let Some(root) = value.as_table_mut() {
            for (old_key, new_key) in &list_legacy_keys {
                if let Some(v) = root.remove(*old_key) {
                    list_table.insert(new_key.to_string(), v);
                }
            }
        }
        if !list_table.is_empty() {
            if let Some(root) = value.as_table_mut() {
                if let Some(existing_list) = root.get_mut("list").and_then(|l| l.as_table_mut()) {
                    for (k, v) in list_table {
                        existing_list.insert(k, v);
                    }
                } else {
                    root.insert("list".to_string(), toml::Value::Table(list_table));
                }
            }
        }

        // 5. Nest visual, physics, interaction, filter under graf
        let mut graf_addons = toml::value::Table::new();
        let graf_keys = ["visual", "physics", "interaction", "filter"];
        for key in &graf_keys {
            if let Some(val) = value.as_table_mut().and_then(|t| t.remove(*key)) {
                graf_addons.insert(key.to_string(), val);
            }
        }
        if !graf_addons.is_empty() {
            if let Some(root) = value.as_table_mut() {
                if let Some(existing_graf) = root.get_mut("graf").and_then(|g| g.as_table_mut()) {
                    for (k, v) in graf_addons {
                        existing_graf.insert(k, v);
                    }
                } else {
                    root.insert("graf".to_string(), toml::Value::Table(graf_addons));
                }
            }
        }

        let migrated_toml = toml::to_string(&value).unwrap();
        let config: ClinConfig = toml::from_str(&migrated_toml).unwrap();
        assert_eq!(config.list.default_view, NotesLayout::Tree);
        assert_eq!(config.graf.physics.ideal_distance, 120.0);
        assert_eq!(config.editor.external_command, Some("nvim".to_string()));
        assert_eq!(config.list.preview_position, PreviewPosition::Left);
    }
}
