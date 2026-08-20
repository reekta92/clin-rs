use ratatui::style::Color;

use super::{Background, Theme, ThemeColors};

struct GraphThemePalette {
    nodes: [[u8; 3]; 8],
    chrome: [u8; 3],
    text: [u8; 3],
    fg: [u8; 3],
    bg: [u8; 3],
}

impl GraphThemePalette {
    const fn rgb(c: [u8; 3]) -> Color {
        Color::Rgb(c[0], c[1], c[2])
    }

    fn build(&self, background: Background) -> ThemeColors {
        ThemeColors {
            node_colors: self.nodes.map(Self::rgb).to_vec(),
            edge_color: Self::rgb(self.chrome),
            border_color: Self::rgb(self.chrome),
            label_color: Self::rgb(self.text),
            selected_indicator_color: Self::rgb(self.fg),
            background_color: match background {
                Background::Transparent => None,
                Background::Solid => Some(Self::rgb(self.bg)),
            },
            minimap_border_color: Self::rgb(self.chrome),
            minimap_viewport_color: Self::rgb(self.fg),
            minimap_bg_color: Some(Self::rgb(self.bg)),
        }
    }
}

const PALETTES: [GraphThemePalette; 18] = [
    GraphThemePalette {
        nodes: [
            [122, 162, 247], // [0] accent — primary UI accent
            [187, 154, 247], // [1] tag — tag badges
            [125, 207, 255], // [2] folder — Vault category
            [224, 175, 104], // [3] heading — section titles (shared: success, warning)
            [158, 206, 106], // [4] pinned — Pinned category
            [247, 118, 142], // [5] destructive — encrypted notes, delete actions
            [148, 226, 213], // [6] smart — Smart folders category
            [255, 158, 100], // [7] subnote — Subnotes category
        ],
        chrome: [86, 95, 137],
        text: [203, 206, 215],
        fg: [255, 255, 255],
        bg: [26, 27, 38],
    },
    GraphThemePalette {
        nodes: [
            [137, 180, 250], // [0] accent — primary UI accent
            [203, 166, 247], // [1] tag — tag badges
            [116, 199, 236], // [2] folder — Vault category
            [249, 226, 175], // [3] heading — section titles (shared: success, warning)
            [166, 227, 161], // [4] pinned — Pinned category
            [245, 189, 220], // [5] destructive — encrypted notes, delete actions
            [242, 205, 205], // [6] smart — Smart folders category
            [250, 179, 135], // [7] subnote — Subnotes category
        ],
        chrome: [108, 112, 134],
        text: [205, 214, 244],
        fg: [205, 214, 244],
        bg: [30, 30, 46],
    },
    GraphThemePalette {
        nodes: [
            [97, 175, 239],  // [0] accent — primary UI accent
            [198, 120, 221], // [1] tag — tag badges
            [86, 182, 194],  // [2] folder — Vault category
            [229, 192, 123], // [3] heading — section titles (shared: success, warning)
            [152, 195, 121], // [4] pinned — Pinned category
            [224, 108, 117], // [5] destructive — encrypted notes, delete actions
            [224, 150, 108], // [6] smart — Smart folders category
            [171, 178, 191], // [7] subnote — Subnotes category
        ],
        chrome: [92, 99, 112],
        text: [171, 178, 191],
        fg: [220, 223, 228],
        bg: [40, 44, 52],
    },
    GraphThemePalette {
        nodes: [
            [184, 187, 38],  // [0] accent — primary UI accent
            [215, 153, 33],  // [1] tag — tag badges
            [204, 94, 74],   // [2] folder — Vault category
            [214, 93, 14],   // [3] heading — section titles (shared: success, warning)
            [104, 157, 106], // [4] pinned — Pinned category
            [131, 165, 152], // [5] destructive — encrypted notes, delete actions
            [146, 131, 116], // [6] smart — Smart folders category
            [254, 128, 25],  // [7] subnote — Subnotes category
        ],
        chrome: [102, 92, 84],
        text: [235, 219, 178],
        fg: [251, 241, 199],
        bg: [40, 40, 40],
    },
    GraphThemePalette {
        nodes: [
            [139, 233, 253], // [0] accent — primary UI accent
            [189, 147, 249], // [1] tag — tag badges
            [139, 233, 253], // [2] folder — Vault category
            [255, 184, 108], // [3] heading — section titles (shared: success, warning)
            [80, 250, 123],  // [4] pinned — Pinned category
            [255, 121, 198], // [5] destructive — encrypted notes, delete actions
            [255, 139, 127], // [6] smart — Smart folders category
            [255, 255, 150], // [7] subnote — Subnotes category
        ],
        chrome: [98, 114, 164],
        text: [248, 248, 242],
        fg: [255, 255, 255],
        bg: [40, 42, 54],
    },
    GraphThemePalette {
        nodes: [
            [136, 192, 208], // [0] accent — primary UI accent
            [143, 188, 187], // [1] tag — tag badges
            [163, 190, 140], // [2] folder — Vault category
            [235, 219, 178], // [3] heading — section titles (shared: success, warning)
            [214, 140, 140], // [4] pinned — Pinned category
            [216, 170, 133], // [5] destructive — encrypted notes, delete actions
            [200, 200, 200], // [6] smart — Smart folders category
            [163, 190, 140], // [7] subnote — Subnotes category
        ],
        chrome: [108, 120, 140],
        text: [216, 222, 233],
        fg: [236, 239, 244],
        bg: [46, 52, 64],
    },
    GraphThemePalette {
        nodes: [
            [180, 142, 173], // [0] accent — primary UI accent
            [234, 154, 151], // [1] tag — tag badges
            [156, 207, 216], // [2] folder — Vault category
            [246, 193, 119], // [3] heading — section titles (shared: success, warning)
            [155, 138, 221], // [4] pinned — Pinned category
            [235, 111, 146], // [5] destructive — encrypted notes, delete actions
            [159, 188, 198], // [6] smart — Smart folders category
            [209, 193, 168], // [7] subnote — Subnotes category
        ],
        chrome: [102, 110, 129],
        text: [87, 82, 121],
        fg: [87, 82, 121],
        bg: [40, 37, 61],
    },
    GraphThemePalette {
        nodes: [
            [255, 215, 89],  // [0] accent — primary UI accent
            [255, 143, 105], // [1] tag — tag badges
            [129, 204, 165], // [2] folder — Vault category
            [100, 200, 218], // [3] heading — section titles (shared: success, warning)
            [150, 205, 255], // [4] pinned — Pinned category
            [220, 150, 255], // [5] destructive — encrypted notes, delete actions
            [255, 180, 120], // [6] smart — Smart folders category
            [200, 230, 150], // [7] subnote — Subnotes category
        ],
        chrome: [95, 120, 102],
        text: [60, 76, 67],
        fg: [60, 76, 67],
        bg: [30, 38, 34],
    },
    GraphThemePalette {
        nodes: [
            [147, 191, 254], // [0] accent — primary UI accent
            [255, 158, 181], // [1] tag — tag badges
            [203, 166, 247], // [2] folder — Vault category
            [137, 180, 130], // [3] heading — section titles (shared: success, warning)
            [247, 234, 168], // [4] pinned — Pinned category
            [255, 173, 130], // [5] destructive — encrypted notes, delete actions
            [125, 196, 228], // [6] smart — Smart folders category
            [242, 205, 205], // [7] subnote — Subnotes category
        ],
        chrome: [95, 115, 135],
        text: [98, 114, 164],
        fg: [98, 114, 164],
        bg: [26, 30, 48],
    },
    GraphThemePalette {
        nodes: [
            [181, 137, 0],   // [0] accent — primary UI accent
            [203, 75, 22],   // [1] tag — tag badges
            [220, 50, 47],   // [2] folder — Vault category
            [211, 54, 130],  // [3] heading — section titles (shared: success, warning)
            [108, 113, 196], // [4] pinned — Pinned category
            [38, 139, 210],  // [5] destructive — encrypted notes, delete actions
            [42, 161, 152],  // [6] smart — Smart folders category
            [133, 153, 0],   // [7] subnote — Subnotes category
        ],
        chrome: [147, 161, 161],
        text: [131, 148, 150],
        fg: [253, 246, 227],
        bg: [0, 43, 54],
    },
    // Catppuccin Frappé (dark) — PALETTES[10]
    GraphThemePalette {
        nodes: [
            [202, 158, 230], // [0] accent — primary UI accent
            [244, 184, 228], // [1] tag — tag badges
            [140, 170, 238], // [2] folder — Vault category
            [231, 130, 132], // [3] heading — section titles (shared: success, warning)
            [166, 209, 137], // [4] pinned — Pinned category
            [234, 153, 156], // [5] destructive — encrypted notes, delete actions
            [153, 209, 219], // [6] smart — Smart folders category
            [239, 159, 118], // [7] subnote — Subnotes category
        ],
        chrome: [115, 121, 148],
        text: [198, 208, 245],
        fg: [181, 191, 226],
        bg: [48, 52, 70],
    },
    // Catppuccin Macchiato (dark) — PALETTES[11]
    GraphThemePalette {
        nodes: [
            [198, 160, 246], // [0] accent — primary UI accent
            [245, 189, 230], // [1] tag — tag badges
            [138, 173, 244], // [2] folder — Vault category
            [237, 135, 150], // [3] heading — section titles (shared: success, warning)
            [166, 218, 149], // [4] pinned — Pinned category
            [238, 153, 160], // [5] destructive — encrypted notes, delete actions
            [145, 215, 227], // [6] smart — Smart folders category
            [245, 169, 127], // [7] subnote — Subnotes category
        ],
        chrome: [110, 115, 141],
        text: [202, 211, 245],
        fg: [184, 192, 224],
        bg: [36, 39, 58],
    },
    // Rose Pine Moon (dark) — PALETTES[12]
    GraphThemePalette {
        nodes: [
            [196, 167, 231], // [0] accent — primary UI accent
            [235, 188, 186], // [1] tag — tag badges
            [62, 143, 176],  // [2] folder — Vault category
            [246, 193, 119], // [3] heading — section titles (shared: success, warning)
            [156, 207, 216], // [4] pinned — Pinned category
            [235, 111, 146], // [5] destructive — encrypted notes, delete actions
            [110, 106, 134], // [6] smart — Smart folders category
            [235, 188, 186], // [7] subnote — Subnotes category
        ],
        chrome: [96, 92, 116],
        text: [224, 222, 244],
        fg: [224, 222, 244],
        bg: [35, 33, 54],
    },
    // Gruvbox Material (dark) — PALETTES[13]
    GraphThemePalette {
        nodes: [
            [215, 153, 33],  // [0] accent — primary UI accent
            [211, 134, 155], // [1] tag — tag badges
            [125, 174, 163], // [2] folder — Vault category
            [234, 157, 52],  // [3] heading — section titles (shared: success, warning)
            [169, 182, 101], // [4] pinned — Pinned category
            [234, 105, 98],  // [5] destructive — encrypted notes, delete actions
            [137, 180, 130], // [6] smart — Smart folders category
            [240, 235, 215], // [7] subnote — Subnotes category
        ],
        chrome: [108, 100, 96],
        text: [235, 219, 178],
        fg: [251, 241, 199],
        bg: [40, 40, 40],
    },
    // GitHub Dark — PALETTES[14]
    GraphThemePalette {
        nodes: [
            [88, 166, 255],  // [0] accent — primary UI accent
            [188, 140, 255], // [1] tag — tag badges
            [63, 185, 80],   // [2] folder — Vault category
            [210, 153, 34],  // [3] heading — section titles (shared: success, warning)
            [255, 123, 114], // [4] pinned — Pinned category
            [248, 81, 73],   // [5] destructive — encrypted notes, delete actions
            [57, 210, 190],  // [6] smart — Smart folders category
            [139, 148, 158], // [7] subnote — Subnotes category
        ],
        chrome: [100, 108, 120],
        text: [201, 209, 217],
        fg: [255, 255, 255],
        bg: [13, 17, 23],
    },
    // Ayu Mirage (dark) — PALETTES[15]
    GraphThemePalette {
        nodes: [
            [255, 204, 102], // [0] accent — primary UI accent
            [255, 167, 89],  // [1] tag — tag badges
            [115, 184, 255], // [2] folder — Vault category
            [247, 135, 121], // [3] heading — section titles (shared: success, warning)
            [135, 201, 105], // [4] pinned — Pinned category
            [255, 167, 89],  // [5] destructive — encrypted notes, delete actions
            [57, 191, 204],  // [6] smart — Smart folders category
            [193, 202, 214], // [7] subnote — Subnotes category
        ],
        chrome: [100, 110, 125],
        text: [203, 204, 198],
        fg: [255, 255, 255],
        bg: [31, 36, 48],
    },
    // Synthwave '84 (dark) — PALETTES[16]
    GraphThemePalette {
        nodes: [
            [255, 126, 219], // [0] accent — primary UI accent
            [54, 249, 242],  // [1] tag — tag badges
            [123, 130, 149], // [2] folder — Vault category
            [255, 123, 114], // [3] heading — section titles (shared: success, warning)
            [114, 241, 184], // [4] pinned — Pinned category
            [254, 68, 68],   // [5] destructive — encrypted notes, delete actions
            [253, 226, 143], // [6] smart — Smart folders category
            [152, 154, 206], // [7] subnote — Subnotes category
        ],
        chrome: [108, 103, 131],
        text: [248, 248, 242],
        fg: [255, 255, 255],
        bg: [36, 29, 53],
    },
    // Material Theme (Darker) — PALETTES[17]
    GraphThemePalette {
        nodes: [
            [130, 170, 255], // [0] accent — primary UI accent
            [199, 146, 234], // [1] tag — tag badges
            [255, 203, 107], // [2] folder — Vault category
            [247, 140, 108], // [3] heading — section titles (shared: success, warning)
            [195, 232, 141], // [4] pinned — Pinned category
            [240, 113, 120], // [5] destructive — encrypted notes, delete actions
            [137, 221, 255], // [6] smart — Smart folders category
            [255, 139, 174], // [7] subnote — Subnotes category
        ],
        chrome: [80, 80, 80],
        text: [238, 255, 255],
        fg: [255, 255, 255],
        bg: [28, 28, 28],
    },
];

fn default_theme_colors(background: Background) -> ThemeColors {
    let gray = Color::Gray;
    let dark_gray = Color::DarkGray;
    let reset = Color::Reset;
    let white = Color::White;
    ThemeColors {
        node_colors: vec![
            Color::Cyan,
            Color::Magenta,
            Color::Blue,
            Color::Yellow,
            Color::Green,
            Color::Red,
            Color::LightCyan,
            Color::LightMagenta,
        ],
        edge_color: dark_gray,
        border_color: dark_gray,
        label_color: gray,
        selected_indicator_color: reset,
        background_color: match background {
            Background::Transparent => None,
            Background::Solid => Some(Color::Black),
        },
        minimap_border_color: dark_gray,
        minimap_viewport_color: white,
        minimap_bg_color: Some(Color::Black),
    }
}

pub fn theme_colors(theme: &Theme, background: Background) -> ThemeColors {
    match theme {
        Theme::Default => default_theme_colors(background),
        Theme::TokyoNight => PALETTES[0].build(background),
        Theme::CatppuccinMocha => PALETTES[1].build(background),
        Theme::Onedark => PALETTES[2].build(background),
        Theme::Gruvbox => PALETTES[3].build(background),
        Theme::Dracula => PALETTES[4].build(background),
        Theme::Nord => PALETTES[5].build(background),
        Theme::RosePine => PALETTES[6].build(background),
        Theme::Everforest => PALETTES[7].build(background),
        Theme::Kanagawa => PALETTES[8].build(background),
        Theme::Solarized => PALETTES[9].build(background),
        Theme::CatppuccinFrappe => PALETTES[10].build(background),
        Theme::CatppuccinMacchiato => PALETTES[11].build(background),
        Theme::RosePineMoon => PALETTES[12].build(background),
        Theme::GruvboxMaterial => PALETTES[13].build(background),
        Theme::GithubDark => PALETTES[14].build(background),
        Theme::AyuMirage => PALETTES[15].build(background),
        Theme::Synthwave => PALETTES[16].build(background),
        Theme::Material => PALETTES[17].build(background),
    }
}

/// Build [`ThemeColors`] from a custom theme's graph palette section.
///
/// Mirrors `GraphThemePalette::build()`. Missing or invalid hex values fall back
/// to `Color::Reset` (or `Color::Cyan` for the first node color).
pub fn custom_theme_colors(g: &crate::config::CustomGraph, background: Background) -> ThemeColors {
    let parse = |s: &str| crate::config::parse_hex_color(s).unwrap_or(Color::Reset);

    // Node colors: parse up to 8, pad with first-parsed or Cyan, truncate at 8.
    let parsed: Vec<Color> = g
        .nodes
        .iter()
        .filter_map(|h| crate::config::parse_hex_color(h))
        .collect();
    let default_node = parsed.first().copied().unwrap_or(Color::Cyan);
    let node_colors: Vec<Color> = if parsed.len() >= 8 {
        parsed[..8].to_vec()
    } else {
        let mut v = parsed;
        v.resize(8, default_node);
        v
    };

    let chrome = parse(&g.chrome);
    let text = parse(&g.text);
    let fg = parse(&g.fg);
    let bg_color =
        g.bg.as_ref()
            .and_then(|h| crate::config::parse_hex_color(h));

    ThemeColors {
        node_colors,
        edge_color: chrome,
        border_color: chrome,
        label_color: text,
        selected_indicator_color: fg,
        background_color: match background {
            Background::Transparent => None,
            Background::Solid => bg_color,
        },
        minimap_border_color: chrome,
        minimap_viewport_color: fg,
        minimap_bg_color: bg_color.or(Some(Color::Black)),
    }
}
