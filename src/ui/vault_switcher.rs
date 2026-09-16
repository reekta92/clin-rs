use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::app::App;
use crate::app_theme::AppThemeColors;

/// Render the vault switcher overlay: full-width accent header bar at row 0,
/// centered title, half-width centered dropdown of vault rows plus a trailing
/// `+ Add new vault…` row beneath. Alternating bg per vault; selected row uses
/// the standard highlight pair. Passive suppression mirrors draw_quick_keybinds.
pub fn draw_vault_switcher(frame: &mut Frame, app: &App) {
    let Some(switcher) = app.vault_switcher.as_ref() else {
        return;
    };
    if app.popups.active.is_some()
        || app.command_palette.is_some()
        || app.popups.confirm.is_some()
        || app.editor.find_popup.is_some()
        || app.help_search.popup.is_some()
        || app.quick_keybinds_open
    {
        return;
    }

    let theme: &AppThemeColors = &app.app_theme;
    let frame_area = frame.area();

    // --- Full-width header bar at row 0, centered title ---
    let title = " [F4] Vaults ";
    let title_width = title.chars().count() as u16;
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

    // --- Dropdown: ~half window, centered (message overlay sizing) ---
    let popup_width = (frame_area.width / 2).clamp(30, 80);
    let inner_width = popup_width.saturating_sub(2) as usize;

    let data_dir = app.storage.data_dir.clone();
    let mut rows: Vec<(String, String, bool)> = switcher // (name, path, is_active)
        .vaults
        .iter()
        .map(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned());
            let is_active = crate::app::vaults::same_vault(p, &data_dir);
            (name, p.to_string_lossy().into_owned(), is_active)
        })
        .collect();
    let add_label = "+ Add new vault…";
    rows.push((add_label.to_string(), String::new(), false));
    let add_index = rows.len() - 1;

    // --- Scroll window: 12 visible rows, clamp keeps selection on screen ---
    let max_visible_rows = 12usize;
    let scroll = switcher
        .selected
        .saturating_sub(max_visible_rows.saturating_sub(1))
        .min(rows.len().saturating_sub(max_visible_rows));

    let total_rows = rows.len().min(max_visible_rows);
    let x = frame_area.x + (frame_area.width.saturating_sub(popup_width)) / 2;
    let height = (total_rows as u16).min(frame_area.height.saturating_sub(2).max(1));
    let dropdown_area = Rect::new(x, frame_area.y + 1, popup_width, height);

    frame.render_widget(Clear, dropdown_area);
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.accent)),
        dropdown_area,
    );

    const SEP: &str = " • ";

    for (i, (name, path, is_active)) in rows.iter().skip(scroll).take(height as usize).enumerate() {
        let row_index = scroll + i;
        let row_y = dropdown_area.y + i as u16;

        // Single accent bg for all rows; selection uses the quick-search
        // convention (`heading` bg, `highlight_fg` text).
        let (bg, fg) = if row_index == switcher.selected {
            (theme.heading, theme.highlight_fg)
        } else {
            (theme.accent, theme.highlight_fg)
        };

        let full_row = Rect::new(dropdown_area.x, row_y, popup_width, 1);
        frame.render_widget(Clear, full_row);
        frame.render_widget(Block::default().style(Style::default().bg(bg)), full_row);

        let mut spans: Vec<Span> = Vec::new();
        if row_index == add_index {
            spans.push(Span::styled(
                add_label,
                Style::default().fg(theme.muted).bg(bg),
            ));
        } else {
            let prefix = if *is_active { "● " } else { "  " };
            spans.push(Span::styled(
                prefix,
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                name.clone(),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(SEP, Style::default().fg(theme.muted).bg(bg)));
            let used = prefix.chars().count() + name.chars().count() + SEP.chars().count();
            let path_width = inner_width.saturating_sub(used + 1);
            let path_display: String = {
                let chars: Vec<char> = path.chars().collect();
                if chars.len() > path_width && path_width > 1 {
                    let keep = path_width.saturating_sub(1);
                    let mut s: String = chars[chars.len() - keep..].iter().collect();
                    s.insert(0, '…');
                    s
                } else {
                    chars.into_iter().collect()
                }
            };
            spans.push(Span::styled(
                path_display,
                Style::default().fg(theme.muted).bg(bg),
            ));
        }

        let content_area = Rect::new(dropdown_area.x + 1, row_y, popup_width.saturating_sub(2), 1);
        frame.render_widget(Paragraph::new(Line::from(spans)), content_area);
    }
}
