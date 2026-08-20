use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

use super::types::*;

// ── Color Overrides (custom SerDe) ─────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColorOverrides {
    pub node_color: Option<Color>,
    pub edge_color: Option<Color>,
    pub label_color: Option<Color>,
    pub selection_ring_color: Option<Color>,
    pub border_color: Option<Color>,
    pub background_color: Option<Color>,
}

impl serde::Serialize for ColorOverrides {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ColorOverrides", 6)?;
        fn fmt_color(c: &Color) -> String {
            if let Color::Rgb(r, g, b) = c {
                format!("#{r:02x}{g:02x}{b:02x}")
            } else {
                format!("{c:?}")
            }
        }
        if let Some(v) = &self.node_color {
            s.serialize_field("node_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.edge_color {
            s.serialize_field("edge_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.label_color {
            s.serialize_field("label_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.selection_ring_color {
            s.serialize_field("selection_ring_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.border_color {
            s.serialize_field("border_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.background_color {
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
            background_color: Option<Color>,
        }
        let raw = ColorOverridesRaw::deserialize(deserializer)?;
        Ok(ColorOverrides {
            node_color: raw.node_color,
            edge_color: raw.edge_color,
            label_color: raw.label_color,
            selection_ring_color: raw.selection_ring_color,
            border_color: raw.border_color,
            background_color: raw.background_color,
        })
    }
}

// ── Config Structs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VisualConfig {
    pub graph_background: Background,
    #[serde(default)]
    pub node_color_mode: NodeColorMode,
    #[serde(default)]
    pub edge_color_mode: EdgeColorMode,
    #[serde(default)]
    pub label_mode: LabelMode,
    pub label_max_length: usize,
    pub node_size: f64,
    #[serde(default)]
    pub node_size_mode: NodeSizeMode,
    pub edge_thickness: u16,
    pub show_legend: bool,
    #[serde(default)]
    pub show_minimap: bool,
    #[serde(default)]
    pub minimap_position: LegendPosition,
    pub minimap_width: u16,
    pub minimap_height: u16,
    #[serde(default)]
    pub canvas_marker: CanvasMarker,
    #[serde(default)]
    pub node_shape: NodeShape,
    pub label_offset: f64,
    pub show_looking_glass: bool,
    pub looking_glass_width: u16,
    pub looking_glass_height: u16,
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
            label_max_length: 20,
            node_size: 2.0,
            node_size_mode: NodeSizeMode::default(),
            edge_thickness: 1,
            show_legend: true,
            show_minimap: false,
            minimap_position: LegendPosition::TopRight,
            minimap_width: 24,
            minimap_height: 12,
            canvas_marker: CanvasMarker::Braille,
            node_shape: NodeShape::default(),
            label_offset: 4.0,
            show_looking_glass: true,
            looking_glass_width: 24,
            looking_glass_height: 12,
            colors: ColorOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PhysicsConfig {
    pub ideal_distance: f64,
    #[serde(default)]
    pub tick_rate: PhysicsTickRate,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            ideal_distance: 80.0,
            tick_rate: PhysicsTickRate::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct InteractionConfig {
    pub zoom_factor: f64,
    pub drag_sensitivity: f64,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self {
            zoom_factor: 1.15,
            drag_sensitivity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
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

    pub show_status_bar: bool,

    /// Show only Nerd Font icons (no text label) on tab bars.
    #[serde(default)]
    pub tab_icons_only: bool,

    /// Icon display mode: Nerd Font, Unicode fallback, or None.
    #[serde(default)]
    pub icon_mode: IconMode,
    /// Show mouse-draggable scrollbars on scrollable regions.
    pub scrollbars: bool,
    /// When true, dragging/clicking the notes-list scrollbar pans the viewport
    /// without moving the selection; any key snaps the viewport back to the
    /// selection (first press is consumed).
    #[serde(default)]
    pub scrollbar_pan_mode: bool,
    #[serde(default)]
    pub hint_bar_style: HintBarStyle,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            background: Background::default(),
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
            show_status_bar: true,
            tab_icons_only: false,
            icon_mode: IconMode::default(),
            scrollbars: true,
            scrollbar_pan_mode: false,
            hint_bar_style: HintBarStyle::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct FilterConfig {
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub show_orphan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    pub max_results: usize,
    pub max_visible: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 20,
            max_visible: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct BackupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub backup_on_save: bool,
    #[serde(default)]
    pub backup_on_quit: bool,
    #[serde(default)]
    pub auto_push: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_backup_interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ImageConfig {
    pub enabled: bool,
    pub max_dimension: u32,
    pub cache_size: usize,
    pub preview_rows: u8,
    pub attachments_subdir: String,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_dimension: 2048,
            cache_size: 32,
            preview_rows: 8,
            attachments_subdir: "attachments".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomSmartFolder {
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub title_contains: Option<String>,
    #[serde(default)]
    pub folder_prefix: Option<String>,
    #[serde(default)]
    pub updated_within_days: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ListConfig {
    pub preview_enabled: bool,
    #[serde(default)]
    pub preview_position: PreviewPosition,
    #[serde(default)]
    pub preview_encryption: bool,
    #[serde(default)]
    pub show_file_size: bool,
    pub date_format: String,
    #[serde(default)]
    pub density: ListDensity,
    #[serde(default)]
    pub default_view: NotesLayout,
    #[serde(default)]
    pub default_sort_field: Option<crate::app::SortField>,
    #[serde(default)]
    pub default_sort_order: Option<crate::app::SortOrder>,
    pub inline_info: bool,
    #[serde(default)]
    pub pinned_on_top: bool,
    #[serde(default)]
    pub show_hidden_files: bool,
    #[serde(default)]
    pub show_all_files: bool,
    #[serde(default)]
    pub skip_dirs: Vec<String>,
    pub folders_first: bool,
    pub calendar_enabled: bool,
    #[serde(default)]
    pub smart_folders_enabled: bool,
    #[serde(default)]
    pub folder_graph_preview: bool,
    #[serde(default)]
    pub pinned_folders: Vec<String>,
    pub preview_width_ratio: f32,
    pub calendar_height: u16,
    #[serde(default)]
    pub calendar_position: CalendarPosition,
    #[serde(default)]
    pub week_start: WeekStart,
    pub sections: Vec<NotesSection>,
    #[serde(default)]
    pub default_expand_depth: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_smart_folders: Vec<CustomSmartFolder>,
}
impl Default for ListConfig {
    fn default() -> Self {
        Self {
            preview_enabled: true,
            preview_position: PreviewPosition::default(),
            preview_encryption: false,
            show_file_size: false,
            date_format: "%Y-%m-%d".to_string(),
            density: ListDensity::default(),
            default_view: NotesLayout::default(),
            default_sort_field: None,
            default_sort_order: None,
            inline_info: true,
            pinned_on_top: false,
            show_hidden_files: false,
            show_all_files: false,
            skip_dirs: Vec::new(),
            folders_first: true,
            calendar_enabled: true,
            calendar_position: CalendarPosition::default(),
            week_start: WeekStart::default(),
            smart_folders_enabled: false,
            folder_graph_preview: false,
            pinned_folders: Vec::new(),
            preview_width_ratio: 0.43,
            calendar_height: 9,
            sections: vec![NotesSection::Calendar, NotesSection::Goals],
            default_expand_depth: None,
            custom_smart_folders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EditorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_command: Option<String>,
    #[serde(default)]
    pub external_enabled: bool,
    #[serde(default)]
    pub preview_enabled: bool,
    pub show_line_numbers: bool,
    pub date_format: String,
    pub edit_mode_highlight: bool,
    pub ghost_syntax: bool,
    pub extended_markdown_features: bool,
    pub soft_wrap: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            external_command: None,
            external_enabled: false,
            preview_enabled: false,
            show_line_numbers: true,
            date_format: "%Y-%m-%d %H:%M".to_string(),
            edit_mode_highlight: true,
            ghost_syntax: true,
            extended_markdown_features: true,
            soft_wrap: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
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
    pub max_node: usize,
}

impl Default for GrafConfig {
    fn default() -> Self {
        Self {
            visual: VisualConfig::default(),
            physics: PhysicsConfig::default(),
            interaction: InteractionConfig::default(),
            filter: FilterConfig::default(),
            search: SearchConfig::default(),
            preview_enabled: false,
            max_node: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CoreConfig {
    pub storage_path: Option<PathBuf>,
    pub mouse_enabled: bool,
    #[serde(default)]
    pub default_folder: Option<String>,
    pub confirm_on_delete: bool,
    #[serde(default)]
    pub confirm_on_quit: bool,
    pub preview_wrap: bool,
    #[serde(default)]
    pub keybind_preset: KeybindPreset,
    #[serde(default)]
    pub enable_key_sequences: bool,
    #[serde(default)]
    pub preview_expand_mode: crate::config::PreviewExpandMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_command: Option<String>,
    pub syntax_highlighting: bool,
    pub code_theme: String,
    pub code_line_numbers: bool,
    pub auto_refresh: bool,
    #[serde(default)]
    pub preview_wrap_indicator: bool,
    pub link_url_max_length: usize,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            storage_path: None,
            mouse_enabled: true,
            default_folder: None,
            confirm_on_delete: true,
            confirm_on_quit: false,
            preview_wrap: true,
            keybind_preset: KeybindPreset::Default,
            enable_key_sequences: false,
            preview_expand_mode: crate::config::PreviewExpandMode::default(),
            preview_command: None,
            syntax_highlighting: true,
            code_theme: "base16-ocean.dark".to_string(),
            code_line_numbers: true,
            auto_refresh: true,
            preview_wrap_indicator: false,
            link_url_max_length: 80,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GoalsConfig {
    pub enabled: bool,
    pub word_goal: usize,
    pub note_goal: usize,
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            word_goal: 500,
            note_goal: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct StatuslineConfig {
    pub header_left: Option<String>,
    pub header_right: Option<String>,
    pub footer_left: Option<String>,
    pub footer_right: Option<String>,
    pub list: Option<StatuslineOverride>,
    pub edit: Option<StatuslineOverride>,
    pub help: Option<StatuslineOverride>,
    pub graph: Option<StatuslineOverride>,
    pub draw: Option<StatuslineOverride>,
    pub canvas: Option<StatuslineOverride>,
    pub backup: Option<StatuslineOverride>,
    pub outline: Option<StatuslineOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct StatuslineOverride {
    pub header_left: Option<String>,
    pub header_right: Option<String>,
    pub footer_left: Option<String>,
    pub footer_right: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ClinConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub backup: BackupConfig,

    #[serde(default)]
    pub list: ListConfig,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub graf: GrafConfig,
    #[serde(default)]
    pub goals: GoalsConfig,
    pub image: ImageConfig,
    #[serde(default)]
    pub statusline: StatuslineConfig,
    #[serde(skip)]
    pub accent_hint_migrated: bool,
}

/// Graph data-viz colors (node/edge/label). Distinct from
/// [`AppThemeColors`](crate::app_theme::AppThemeColors) (app chrome).
pub struct ThemeColors {
    pub node_colors: Vec<Color>,
    pub edge_color: Color,
    pub border_color: Color,
    pub label_color: Color,
    pub selected_indicator_color: Color,
    pub background_color: Option<Color>,
    pub minimap_border_color: Color,
    pub minimap_viewport_color: Color,
    pub minimap_bg_color: Option<Color>,
}

pub fn parse_hex_color(s: &str) -> Option<Color> {
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

pub fn deserialize_optional_color<'de, D>(deserializer: D) -> Result<Option<Color>, D::Error>
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

pub fn serialize_background<S>(bg: &Background, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&bg.to_string())
}

pub fn deserialize_background<'de, D>(deserializer: D) -> Result<Background, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<Background>().map_err(serde::de::Error::custom)
}
