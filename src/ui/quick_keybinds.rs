use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};
use strum::IntoEnumIterator;

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
        ViewMode::Backup => "Backup",
        ViewMode::Outline => "Outline",
        ViewMode::Setup => "Setup",
    }
}

/// Render the QuickKeybinds dropdown over the header bar (row 0) + a list
/// below it (row 1+). Passive: never takes keys or mouse. Suppresses itself
/// when a popup, command palette, or another header-anchored overlay
/// (find/help-search) is open so they never visually collide.
pub fn draw_quick_keybinds(frame: &mut Frame, app: &App) {
    if !app.quick_keybinds_open
        || app.popups.active.is_some()
        || app.command_palette.is_some()
        || app.editor.find_popup.is_some()
        || app.help_search.popup.is_some()
    {
        return;
    }

    let theme: &AppThemeColors = &app.app_theme;
    let lines: Vec<(String, String)> = match app.mode {
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
        ViewMode::Backup => scope_keybind_lines::<crate::keybinds::BackupAction>(
            &app.keybinds,
            Keybinds::backup_keys_display,
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

    let frame_area = frame.area();

    // --- Compute popup width from content ---
    const SEP: &str = " • ";
    let max_key_width = lines.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    let max_action_width = lines.iter().map(|(_, d)| d.chars().count()).max().unwrap_or(0);
    let inner_pad: u16 = 1;
    let content_width = (max_key_width + SEP.chars().count() + max_action_width) as u16;
    let title = format!(" Keybinds — {} ", view_name(app.mode));
    let title_width = title.chars().count() as u16;
    let popup_width = content_width.max(title_width) + inner_pad * 2;
    let popup_width = popup_width.min(60).max(10);
    let margin: u16 = 2;

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

    if lines.is_empty() {
        return;
    }

    // --- Dropdown at top-right, fits content, inner padding, alternating rows ---
    let x = frame_area.right().saturating_sub(popup_width + margin);
    let avail_height = frame_area.height.saturating_sub(2);
    let height = (lines.len() as u16).min(avail_height.max(1));
    let dropdown_area = Rect::new(x, frame_area.y + 1, popup_width, height);
    frame.render_widget(Clear, dropdown_area);
    // Base background fills gaps (right padding after shorter rows, empty space)
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.accent)),
        dropdown_area,
    );

    let alt_bg = darken(theme.accent, 18);

    for (i, (keys, desc)) in lines.iter().take(height as usize).enumerate() {
        let bg = if i % 2 == 0 { theme.accent } else { alt_bg };
        let row_y = dropdown_area.y + i as u16;

        // Clear and fill the full row width with alternating background
        let full_row = Rect::new(dropdown_area.x, row_y, popup_width, 1);
        frame.render_widget(Clear, full_row);
        frame.render_widget(Block::default().style(Style::default().bg(bg)), full_row);

        // Render content at padded offset
        let content_area = Rect::new(dropdown_area.x + inner_pad, row_y, content_width, 1);
        let row = Line::from(vec![
            Span::styled(
                format!("{:<width$}", keys, width = max_key_width),
                Style::default().fg(theme.highlight_fg).bg(bg),
            ),
            Span::styled(SEP, Style::default().fg(theme.muted).bg(bg)),
            Span::styled(desc.clone(), Style::default().fg(theme.text).bg(bg)),
        ]);
        frame.render_widget(Paragraph::new(row), content_area);
    }
}

/// Darken an RGB color by subtracting `delta` from each channel.
fn darken(c: Color, delta: u8) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_sub(delta),
            g.saturating_sub(delta),
            b.saturating_sub(delta),
        ),
        other => other,
    }
}
