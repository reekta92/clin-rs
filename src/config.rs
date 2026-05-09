use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[path = "graf/themes.rs"]
pub mod themes;

// ── Enums ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
            _ => Err(format!("Unknown theme: {}", s)),
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
            _ => Err(format!("Unknown background: {}", s)),
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeColorMode {
    #[default]
    Tag,
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
            _ => Err(format!("Unknown node_color_mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EdgeColorMode {
    #[default]
    Source,
    Target,
    Uniform,
}

impl FromStr for EdgeColorMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "source" => Ok(EdgeColorMode::Source),
            "target" => Ok(EdgeColorMode::Target),
            "uniform" => Ok(EdgeColorMode::Uniform),
            _ => Err(format!("Unknown edge_color_mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
            _ => Err(format!("Unknown label_mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
            _ => Err(format!("Unknown node_size_mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
            _ => Err(format!("Unknown canvas_marker: {}", s)),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
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
            _ => Err(format!("Unknown node_shape: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
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
            _ => Err(format!("Unknown border_style: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
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
            _ => Err(format!("Unknown legend position: {}", s)),
        }
    }
}

// ── Color helpers ────────────────────────────────────────────────────────────

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
            .ok_or_else(|| serde::de::Error::custom(format!("invalid hex color: {}", s))),
    }
}

// ── ColorOverrides ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
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
                format!("#{:02x}{:02x}{:02x}", r, g, b)
            } else {
                format!("{:?}", c)
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

// ── Graf sub-config structs ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConfig {
    #[serde(default)]
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
    #[serde(default = "default_true")]
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
    #[serde(default = "default_damping")]
    pub damping: f32,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    #[serde(default = "default_gravity")]
    pub gravity: f64,
    #[serde(default = "default_true")]
    pub cooling: bool,
    #[serde(default = "default_true")]
    pub prevent_overlapping: bool,
    #[serde(default = "default_timestep")]
    pub timestep: f64,
    #[serde(default = "default_thread_sleep_ms")]
    pub thread_sleep_ms: u64,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            ideal_distance: default_ideal_distance(),
            damping: default_damping(),
            max_iterations: default_max_iterations(),
            gravity: default_gravity(),
            cooling: default_true(),
            prevent_overlapping: default_true(),
            timestep: default_timestep(),
            thread_sleep_ms: default_thread_sleep_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionConfig {
    #[serde(default = "default_double_click")]
    pub double_click_ms: u64,
    #[serde(default = "default_zoom_factor")]
    pub zoom_factor: f64,
    #[serde(default = "default_drag_sensitivity")]
    pub drag_sensitivity: f64,
    #[serde(default = "default_auto_fit_padding")]
    pub auto_fit_padding: f64,
    #[serde(default = "default_drag_scale")]
    pub drag_scale: f64,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self {
            double_click_ms: default_double_click(),
            zoom_factor: default_zoom_factor(),
            drag_sensitivity: default_drag_sensitivity(),
            auto_fit_padding: default_auto_fit_padding(),
            drag_scale: default_drag_scale(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilterConfig {
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub min_links: usize,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            exclude_tags: Vec::new(),
            min_links: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendConfig {
    #[serde(default)]
    pub position: LegendPosition,
    #[serde(default = "default_max_legend_items")]
    pub max_items: usize,
}

impl Default for LegendConfig {
    fn default() -> Self {
        Self {
            position: LegendPosition::BottomRight,
            max_items: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_search_max_results")]
    pub max_results: usize,
    #[serde(default = "default_search_max_visible")]
    pub max_visible: usize,
    #[serde(default = "default_search_popup_width")]
    pub popup_width: u16,
    #[serde(default = "default_search_popup_y")]
    pub popup_y: u16,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: default_search_max_results(),
            max_visible: default_search_max_visible(),
            popup_width: default_search_popup_width(),
            popup_y: default_search_popup_y(),
        }
    }
}

// ── ThemeConfig (from old BootstrapConfig) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    #[serde(default = "default_theme", serialize_with = "serialize_theme", deserialize_with = "deserialize_theme")]
    pub theme: Theme,
    #[serde(default, serialize_with = "serialize_background", deserialize_with = "deserialize_background")]
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

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Default,
            background: Background::Transparent,
            accent: None,
            heading: None,
            success: None,
            destructive: None,
            muted: None,
            text: None,
            border: None,
            tag: None,
            folder: None,
            background_color: None,
        }
    }
}

// ── ClinConfig (unified) ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClinConfig {
    // Bootstrap-origin fields
    pub storage_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_storage_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_editor: Option<String>,
    #[serde(default)]
    pub external_editor_enabled: bool,
    #[serde(default = "default_preview_enabled")]
    pub preview_enabled: bool,
    #[serde(default)]
    pub editor_preview_enabled: bool,
    #[serde(default)]
    pub theme: ThemeConfig,

    // Graf-origin sub-structs
    #[serde(default)]
    pub visual: VisualConfig,
    #[serde(default)]
    pub physics: PhysicsConfig,
    #[serde(default)]
    pub interaction: InteractionConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub legend: LegendConfig,
    #[serde(default)]
    pub search: SearchConfig,
}

// ── Default helper functions ──────────────────────────────────────────────────

fn default_preview_enabled() -> bool { true }
fn default_label_max() -> usize { 20 }
fn default_node_size() -> f64 { 2.0 }
fn default_edge_thickness() -> u16 { 1 }
fn default_true() -> bool { true }
fn default_ideal_distance() -> f64 { 80.0 }
fn default_damping() -> f32 { 0.95 }
fn default_max_iterations() -> usize { 800 }
fn default_gravity() -> f64 { 0.01 }
fn default_double_click() -> u64 { 300 }
fn default_zoom_factor() -> f64 { 1.15 }
fn default_drag_sensitivity() -> f64 { 1.0 }
fn default_border_title() -> String { "graf".to_string() }
fn default_max_legend_items() -> usize { 10 }
fn default_minimap_width() -> u16 { 24 }
fn default_minimap_height() -> u16 { 12 }
fn default_label_offset() -> f64 { 4.0 }
fn default_grid_divisions() -> usize { 10 }
fn default_timestep() -> f64 { 0.016 }
fn default_thread_sleep_ms() -> u64 { 16 }
fn default_auto_fit_padding() -> f64 { 1.4 }
fn default_drag_scale() -> f64 { 200.0 }
fn default_search_max_results() -> usize { 20 }
fn default_search_max_visible() -> usize { 10 }
fn default_search_popup_width() -> u16 { 50 }
fn default_search_popup_y() -> u16 { 3 }

fn default_theme() -> Theme { Theme::Default }

fn serialize_theme<S>(theme: &Theme, serializer: S) -> Result<S::Ok, S::Error>
where S: Serializer {
    serializer.serialize_str(&theme.to_string())
}

fn deserialize_theme<'de, D>(deserializer: D) -> Result<Theme, D::Error>
where D: Deserializer<'de> {
    let s = String::deserialize(deserializer)?;
    s.parse::<Theme>().map_err(serde::de::Error::custom)
}

fn serialize_background<S>(bg: &Background, serializer: S) -> Result<S::Ok, S::Error>
where S: Serializer {
    serializer.serialize_str(&bg.to_string())
}

fn deserialize_background<'de, D>(deserializer: D) -> Result<Background, D::Error>
where D: Deserializer<'de> {
    let s = String::deserialize(deserializer)?;
    s.parse::<Background>().map_err(serde::de::Error::custom)
}

// ── ThemeColors ──────────────────────────────────────────────────────────────

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

// ── ClinConfig implementation ─────────────────────────────────────────────────

impl ClinConfig {
    pub fn config_path() -> Result<PathBuf> {
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

            // Check for old graf.toml to migrate
            let proj_dirs = ProjectDirs::from("com", "clin", "clin")
                .ok_or_else(|| anyhow::anyhow!("no home dir"))?;
            let graf_path = proj_dirs.config_dir().join("graf.toml");
            let mut config = Self::default();

            if graf_path.exists() {
                // Migrate: load graf.toml, merge, rename old file
                if let Ok(content) = fs::read_to_string(&graf_path) {
                    if let Ok(graf_config) = toml::from_str::<GrafConfigOnly>(&content) {
                        config.visual = graf_config.visual;
                        config.physics = graf_config.physics;
                        config.interaction = graf_config.interaction;
                        config.display = graf_config.display;
                        config.filter = graf_config.filter;
                        config.legend = graf_config.legend;
                        config.search = graf_config.search;
                    }
                }
                let _ = fs::rename(&graf_path, graf_path.with_extension("toml.migrated"));
            }

            let content = toml::to_string_pretty(&config)
                .context("failed to serialize default config")?;
            let mut file = fs::File::create(&config_path)
                .context("failed to create config file")?;
            file.write_all(content.as_bytes())
                .context("failed to write config file")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600));
            }

            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)
            .context("failed to read config")?;
        let config: ClinConfig = toml::from_str(&content)
            .context("failed to parse config")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }
        let content = toml::to_string_pretty(self)
            .context("failed to serialize config")?;
        let mut file = fs::File::create(&config_path)
            .context("failed to create config file")?;
        file.write_all(content.as_bytes())
            .context("failed to write config file")?;
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
        let mut colors = themes::theme_colors(&self.theme.theme, self.visual.graph_background.clone());

        if let Some(ref c) = self.visual.colors.node_color {
            colors.node_colors = vec![*c];
        }
        if let Some(c) = self.visual.colors.edge_color {
            colors.edge_color = c;
        }
        if let Some(c) = self.visual.colors.label_color {
            colors.label_color = c;
        }
        if let Some(c) = self.visual.colors.selection_ring_color {
            colors.selected_indicator_color = c;
        }
        if let Some(c) = self.visual.colors.border_color {
            colors.border_color = c;
            colors.legend_border_color = c;
            colors.minimap_border_color = c;
        }
        if let Some(c) = self.visual.colors.title_color {
            colors.title_color = c;
        }
        if let Some(c) = self.visual.colors.grid_color {
            colors.grid_color = c;
        }
        if let Some(c) = self.visual.colors.legend_text_color {
            colors.legend_text_color = c;
        }
        if let Some(c) = self.visual.colors.status_bar_color {
            colors.status_bar_color = c;
        }
        if let Some(c) = self.visual.colors.background_color {
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
        let fmt = fmt.replace("{date}", &chrono::Local::now().format("%Y-%m-%d").to_string());
        let fmt = fmt.replace("{time}", &chrono::Local::now().format("%H:%M:%S").to_string());
        let fmt = fmt.replace("{size}", &format!("{:.0}%", viewport_size_pct.unwrap_or(0.0).clamp(0.0, 100.0)));
        fmt.replace("{ratio}", &format!("{:.1}x", viewport_ratio.unwrap_or(1.0)))
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        if self.visual.label_max_length < 1 || self.visual.label_max_length > 60 {
            errs.push(format!(
                "visual.label_max_length must be 1-60, got {}",
                self.visual.label_max_length
            ));
        }
        if self.visual.node_size < 1.0 || self.visual.node_size > 5.0 {
            errs.push(format!(
                "visual.node_size must be 1.0-5.0, got {}",
                self.visual.node_size
            ));
        }
        if self.visual.edge_thickness < 1 || self.visual.edge_thickness > 3 {
            errs.push(format!(
                "visual.edge_thickness must be 1-3, got {}",
                self.visual.edge_thickness
            ));
        }
        if self.interaction.zoom_factor <= 0.0 {
            errs.push(format!(
                "interaction.zoom_factor must be > 0, got {}",
                self.interaction.zoom_factor
            ));
        }
        if self.visual.show_legend && self.visual.show_minimap {
            let same_corner = matches!(
                (&self.legend.position, &self.visual.minimap_position),
                (LegendPosition::TopRight, LegendPosition::TopRight)
                    | (LegendPosition::TopLeft, LegendPosition::TopLeft)
                    | (LegendPosition::BottomRight, LegendPosition::BottomRight)
                    | (LegendPosition::BottomLeft, LegendPosition::BottomLeft)
            );
            if same_corner {
                errs.push(
                    "legend.position and visual.minimap_position are in the same corner — they will overlap".to_string()
                );
            }
        }
        errs
    }
}

// ── Helper struct for graf.toml migration ─────────────────────────────────────

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
    legend: LegendConfig,
    #[serde(default)]
    search: SearchConfig,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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
}
