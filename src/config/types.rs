use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListDensity {
    #[default]
    Compact,
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
    CatppuccinFrappe,
    CatppuccinMacchiato,
    RosePineMoon,
    GruvboxMaterial,
    GithubDark,
    AyuMirage,
    Synthwave,
    Material,
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
            "catppuccin_frappe" | "catppuccinfrappe" => Ok(Theme::CatppuccinFrappe),
            "catppuccin_macchiato" | "catppuccinmacchiato" => Ok(Theme::CatppuccinMacchiato),
            "rose_pine_moon" | "rosepinemoon" => Ok(Theme::RosePineMoon),
            "gruvbox_material" | "gruvboxmaterial" => Ok(Theme::GruvboxMaterial),
            "github_dark" | "githubdark" => Ok(Theme::GithubDark),
            "ayu_mirage" | "ayumirage" => Ok(Theme::AyuMirage),
            "synthwave" | "synthwave84" => Ok(Theme::Synthwave),
            "material" | "material_theme" => Ok(Theme::Material),
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
            Theme::CatppuccinFrappe => write!(f, "catppuccin_frappe"),
            Theme::CatppuccinMacchiato => write!(f, "catppuccin_macchiato"),
            Theme::RosePineMoon => write!(f, "rose_pine_moon"),
            Theme::GruvboxMaterial => write!(f, "gruvbox_material"),
            Theme::GithubDark => write!(f, "github_dark"),
            Theme::AyuMirage => write!(f, "ayu_mirage"),
            Theme::Synthwave => write!(f, "synthwave"),
            Theme::Material => write!(f, "material"),
        }
    }
}

impl Theme {
    /// The canonical display order for the theme switcher UI.
    pub const BUILTIN_NAMES: &'static [&'static str] = &[
        "default",
        "tokyo_night",
        "catppuccin_mocha",
        "catppuccin_frappe",
        "catppuccin_macchiato",
        "onedark",
        "gruvbox",
        "gruvbox_material",
        "dracula",
        "nord",
        "rose_pine",
        "rose_pine_moon",
        "everforest",
        "kanagawa",
        "solarized",
        "github_dark",
        "ayu_mirage",
        "synthwave",
        "material",
    ];
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeybindPreset {
    #[default]
    Default,
    Helix,
    Vim,
    Emacs,
}

impl std::str::FromStr for KeybindPreset {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(KeybindPreset::Default),
            "helix" => Ok(KeybindPreset::Helix),
            "vim" => Ok(KeybindPreset::Vim),
            "emacs" => Ok(KeybindPreset::Emacs),
            _ => Err(format!("Unknown keybind preset: {s}")),
        }
    }
}

impl std::fmt::Display for KeybindPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeybindPreset::Default => write!(f, "default"),
            KeybindPreset::Helix => write!(f, "helix"),
            KeybindPreset::Vim => write!(f, "vim"),
            KeybindPreset::Emacs => write!(f, "emacs"),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum IconMode {
    #[default]
    Nerd,
    Unicode,
    None,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreviewPosition {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HintBarStyle {
    #[default]
    #[serde(alias = "accent")]
    Classic,
    #[serde(alias = "powerline_sharp")]
    Sharp,
    #[serde(alias = "powerline_rounded")]
    Rounded,
    #[serde(alias = "powerline_slanted")]
    Slanted,
    Bubbles,
    Blurred,
    Chips,
    Brackets,
}

impl HintBarStyle {
    /// Every style in picker/display order.
    pub const ALL: [HintBarStyle; 8] = [
        HintBarStyle::Classic,
        HintBarStyle::Sharp,
        HintBarStyle::Rounded,
        HintBarStyle::Slanted,
        HintBarStyle::Bubbles,
        HintBarStyle::Blurred,
        HintBarStyle::Chips,
        HintBarStyle::Brackets,
    ];

    /// Display name ("Classic", "Bubbles", …).
    pub fn name(self) -> &'static str {
        match self {
            HintBarStyle::Classic => "Classic",
            HintBarStyle::Sharp => "Sharp",
            HintBarStyle::Rounded => "Rounded",
            HintBarStyle::Slanted => "Slanted",
            HintBarStyle::Bubbles => "Bubbles",
            HintBarStyle::Blurred => "Blurred",
            HintBarStyle::Chips => "Chips",
            HintBarStyle::Brackets => "Brackets",
        }
    }

    /// Config/template string ("classic", "bubbles", …) — matches serde names.
    pub fn as_config_str(self) -> &'static str {
        match self {
            HintBarStyle::Classic => "classic",
            HintBarStyle::Sharp => "sharp",
            HintBarStyle::Rounded => "rounded",
            HintBarStyle::Slanted => "slanted",
            HintBarStyle::Bubbles => "bubbles",
            HintBarStyle::Blurred => "blurred",
            HintBarStyle::Chips => "chips",
            HintBarStyle::Brackets => "brackets",
        }
    }

    /// Position in `ALL`.
    pub fn index(self) -> usize {
        HintBarStyle::ALL
            .iter()
            .position(|&s| s == self)
            .unwrap_or(0)
    }

    /// `ALL.get(idx)`, fallback Classic (default) on out-of-range.
    pub fn from_index(idx: usize) -> Self {
        HintBarStyle::ALL.get(idx).copied().unwrap_or_default()
    }

    /// Styles painting cells on filled backgrounds.
    /// true: Sharp|Rounded|Slanted|Bubbles|Blurred|Chips. false: Classic|Brackets.
    pub fn has_filled_cells(self) -> bool {
        match self {
            HintBarStyle::Classic | HintBarStyle::Brackets => false,
            HintBarStyle::Sharp
            | HintBarStyle::Rounded
            | HintBarStyle::Slanted
            | HintBarStyle::Bubbles
            | HintBarStyle::Blurred
            | HintBarStyle::Chips => true,
        }
    }

    /// Chained powerline family: Sharp|Rounded|Slanted.
    pub fn is_chained(self) -> bool {
        matches!(
            self,
            HintBarStyle::Sharp | HintBarStyle::Rounded | HintBarStyle::Slanted
        )
    }

    /// Detached-family edge glyphs (left, right), drawn fg=cell-bg on bar-bg.
    /// Bubbles → ("\u{e0b6}", "\u{e0b4}"); Blurred → ("░▒▓", "▓▒░"); Chips → ("", ""); others → None.
    pub fn cell_caps(self) -> Option<(&'static str, &'static str)> {
        match self {
            HintBarStyle::Bubbles => Some(("\u{e0b6}", "\u{e0b4}")),
            HintBarStyle::Blurred => Some(("░▒▓", "▓▒░")),
            HintBarStyle::Chips => Some(("", "")),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreviewExpandMode {
    #[default]
    Inline,
    External,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CalendarPosition {
    Top,
    #[default]
    Bottom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WeekStart {
    #[default]
    Sunday,
    Monday,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhysicsTickRate {
    #[default]
    Auto,
    Fixed,
}

impl FromStr for PhysicsTickRate {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(PhysicsTickRate::Auto),
            "fixed" => Ok(PhysicsTickRate::Fixed),
            _ => Err(format!("Unknown physics tick_rate: {s}")),
        }
    }
}

impl std::fmt::Display for PhysicsTickRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PhysicsTickRate::Auto => "auto",
                PhysicsTickRate::Fixed => "fixed",
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NotesSection {
    Calendar,
    #[default]
    Goals,
    Draw,
    Graf,
}

impl std::str::FromStr for NotesSection {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "calendar" => Ok(NotesSection::Calendar),
            "goals" => Ok(NotesSection::Goals),
            "draw" => Ok(NotesSection::Draw),
            "graf" => Ok(NotesSection::Graf),
            _ => Err(format!(
                "Unknown section: {s}. Expected calendar, goals, draw, or graf."
            )),
        }
    }
}

impl std::fmt::Display for NotesSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotesSection::Calendar => write!(f, "calendar"),
            NotesSection::Goals => write!(f, "goals"),
            NotesSection::Draw => write!(f, "draw"),
            NotesSection::Graf => write!(f, "graf"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn new_themes_round_trip() {
        let cases = [
            ("catppuccin_frappe", Theme::CatppuccinFrappe),
            ("catppuccin_macchiato", Theme::CatppuccinMacchiato),
            ("rose_pine_moon", Theme::RosePineMoon),
            ("gruvbox_material", Theme::GruvboxMaterial),
            ("github_dark", Theme::GithubDark),
            ("ayu_mirage", Theme::AyuMirage),
            ("synthwave", Theme::Synthwave),
            ("material", Theme::Material),
        ];
        for (s, variant) in cases {
            assert_eq!(Theme::from_str(s).unwrap(), variant, "parse {s}");
            assert_eq!(variant.to_string(), s, "display {s}");
        }
        // Concatenated aliases parse to the same variant.
        assert_eq!(Theme::from_str("githubdark").unwrap(), Theme::GithubDark);
        assert_eq!(Theme::from_str("synthwave84").unwrap(), Theme::Synthwave);
        assert_eq!(Theme::from_str("material_theme").unwrap(), Theme::Material);
    }

    #[test]
    fn unknown_theme_still_errors() {
        assert!(Theme::from_str("not_a_theme").is_err());
    }

    #[test]
    fn hint_bar_style_round_trip() {
        for s in HintBarStyle::ALL {
            assert_eq!(HintBarStyle::from_index(s.index()), s, "from_index roundtrip for {:?}", s);
        }
        // Names must be unique.
        let mut names: Vec<&str> = HintBarStyle::ALL.iter().map(|s| s.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), HintBarStyle::ALL.len(), "name() values not unique");
        // Config strings must be unique.
        let mut cfg: Vec<&str> = HintBarStyle::ALL.iter().map(|s| s.as_config_str()).collect();
        cfg.sort();
        cfg.dedup();
        assert_eq!(cfg.len(), HintBarStyle::ALL.len(), "as_config_str() values not unique");
    }
}
