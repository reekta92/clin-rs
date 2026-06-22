pub trait OverlayView<R> {
    fn update(&mut self, _config: &mut crate::config::ClinConfig) {}
    fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        config: &crate::config::ClinConfig,
    );
    fn handle_event(
        &mut self,
        event: crossterm::event::Event,
        terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<R>>;
    fn title(&self) -> String;
    fn render_title(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
    ) {
        crate::ui::draw_view_title_bar(frame, area, &self.title(), theme, None);
    }
}

pub fn run_overlay<V, R>(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    view: &mut V,
    config: &mut crate::config::ClinConfig,
    theme: &crate::app_theme::AppThemeColors,
    poll_rate: std::time::Duration,
) -> anyhow::Result<R>
where
    V: OverlayView<R>,
{
    loop {
        view.update(config);
        terminal.draw(|f| {
            let outer = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ])
                .split(f.area());
            view.render_title(f, outer[0], theme);
            view.render(f, outer[1], theme, config);
        })?;

        if crossterm::event::poll(poll_rate)? {
            match crossterm::event::read()? {
                crossterm::event::Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    let _ = terminal.clear();
                }
                // Global Ctrl+C — immediately kill process
                crossterm::event::Event::Key(key)
                    if key.code == crossterm::event::KeyCode::Char('c')
                        && key.modifiers == crossterm::event::KeyModifiers::CONTROL =>
                {
                    crate::force_quit();
                }
                ev => {
                    if let Some(result) = view.handle_event(ev, terminal, config)? {
                        return Ok(result);
                    }
                }
            }
        }
    }
}
