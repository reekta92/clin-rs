use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

use crate::app_theme::AppThemeColors;
use crate::list_view::PreviewContent;

/// Draw a preview pane with shared rendering logic.
///
/// Handles all 5 states:
/// 1. `hide_encrypted` → lock icon placeholder
/// 2. Markdown pending → "Rendering preview..."
/// 3. Markdown empty → "(empty note)"
/// 4. Markdown with pages → rendered snapshot + page indicator
/// 5. CanvasGrid / DrawGrid → rendered snapshot
/// 6. `None` → "Select a note to preview"
pub fn draw_preview_pane(
    frame: &mut Frame,
    rect: Rect,
    theme: &AppThemeColors,
    content: Option<&PreviewContent>,
    hide_encrypted: bool,
    scroll_offset: u16,
) {
    if hide_encrypted {
        let lock_lines = vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "\u{f023}  Encrypted Note",
                    Style::default()
                        .fg(theme.destructive)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Content hidden — decrypt to preview",
                    Style::default().fg(theme.muted),
                ),
            ]),
        ];
        let lock_para = Paragraph::new(lock_lines)
            .style(theme.preview_bg_style())
            .block(
                Block::default()
                    .style(theme.preview_bg_style())
                    .borders(Borders::NONE)
                    .padding(Padding::new(2, 2, 1, 1)),
            );
        frame.render_widget(lock_para, rect);
    } else {
        match content {
            Some(PreviewContent::Markdown(renderer))
                if !renderer.is_pending() && renderer.pages_built() =>
            {
                if renderer.is_content_empty() {
                    let placeholder = Paragraph::new(Line::from(vec![Span::styled(
                        "(empty note)",
                        Style::default().fg(theme.muted),
                    )]))
                    .style(theme.preview_bg_style())
                    .block(
                        Block::default()
                            .style(theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                    frame.render_widget(placeholder, rect);
                } else if let Some(page_grid) = renderer.current_page_grid() {
                    let snapshot = crate::snapshot::RenderedSnapshot::new(page_grid)
                        .scroll_offset(scroll_offset)
                        .block(
                            Block::default()
                                .style(theme.preview_bg_style())
                                .borders(Borders::NONE)
                                .padding(Padding::new(2, 2, 1, 1)),
                        );
                    frame.render_widget(snapshot, rect);
                    if renderer.total_pages() > 1 {
                        let indicator = format!(
                            " {}/{} ",
                            renderer.current_page() + 1,
                            renderer.total_pages()
                        );
                        let ind_width = indicator.len() as u16;
                        let ind_x = rect.right().saturating_sub(ind_width + 2);
                        let ind_y = rect.bottom().saturating_sub(1);
                        if ind_x >= rect.x && ind_y >= rect.y {
                            let ind_area = Rect::new(ind_x, ind_y, ind_width, 1);
                            let ind_widget = Paragraph::new(Span::styled(
                                indicator,
                                Style::default()
                                    .fg(theme.muted)
                                    .add_modifier(Modifier::DIM),
                            ));
                            frame.render_widget(ind_widget, ind_area);
                        }
                    }
                }
            }
            Some(PreviewContent::Markdown(_)) => {
                let loading = Paragraph::new("Rendering preview...")
                    .style(Style::default().fg(theme.muted))
                    .block(
                        Block::default()
                            .style(theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(loading, rect);
            }
            Some(PreviewContent::CanvasGrid(grid) | PreviewContent::DrawGrid(grid)) => {
                let snapshot = crate::snapshot::RenderedSnapshot::new(grid)
                    .scroll_offset(scroll_offset)
                    .block(
                        Block::default()
                            .style(theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(snapshot, rect);
            }
            None => {
                let placeholder = Paragraph::new("Select a note to preview")
                    .style(theme.preview_bg_style())
                    .block(
                        Block::default()
                            .style(theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(placeholder, rect);
            }
        }
    }
}
