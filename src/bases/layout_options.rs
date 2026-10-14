//! Core layout settings remain in the lossless view mapping; native geometry uses terminal cells.
use super::format::{Layout, ViewConfig};
use anyhow::{Context, Result, bail};

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
#[cfg(test)]
mod tests {
    use super::*;
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
