use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::app_theme::AppThemeColors;
use crate::list_view::PreviewContent;

pub fn draw_preview_pane(
    frame: &mut Frame,
    rect: Rect,
    theme: &AppThemeColors,
    content: Option<&PreviewContent>,
    hide_encrypted: bool,
    scroll_offset: u16,
    icon_mode: crate::config::IconMode,
) {
    if hide_encrypted {
        let lock_lines = vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!(
                        "{}  Encrypted Note",
                        crate::ui::get_icon("\u{f023}", "\u{1f512}", icon_mode)
                    ),
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
            Some(PreviewContent::Markdown(renderer)) if renderer.document().is_some() => {
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
                } else if let Some(doc) = renderer.document() {
                    let block = Block::default()
                        .style(theme.preview_bg_style())
                        .borders(Borders::NONE)
                        .padding(Padding::new(2, 2, 1, 1));
                    let inner = block.inner(rect);
                    frame.render_widget(block, rect);

                    let page = renderer.current_page_range();
                    let start = page
                        .start
                        .saturating_add(scroll_offset as usize)
                        .min(page.end);
                    let widget_range = start..page.end;
                    let widget = crate::markdown::MarkdownWidget::new(doc, widget_range);
                    frame.render_widget(widget, inner);
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
                                Style::default().fg(theme.muted).add_modifier(Modifier::DIM),
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
            Some(
                PreviewContent::CanvasGrid { grid, .. } | PreviewContent::DrawGrid { grid, .. },
            ) => {
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
            Some(PreviewContent::Image(_)) => {
                let loading = Paragraph::new("Image loading...")
                    .style(Style::default().fg(theme.muted))
                    .block(
                        Block::default()
                            .style(theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(loading, rect);
            }

            // Handled directly in draw_list_view; never reached here.
            Some(PreviewContent::SubnoteGraph { .. } | PreviewContent::FolderGraph { .. }) => {}
            Some(PreviewContent::SmartFolderInfo {
                kind: _,
                label,
                note_count,
                conditions,
            }) => {
                let mut lines: Vec<Line> = Vec::new();
                // Header
                let icon = crate::ui::get_icon("\u{f0e7}", "\u{26a1}", icon_mode);
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(" {icon} "),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        label.as_str(),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                // Conditions
                for cond in conditions {
                    lines.push(Line::from(vec![Span::styled(
                        format!("  \u{2022} {cond}"),
                        Style::default().fg(theme.fg),
                    )]));
                }
                lines.push(Line::from(""));
                // Note count
                let suffix = if *note_count == 1 { "note" } else { "notes" };
                lines.push(Line::from(vec![Span::styled(
                    format!("  {note_count} {suffix} match this folder"),
                    Style::default().fg(theme.muted),
                )]));
                let para = Paragraph::new(lines).style(theme.preview_bg_style()).block(
                    Block::default()
                        .style(theme.preview_bg_style())
                        .borders(Borders::NONE)
                        .padding(Padding::new(2, 2, 1, 1)),
                );
                frame.render_widget(para, rect);
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
