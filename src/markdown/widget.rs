use super::style::RenderedDocument;
use crate::config::TextAlignment;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

pub(crate) struct MarkdownWidget<'a> {
    document: &'a RenderedDocument,
    line_range: Range<usize>,
    text_align: TextAlignment,
}

impl<'a> MarkdownWidget<'a> {
    pub fn new(document: &'a RenderedDocument, line_range: Range<usize>) -> Self {
        Self {
            document,
            line_range,
            text_align: TextAlignment::Left,
        }
    }

    pub fn text_align(mut self, align: TextAlignment) -> Self {
        self.text_align = align;
        self
    }
}

impl Widget for MarkdownWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let start = self.line_range.start.min(self.document.line_count());
        let end = self
            .line_range
            .end
            .max(start)
            .min(self.document.line_count())
            .min(start.saturating_add(area.height as usize));

        for line_idx in start..end {
            let line = match self.document.line(line_idx) {
                Some(l) => l,
                None => continue,
            };
            let y = area.y + (line_idx - start) as u16;
            let mut x = area.x;

            // Compute content width for alignment.
            let content_width: u16 = line
                .spans
                .iter()
                .map(|s| UnicodeWidthStr::width(s.text.as_str()) as u16)
                .sum();

            match self.text_align {
                TextAlignment::Center if content_width < area.width => {
                    x += (area.width - content_width) / 2;
                }
                TextAlignment::Right if content_width < area.width => {
                    x += area.width - content_width;
                }
                _ => {}
            }

            for span in &line.spans {
                if x >= area.right() {
                    break;
                }
                if span.text.is_empty() {
                    continue;
                }
                let max_width = (area.right() - x) as usize;
                let (next_x, _) = buf.set_stringn(x, y, &span.text, max_width, span.style);
                x = next_x;
            }
        }
    }
}
