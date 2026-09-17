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
                TextAlignment::Justified if content_width < area.width && !line.is_blank => {
                    let continued = self
                        .document
                        .line(line_idx + 1)
                        .is_some_and(|n| n.source_line == line.source_line);
                    if continued {
                        struct Token<'a> {
                            text: &'a str,
                            is_space: bool,
                            style: ratatui::style::Style,
                        }
                        let mut tokens = Vec::new();
                        for span in &line.spans {
                            if span.text.is_empty() {
                                continue;
                            }
                            let mut start = 0;
                            let mut in_space = span.text.starts_with(' ');
                            for (i, c) in span.text.char_indices() {
                                let is_sp = c == ' ';
                                if is_sp != in_space {
                                    tokens.push(Token {
                                        text: &span.text[start..i],
                                        is_space: in_space,
                                        style: span.style,
                                    });
                                    start = i;
                                    in_space = is_sp;
                                }
                            }
                            if start < span.text.len() {
                                tokens.push(Token {
                                    text: &span.text[start..],
                                    is_space: in_space,
                                    style: span.style,
                                });
                            }
                        }

                        let mut first_non_space = None;
                        let mut last_non_space = None;
                        for (i, tok) in tokens.iter().enumerate() {
                            if !tok.is_space {
                                if first_non_space.is_none() {
                                    first_non_space = Some(i);
                                }
                                last_non_space = Some(i);
                            }
                        }

                        let mut gaps = 0;
                        if let (Some(first), Some(last)) = (first_non_space, last_non_space) {
                            for (i, tok) in tokens.iter().enumerate() {
                                if tok.is_space && i > first && i < last {
                                    gaps += 1;
                                }
                            }
                        }
                        #[allow(clippy::manual_checked_ops)]
                        if gaps > 0 {
                            let extra = area.width.saturating_sub(content_width) as usize;
                            let mut gap_idx = 0;
                            let first = first_non_space.expect("gaps > 0 implies Some");
                            let last = last_non_space.expect("gaps > 0 implies Some");

                            for (i, tok) in tokens.iter().enumerate() {
                                if x >= area.right() {
                                    break;
                                }
                                let max_width = (area.right() - x) as usize;
                                let (next_x, _) =
                                    buf.set_stringn(x, y, tok.text, max_width, tok.style);
                                x = next_x;

                                if tok.is_space && i > first && i < last {
                                    let add =
                                        extra / gaps + if gap_idx < extra % gaps { 1 } else { 0 };
                                    gap_idx += 1;
                                    for _ in 0..add {
                                        if x >= area.right() {
                                            break;
                                        }
                                        let (nx, _) = buf.set_stringn(x, y, " ", 1, tok.style);
                                        x = nx;
                                    }
                                }
                            }
                            // ponytail: wrapped headings/code share source_line continuation and will justify too.
                            continue;
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::style::{RenderLine, StyledSpan};
    use ratatui::style::Style;

    #[test]
    fn justified_expands_word_gaps() {
        let span = StyledSpan {
            text: "aa bb cc".to_string(),
            style: Style::default(),
        };
        let line = RenderLine {
            spans: vec![span],
            visual_width: 8,
            is_blank: false,
            image_url: None,
            source_line: 42,
        };
        let doc = RenderedDocument::new(vec![line.clone(), line]); // next line has same source_line
        let widget = MarkdownWidget::new(&doc, 0..1).text_align(TextAlignment::Justified);
        let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 20, 1));
        ratatui::widgets::Widget::render(widget, buf.area, &mut buf);
        let content: String = (0..20)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect::<String>();
        assert_eq!(content, "aa       bb       cc");
    }

    #[test]
    fn justified_last_line_ragged() {
        let span = StyledSpan {
            text: "aa bb cc".to_string(),
            style: Style::default(),
        };
        let line1 = RenderLine {
            spans: vec![span.clone()],
            visual_width: 8,
            is_blank: false,
            image_url: None,
            source_line: 42,
        };
        let line2 = RenderLine {
            spans: vec![span],
            visual_width: 8,
            is_blank: false,
            image_url: None,
            source_line: 43, // different source line!
        };
        let doc = RenderedDocument::new(vec![line1, line2]);
        let widget = MarkdownWidget::new(&doc, 0..1).text_align(TextAlignment::Justified);
        let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 20, 1));
        ratatui::widgets::Widget::render(widget, buf.area, &mut buf);
        let content: String = (0..20)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect::<String>();
        assert_eq!(content, "aa bb cc            ");
    }
}
