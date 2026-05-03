use ratatui::style::Color;

use super::{Background, Theme, ThemeColors};

struct ThemePalette {
    nodes: [[u8; 3]; 8],
    chrome: [u8; 3],
    title: [u8; 3],
    text: [u8; 3],
    fg: [u8; 3],
    grid: [u8; 3],
    bg: [u8; 3],
}

impl ThemePalette {
    const fn rgb(c: [u8; 3]) -> Color {
        Color::Rgb(c[0], c[1], c[2])
    }

    fn build(&self, background: Background) -> ThemeColors {
        ThemeColors {
            node_colors: self.nodes.map(Self::rgb).to_vec(),
            edge_color: Self::rgb(self.chrome),
            border_color: Self::rgb(self.chrome),
            title_color: Self::rgb(self.title),
            label_color: Self::rgb(self.text),
            legend_text_color: Self::rgb(self.text),
            legend_border_color: Self::rgb(self.chrome),
            selected_indicator_color: Self::rgb(self.fg),
            grid_color: Self::rgb(self.grid),
            background_color: match background {
                Background::Transparent => None,
                Background::Solid => Some(Self::rgb(self.bg)),
            },
            status_bar_color: Self::rgb(self.chrome),
            minimap_border_color: Self::rgb(self.chrome),
            minimap_viewport_color: Self::rgb(self.fg),
            minimap_bg_color: Some(Self::rgb(self.bg)),
        }
    }
}

const PALETTES: [ThemePalette; 10] = [
    // 0: TokyoNight
    ThemePalette {
        nodes: [
            [122, 162, 247],
            [187, 154, 247],
            [125, 207, 255],
            [224, 175, 104],
            [158, 206, 106],
            [247, 118, 142],
            [148, 226, 213],
            [255, 158, 100],
        ],
        chrome: [86, 95, 137],
        title: [187, 154, 247],
        text: [203, 206, 215],
        fg: [255, 255, 255],
        grid: [56, 62, 95],
        bg: [26, 27, 38],
    },
    // 1: CatppuccinMocha
    ThemePalette {
        nodes: [
            [137, 180, 250],
            [203, 166, 247],
            [116, 199, 236],
            [249, 226, 175],
            [166, 227, 161],
            [245, 189, 220],
            [242, 205, 205],
            [250, 179, 135],
        ],
        chrome: [108, 112, 134],
        title: [205, 214, 244],
        text: [205, 214, 244],
        fg: [205, 214, 244],
        grid: [49, 50, 68],
        bg: [30, 30, 46],
    },
    // 2: Onedark
    ThemePalette {
        nodes: [
            [97, 175, 239],
            [198, 120, 221],
            [86, 182, 194],
            [229, 192, 123],
            [152, 195, 121],
            [224, 108, 117],
            [224, 150, 108],
            [171, 178, 191],
        ],
        chrome: [92, 99, 112],
        title: [171, 178, 191],
        text: [171, 178, 191],
        fg: [220, 223, 228],
        grid: [56, 63, 76],
        bg: [40, 44, 52],
    },
    // 3: Gruvbox
    ThemePalette {
        nodes: [
            [184, 187, 38],
            [215, 153, 33],
            [204, 94, 74],
            [214, 93, 14],
            [104, 157, 106],
            [131, 165, 152],
            [146, 131, 116],
            [254, 128, 25],
        ],
        chrome: [102, 92, 84],
        title: [235, 219, 178],
        text: [235, 219, 178],
        fg: [251, 241, 199],
        grid: [60, 56, 54],
        bg: [40, 40, 40],
    },
    // 4: Dracula
    ThemePalette {
        nodes: [
            [139, 233, 253],
            [189, 147, 249],
            [139, 233, 253],
            [255, 184, 108],
            [80, 250, 123],
            [255, 121, 198],
            [255, 139, 127],
            [255, 255, 150],
        ],
        chrome: [98, 114, 164],
        title: [248, 248, 242],
        text: [248, 248, 242],
        fg: [255, 255, 255],
        grid: [68, 71, 90],
        bg: [40, 42, 54],
    },
    // 5: Nord
    ThemePalette {
        nodes: [
            [136, 192, 208],
            [143, 188, 187],
            [163, 190, 140],
            [235, 219, 178],
            [214, 140, 140],
            [216, 170, 133],
            [200, 200, 200],
            [163, 190, 140],
        ],
        chrome: [67, 76, 94],
        title: [216, 222, 233],
        text: [216, 222, 233],
        fg: [236, 239, 244],
        grid: [59, 66, 82],
        bg: [46, 52, 64],
    },
    // 6: RosePine
    ThemePalette {
        nodes: [
            [180, 142, 173],
            [234, 154, 151],
            [156, 207, 216],
            [246, 193, 119],
            [155, 138, 221],
            [235, 111, 146],
            [159, 188, 198],
            [209, 193, 168],
        ],
        chrome: [102, 110, 129],
        title: [87, 82, 121],
        text: [87, 82, 121],
        fg: [87, 82, 121],
        grid: [57, 53, 82],
        bg: [40, 37, 61],
    },
    // 7: Everforest
    ThemePalette {
        nodes: [
            [255, 215, 89],
            [255, 143, 105],
            [129, 204, 165],
            [100, 200, 218],
            [150, 205, 255],
            [220, 150, 255],
            [255, 180, 120],
            [200, 230, 150],
        ],
        chrome: [95, 120, 102],
        title: [60, 76, 67],
        text: [60, 76, 67],
        fg: [60, 76, 67],
        grid: [40, 50, 45],
        bg: [30, 38, 34],
    },
    // 8: Kanagawa
    ThemePalette {
        nodes: [
            [147, 191, 254],
            [255, 158, 181],
            [203, 166, 247],
            [137, 180, 130],
            [247, 234, 168],
            [255, 173, 130],
            [125, 196, 228],
            [242, 205, 205],
        ],
        chrome: [95, 115, 135],
        title: [98, 114, 164],
        text: [98, 114, 164],
        fg: [98, 114, 164],
        grid: [34, 40, 62],
        bg: [26, 30, 48],
    },
    // 9: Solarized
    ThemePalette {
        nodes: [
            [181, 137, 0],
            [203, 75, 22],
            [220, 50, 47],
            [211, 54, 130],
            [108, 113, 196],
            [38, 139, 210],
            [42, 161, 152],
            [133, 153, 0],
        ],
        chrome: [147, 161, 161],
        title: [131, 148, 150],
        text: [131, 148, 150],
        fg: [253, 246, 227],
        grid: [0, 74, 94], // Lighter than bg (0, 43, 54) for visibility
        bg: [0, 43, 54],
    },
];

fn default_theme_colors(background: Background) -> ThemeColors {
    let gray = Color::Gray;
    let dark_gray = Color::DarkGray;
    let reset = Color::Reset;
    let white = Color::White;
    ThemeColors {
        node_colors: vec![gray; 8],
        edge_color: dark_gray,
        border_color: dark_gray,
        title_color: gray,
        label_color: gray,
        legend_text_color: gray,
        legend_border_color: dark_gray,
        selected_indicator_color: reset,
        grid_color: dark_gray,
        background_color: match background {
            Background::Transparent => None,
            Background::Solid => Some(Color::Black),
        },
        status_bar_color: dark_gray,
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
    }
}
