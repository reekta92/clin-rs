use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::de::{deserialize_background, deserialize_optional_color, serialize_background};
use super::defaults::*;
use super::types::*;

// ── Color Overrides (custom SerDe) ─────────────────────────────────────────

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
        if let Some(v) = &self.title_color {
            s.serialize_field("title_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.grid_color {
            s.serialize_field("grid_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.legend_text_color {
            s.serialize_field("legend_text_color", &fmt_color(v))?;
        }
        if let Some(v) = &self.status_bar_color {
            s.serialize_field("status_bar_color", &fmt_color(v))?;
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

// ── Config Structs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
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

    #[serde(default = "default_true")]
    pub show_status_bar: bool,

    /// Show only Nerd Font icons (no text label) on tab bars.
    #[serde(default)]
    pub tab_icons_only: bool,

    /// Icon display mode: Nerd Font, Unicode fallback, or None.
    #[serde(default)]
    pub icon_mode: IconMode,
    #[serde(default)]
    pub hint_bar_style: HintBarStyle,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
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
            show_status_bar: default_true(),
            tab_icons_only: false,
            icon_mode: IconMode::default(),
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
    pub min_links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
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
    #[serde(default)]
    pub show_hidden_files: bool,
    #[serde(default)]
    pub show_all_files: bool,
    #[serde(default = "default_true")]
    pub folders_first: bool,
    #[serde(default = "default_true")]
    pub calendar_enabled: bool,
    #[serde(default = "default_preview_width_ratio")]
    pub preview_width_ratio: f32,
    #[serde(default = "default_calendar_height")]
    pub calendar_height: u16,
    #[serde(default)]
    pub calendar_position: CalendarPosition,
    #[serde(default)]
    pub week_start: WeekStart,
    #[serde(default = "default_sections")]
    pub sections: Vec<NotesSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CoreConfig {
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
    #[serde(default = "default_true")]
    pub preview_wrap: bool,
    #[serde(default)]
    pub keybind_preset: KeybindPreset,
    #[serde(default)]
    pub enable_key_sequences: bool,
    #[serde(default)]
    pub preview_expand_mode: crate::config::PreviewExpandMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_command: Option<String>,
    #[serde(default = "default_true")]
    pub syntax_highlighting: bool,
    #[serde(default = "default_code_theme")]
    pub code_theme: String,
    #[serde(default = "default_true")]
    pub code_line_numbers: bool,
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    #[serde(default)]
    pub preview_wrap_indicator: bool,
    #[serde(default = "default_link_url_max")]
    pub link_url_max_length: usize,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            storage_path: None,
            previous_storage_path: None,
            mouse_enabled: default_true(),
            default_folder: None,
            confirm_on_delete: default_true(),
            confirm_on_quit: false,
            preview_wrap: default_true(),
            keybind_preset: KeybindPreset::Default,
            enable_key_sequences: false,
            preview_expand_mode: crate::config::PreviewExpandMode::default(),
            syntax_highlighting: default_true(),
            preview_command: None,
            code_theme: default_code_theme(),
            code_line_numbers: default_true(),
            auto_refresh: default_true(),
            preview_wrap_indicator: false,
            link_url_max_length: default_link_url_max(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GoalsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_word_goal")]
    pub word_goal: usize,
    #[serde(default = "default_note_goal")]
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
}

/// Graph data-viz colors (node/edge/label). Distinct from
/// [`AppThemeColors`](crate::app_theme::AppThemeColors) (app chrome).
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
