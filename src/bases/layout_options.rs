//! Core layout settings remain in the lossless view mapping; native geometry uses terminal cells.
use super::format::{Layout, ViewConfig};
use anyhow::{Context, Result, bail};

#[derive(Clone, Debug)]
pub(super) enum LayoutOptions {
    Table(TableOptions),
    List(ListOptions),
    Other,
}
impl LayoutOptions {
    pub fn parse(view: &ViewConfig) -> Result<Self> {
        Ok(match view.layout {
            Layout::Table => Self::Table(TableOptions::parse(view)?),
            Layout::List => Self::List(ListOptions::parse(view)?),
            _ => Self::Other,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Markers {
    Bullet,
    Number,
    None,
}
#[derive(Clone, Debug)]
pub(super) struct ListOptions {
    pub markers: Markers,
    pub indent: bool,
    pub separator: String,
}
impl ListOptions {
    pub fn parse(view: &ViewConfig) -> Result<Self> {
        let markers = match view
            .options
            .get("markers")
            .map(|value| value.as_str().context("markers must be text"))
            .transpose()?
            .unwrap_or("bullet")
        {
            "bullet" => Markers::Bullet,
            "number" => Markers::Number,
            "none" => Markers::None,
            _ => bail!("markers must be bullet, number or none"),
        };
        let indent = view
            .options
            .get("indentProperties")
            .map(|value| {
                value
                    .as_bool()
                    .context("indentProperties must be a boolean")
            })
            .transpose()?
            .unwrap_or(false);
        let separator = view
            .options
            .get("separator")
            .map(|value| value.as_str().context("separator must be text"))
            .transpose()?
            .filter(|value| !value.is_empty())
            .unwrap_or(", ")
            .to_owned();
        Ok(Self {
            markers,
            indent,
            separator,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImageFit {
    Cover,
    Contain,
}
#[derive(Clone, Debug)]
pub(super) struct CardOptions {
    pub width: u16,
    pub image: Option<String>,
    pub fit: ImageFit,
    pub ratio: f64,
}
impl CardOptions {
    pub fn parse(view: &ViewConfig, cell_width: u16) -> Result<Self> {
        let key = if matches!(view.layout, Layout::Kanban) {
            "columnWidth"
        } else {
            "cardSize"
        };
        let pixels = view
            .options
            .get(key)
            .map(|value| value.as_f64().context("card/column width must be numeric"))
            .transpose()?
            .unwrap_or(200.0);
        if !pixels.is_finite() || !(1.0..=65535.0).contains(&pixels) {
            bail!("card/column width must be between 1 and 65535 pixels");
        }
        let image = view
            .options
            .get("image")
            .map(|value| {
                value
                    .as_str()
                    .context("image property must be text")
                    .map(str::to_owned)
            })
            .transpose()?
            .filter(|value| !value.is_empty());
        let fit = match view
            .options
            .get("imageFit")
            .map(|value| value.as_str().context("imageFit must be text"))
            .transpose()?
            .unwrap_or("cover")
        {
            "" | "cover" => ImageFit::Cover,
            "contain" => ImageFit::Contain,
            _ => bail!("imageFit must be cover or contain"),
        };
        let ratio = view
            .options
            .get("imageAspectRatio")
            .map(|value| value.as_f64().context("image aspect ratio must be numeric"))
            .transpose()?
            .unwrap_or(1.0);
        if !ratio.is_finite() || ratio <= 0.0 {
            bail!("image aspect ratio must be positive and finite");
        }
        Ok(Self {
            width: (pixels / f64::from(cell_width.max(1)))
                .round()
                .clamp(12.0, 65535.0) as u16,
            image,
            fit,
            ratio,
        })
    }
    pub fn cover_height(&self, width: u16, cell: (u16, u16), available: u16) -> u16 {
        if self.image.is_none() {
            return 0;
        }
        (f64::from(width) * f64::from(cell.0.max(1)) / f64::from(cell.1.max(1)) / self.ratio)
            .round()
            .clamp(1.0, f64::from(available.max(1))) as u16
    }
}
#[derive(Clone, Debug)]
pub(super) struct TableOptions {
    pub height: u16,
    pub widths: std::collections::BTreeMap<String, f64>,
}
impl TableOptions {
    pub fn parse(view: &ViewConfig) -> Result<Self> {
        let height = match view
            .options
            .get("rowHeight")
            .map(|value| value.as_str().context("rowHeight must be text"))
            .transpose()?
            .unwrap_or("")
        {
            "" => 1,
            "medium" => 2,
            "tall" => 4,
            "extra" => 8,
            _ => bail!("rowHeight must be empty (short), medium, tall or extra"),
        };
        let mut widths = std::collections::BTreeMap::new();
        if let Some(value) = view.options.get("columnSize") {
            for (key, value) in value
                .as_mapping()
                .context("columnSize must be a property-width mapping")?
            {
                let key = key.as_str().context("columnSize property must be text")?;
                let pixels = value.as_f64().context("columnSize width must be numeric")?;
                if !pixels.is_finite() || !(1.0..=65535.0).contains(&pixels) {
                    bail!("columnSize width must be between 1 and 65535 pixels");
                }
                widths.insert(key.to_owned(), pixels);
            }
        }
        Ok(Self { height, widths })
    }
    pub fn width(&self, property: &str, cell_width: u16) -> u16 {
        self.widths.get(property).map_or(20, |pixels| {
            (pixels / f64::from(cell_width.max(1)))
                .round()
                .clamp(4.0, 65535.0) as u16
        })
    }

    pub fn fitted_widths(&self, properties: &[String], width: u16, cell_width: u16) -> Vec<u16> {
        let mut widths: Vec<_> = properties
            .iter()
            .map(|property| self.width(property, cell_width))
            .collect();
        let mut total: u64 = widths.iter().map(|width| u64::from(*width)).sum();
        if total <= u64::from(width) {
            return widths;
        }
        let mut remaining = width;
        let count = widths.len();
        for (index, column) in widths.iter_mut().enumerate() {
            let reserved = (count - index - 1).min(usize::from(remaining)) as u16;
            let fitted = (u64::from(remaining) * u64::from(*column) / total)
                .max(1)
                .min(u64::from(remaining - reserved)) as u16;
            total -= u64::from(*column);
            *column = fitted;
            remaining -= fitted;
        }
        widths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bases::format::BaseDocument;
    #[test]
    fn list_options_reject_invalid_types_and_keep_core_defaults() {
        for field in [
            "markers: bullets",
            "markers: 1",
            "indentProperties: yes",
            "indentProperties: null",
            "separator: []",
            "separator: null",
        ] {
            let document =
                BaseDocument::parse(&format!("views: [{{type: list, {field}}}]\n")).unwrap();
            assert!(
                ListOptions::parse(&document.config.views[0]).is_err(),
                "{field}"
            );
        }
        for field in ["", "separator: ''"] {
            let document =
                BaseDocument::parse(&format!("views: [{{type: list, {field}}}]\n")).unwrap();
            let options = ListOptions::parse(&document.config.views[0]).unwrap();
            assert_eq!(options.markers, Markers::Bullet);
            assert!(!options.indent);
            assert_eq!(options.separator, ", ");
        }
    }
    #[test]
    fn table_options_validate_core_types_and_pixel_widths() {
        for (height, lines) in [("''", 1), ("medium", 2), ("tall", 4), ("extra", 8)] {
            let doc=BaseDocument::parse(&format!("views: [{{type: table, name: Test, rowHeight: {height}, columnSize: {{file.name: 240}}}}]" )).unwrap();
            let options = TableOptions::parse(&doc.config.views[0]).unwrap();
            assert_eq!(options.height, lines);
            assert_eq!(options.width("file.name", 8), 30);
        }
        for option in [
            "rowHeight: huge",
            "rowHeight: 4",
            "columnSize: 5",
            "columnSize: {file.name: 0}",
            "columnSize: {file.name: true}",
            "columnSize: {1: 10}",
        ] {
            let doc =
                BaseDocument::parse(&format!("views: [{{type: table, name: Test, {option}}}]"))
                    .unwrap();
            assert!(TableOptions::parse(&doc.config.views[0]).is_err());
        }
    }
    #[test]
    fn core_card_settings_validate_and_map_pixel_geometry_to_cells() {
        let document=crate::bases::format::BaseDocument::parse("views: [{type: cards, cardSize: 320, image: note.cover, imageFit: contain, imageAspectRatio: 2}]\n").unwrap();
        let options = CardOptions::parse(&document.config.views[0], 8).unwrap();
        assert_eq!(options.width, 40);
        assert_eq!(options.fit, ImageFit::Contain);
        assert_eq!(options.cover_height(40, (8, 16), 100), 10);
        for setting in [
            "cardSize: -1",
            "imageAspectRatio: 0",
            "imageFit: stretch",
            "image: 123",
        ] {
            let document = crate::bases::format::BaseDocument::parse(&format!(
                "views: [{{type: cards, {setting}}}]\n"
            ))
            .unwrap();
            assert!(
                CardOptions::parse(&document.config.views[0], 8).is_err(),
                "{setting}"
            );
        }
    }
}
