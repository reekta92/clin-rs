pub mod app;
pub mod data;
pub mod input;
pub mod render;
pub mod state;

use ratatui::style::Color;

pub const COLOR_PICKER_PALETTE: &[(&str, &str, Color)] = &[
    ("Red", "#ff5252", Color::Rgb(255, 82, 82)),
    ("Orange", "#ff9800", Color::Rgb(255, 152, 0)),
    ("Yellow", "#ffeb3b", Color::Rgb(255, 235, 59)),
    ("Green", "#4caf50", Color::Rgb(76, 175, 80)),
    ("Cyan", "#00bcd4", Color::Rgb(0, 188, 212)),
    ("Purple", "#9c27b0", Color::Rgb(156, 39, 176)),
    ("Blue", "#2196f3", Color::Rgb(33, 150, 243)),
    ("Magenta", "#e91e63", Color::Rgb(233, 30, 99)),
    ("White", "#ffffff", Color::Rgb(255, 255, 255)),
];
