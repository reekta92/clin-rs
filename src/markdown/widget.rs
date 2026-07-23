use super::style::RenderedDocument;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use std::ops::Range;

pub(crate) struct MarkdownWidget<'a> {
    document: &'a RenderedDocument,
    line_range: Range<usize>,
}

impl<'a> MarkdownWidget<'a> {
    pub fn new(document: &'a RenderedDocument, line_range: Range<usize>) -> Self {
        Self {
            document,
            line_range,
        }
    }
}

impl<'a> Widget for MarkdownWidget<'a> {
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
