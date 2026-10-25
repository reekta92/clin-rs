use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};
use strum::IntoEnumIterator;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::keybinds::Keybinds;

/// One scope's keybind rows, in action-enum declaration order, skipping
/// unbound actions. `keys_of` is a method pointer to the generated
/// `Keybinds::<scope>_keys_display` accessor (returns `String`, empty when
/// the action has no bound combo).
fn scope_keybind_lines<A>(
    keybinds: &Keybinds,
    keys_of: fn(&Keybinds, A) -> String,
) -> Vec<(String, String)>
where
    A: IntoEnumIterator + Copy + std::fmt::Debug,
{
    let mut out = Vec::new();
    for a in A::iter() {
        let keys = keys_of(keybinds, a);
        if keys.is_empty() {
            continue;
        }
        out.push((keys, format!("{a:?}")));
    }
    out
}

fn view_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::List => "List",
        ViewMode::Edit => "Editor",
        ViewMode::Help => "Help",
        ViewMode::Graph => "Graph",
        ViewMode::Draw => "Draw",
        ViewMode::Canvas => "Canvas",
        ViewMode::Bases => "Bases",
        ViewMode::Outline => "Outline",
        ViewMode::Calendar => "Calendar",
        ViewMode::Setup => "Setup",
    }
}

/// Render the QuickKeybinds dropdown over the header bar (row 0) + a list
/// below it (row 1+). Passive: never takes keys or mouse. Suppresses itself
/// when a popup, command palette, or another header-anchored overlay
/// (find/help-search) is open so they never visually collide.
pub fn draw_quick_keybinds(frame: &mut Frame, app: &App) {
    if app.base_embeds.has_popup()
        || !app.quick_keybinds_open
        || app.popups.active.is_some()
        || app.command_palette.is_some()
        || app.editor.find_popup.is_some()
        || app.help_search.popup.is_some()
        || app.vault_switcher.is_some()
        || (app.mode == ViewMode::Calendar
            && app
                .calendar_view
                .as_ref()
                .is_some_and(|view| view.dialog.is_some()))
        || app
            .bases_state
            .as_ref()
            .is_some_and(|state| state.has_popup())
    {
        return;
    }

    let theme: &AppThemeColors = &app.app_theme;
    let mut lines: Vec<(String, String)> = match app.mode {
        ViewMode::Calendar => [
            ("m / w / d / a", "Month / week / day / agenda"),
            ("n / e / Delete", "New / edit / delete event"),
            ("hjkl / Arrows", "Navigate dates / events"),
            ("J / K", "Next / previous event"),
            ("L / o", "Link / open note"),
            ("s / i / E", "Sync / import / export"),
            ("?", "Calendar help"),
            ("Esc", "Back to Notes"),
        ]
        .into_iter()
        .map(|(key, name)| (key.to_owned(), name.to_owned()))
        .collect(),
        ViewMode::List => scope_keybind_lines::<crate::keybinds::ListAction>(
            &app.keybinds,
            Keybinds::list_keys_display,
        ),
        ViewMode::Edit => scope_keybind_lines::<crate::keybinds::EditAction>(
            &app.keybinds,
            Keybinds::edit_keys_display,
        ),
        ViewMode::Help => scope_keybind_lines::<crate::keybinds::HelpAction>(
            &app.keybinds,
            Keybinds::help_keys_display,
        ),
        ViewMode::Graph => scope_keybind_lines::<crate::keybinds::GraphAction>(
            &app.keybinds,
            Keybinds::graph_keys_display,
        ),
        ViewMode::Draw => scope_keybind_lines::<crate::keybinds::DrawAction>(
            &app.keybinds,
            Keybinds::draw_keys_display,
        ),
        ViewMode::Canvas => scope_keybind_lines::<crate::keybinds::CanvasAction>(
            &app.keybinds,
            Keybinds::canvas_keys_display,
        ),
        ViewMode::Bases => scope_keybind_lines::<crate::keybinds::BasesAction>(
            &app.keybinds,
            Keybinds::bases_keys_display,
        ),
        ViewMode::Outline => scope_keybind_lines::<crate::keybinds::OutlineAction>(
            &app.keybinds,
            Keybinds::outline_keys_display,
        ),
        ViewMode::Setup => scope_keybind_lines::<crate::keybinds::SetupAction>(
            &app.keybinds,
            Keybinds::setup_keys_display,
        ),
    };

    let scope = if app.base_embeds.focused() {
        ViewMode::Bases
    } else {
        app.mode
    };
    let palette_keys = app.palette_keys_display(scope);
    if !palette_keys.is_empty() {
        lines.insert(0, (palette_keys, "Command palette".into()));
    }

    let frame_area = frame.area();
    if frame_area.height < 2 || frame_area.width == 0 {
        return;
    }

    // --- Compute popup width from content ---
    const SEP: &str = " • ";
    let max_key_width = lines.iter().map(|(k, _)| k.width()).max().unwrap_or(0);
    let max_action_width = lines.iter().map(|(_, d)| d.width()).max().unwrap_or(0);
    let inner_pad: u16 = 1;
    let content_width = (max_key_width + SEP.width() + max_action_width) as u16;
    let title = format!(" Keybinds — {} ", view_name(app.mode));
    let title = crate::ui::truncate_cells(&title, frame_area.width as usize);
    let title_width = title.width() as u16;
    let popup_width = content_width.max(title_width) + inner_pad * 2;
    let mut popup_width = popup_width.clamp(10, 60);
    let margin = 2.min(frame_area.width.saturating_sub(1) / 2);

    // --- Full-width header bar at row 0, centered title (mirrors draw_quick_search) ---
    let header_rect = Rect::new(frame_area.x, frame_area.y, frame_area.width, 1);
    frame.render_widget(Clear, header_rect);
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.accent)),
        header_rect,
    );
    let label_x = frame_area.x + (frame_area.width.saturating_sub(title_width)) / 2;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            title,
            Style::default().fg(theme.highlight_fg).bg(theme.accent),
        ))),
        Rect::new(label_x, frame_area.y, title_width, 1),
    );

    // --- Multi-column layout ---
    let avail_height = frame_area.height.saturating_sub(2);
    let rows_per_col = avail_height.max(1) as usize;
    const COL_GAP: u16 = 1;
    let max_total = frame_area.width.saturating_sub(margin * 2);
    let max_cols = ((max_total as usize + 1) / 21).max(1);
    let num_cols = lines.len().div_ceil(rows_per_col).max(1).min(max_cols);
    let gaps = (num_cols.saturating_sub(1) as u16).saturating_mul(COL_GAP);
    popup_width = popup_width.min(max_total.saturating_sub(gaps) / num_cols as u16);

    if lines.is_empty() {
        return;
    }

    // --- Multi-column dropdown, right-anchored ---
    let columns: Vec<&[(String, String)]> = lines.chunks(rows_per_col).take(num_cols).collect();
    let num_cols_actual = columns.len();
    let avail_content_width = popup_width.saturating_sub(inner_pad * 2);

    let dropdown_x = frame_area.right().saturating_sub(
        margin + (num_cols_actual as u16 * popup_width) + ((num_cols_actual as u16 - 1) * COL_GAP),
    );

    let alt_bg = crate::app_theme::darken(theme.accent, 18);

    for (ci, col_lines) in columns.iter().enumerate() {
        let col_x = dropdown_x + ci as u16 * (popup_width + COL_GAP);
        let col_height = col_lines.len() as u16;
        let col_area = Rect::new(col_x, frame_area.y + 1, popup_width, col_height);

        frame.render_widget(Clear, col_area);
        frame.render_widget(
            Block::default().style(Style::default().bg(theme.accent)),
            col_area,
        );

        for (i, (keys, desc)) in col_lines.iter().enumerate() {
            let bg = if i % 2 == 0 { theme.accent } else { alt_bg };
            let row_y = col_area.y + i as u16;

            let full_row = Rect::new(col_area.x, row_y, popup_width, 1);
            frame.render_widget(Clear, full_row);
            frame.render_widget(Block::default().style(Style::default().bg(bg)), full_row);

            let content_area = Rect::new(col_area.x + inner_pad, row_y, avail_content_width, 1);
            let row = Line::from(vec![
                Span::styled(
                    format!(
                        "{keys}{}",
                        " ".repeat(max_key_width.saturating_sub(keys.width()))
                    ),
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(SEP, Style::default().fg(theme.muted).bg(bg)),
                Span::styled(desc.clone(), Style::default().fg(theme.highlight_fg).bg(bg)),
            ]);
            frame.render_widget(
                Paragraph::new(crate::ui::fit_footer_line(
                    row,
                    content_area.width as usize,
                    theme,
                    true,
                )),
                content_area,
            );
        }
    }
    if num_cols_actual * rows_per_col < lines.len() {
        let x = dropdown_x + (num_cols_actual.saturating_sub(1) as u16) * (popup_width + COL_GAP);
        let rect = Rect::new(x, frame_area.y + rows_per_col as u16, popup_width, 1);
        frame.render_widget(
            Paragraph::new(crate::ui::truncate_cells(
                "… More shortcuts in Help",
                rect.width as usize,
            ))
            .style(Style::default().fg(theme.highlight_fg).bg(theme.accent)),
            rect,
        );
    }
}
