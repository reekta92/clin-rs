pub mod app;
pub mod data;
pub mod input;
pub mod render;
pub mod state;

use ratatui::style::Color;

pub const COLOR_PICKER_PALETTE: &[(&str, &str, Color)] = &[
    ("red", "#ff5252", Color::Rgb(255, 82, 82)),
    ("orange", "#ff9800", Color::Rgb(255, 152, 0)),
    ("yellow", "#ffeb3b", Color::Rgb(255, 235, 59)),
    ("green", "#4caf50", Color::Rgb(76, 175, 80)),
    ("cyan", "#00bcd4", Color::Rgb(0, 188, 212)),
    ("purple", "#9c27b0", Color::Rgb(156, 39, 176)),
    ("blue", "#2196f3", Color::Rgb(33, 150, 243)),
    ("magenta", "#e91e63", Color::Rgb(233, 30, 99)),
    ("white", "#ffffff", Color::Rgb(255, 255, 255)),
];
