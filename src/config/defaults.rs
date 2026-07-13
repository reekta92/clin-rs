use super::types::Background;

pub fn default_preview_enabled() -> bool {
    true
}
pub fn default_preview_width_ratio() -> f32 {
    0.43
}
pub fn default_calendar_height() -> u16 {
    9
}
pub fn default_label_max() -> usize {
    20
}
pub fn default_node_size() -> f64 {
    2.0
}
pub fn default_edge_thickness() -> u16 {
    1
}
pub fn default_true() -> bool {
    true
}
pub fn default_code_theme() -> String {
    "base16-ocean.dark".to_string()
}
pub fn default_link_url_max() -> usize {
    80
}
pub fn default_ideal_distance() -> f64 {
    80.0
}
pub fn default_zoom_factor() -> f64 {
    1.15
}
pub fn default_drag_sensitivity() -> f64 {
    1.0
}
pub fn default_minimap_width() -> u16 {
    24
}
pub fn default_minimap_height() -> u16 {
    12
}
pub fn default_label_offset() -> f64 {
    4.0
}
pub fn default_grid_divisions() -> usize {
    10
}
pub fn default_search_max_results() -> usize {
    20
}
pub fn default_search_max_visible() -> usize {
    10
}
pub fn default_graph_background() -> Background {
    Background::Solid
}
pub fn default_theme() -> String {
    "default".to_string()
}
pub fn default_date_format() -> String {
    "%Y-%m-%d %H:%M".to_string()
}
pub fn default_word_goal() -> usize {
    500
}
pub fn default_note_goal() -> usize {
    3
}
pub fn default_sections() -> Vec<super::types::NotesSection> {
    vec![
        super::types::NotesSection::Calendar,
        super::types::NotesSection::Goals,
    ]
}
