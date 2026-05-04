import re

def process_file():
    with open('src/ui.rs', 'r') as f:
        text = f.read()

    # 1. Update function signatures
    text = text.replace(
        "pub fn help_page_text(keybinds: &Keybinds) -> Text<'static> {",
        "pub fn help_page_text(keybinds: &Keybinds, theme: &crate::app_theme::AppThemeColors) -> Text<'static> {"
    )
    text = text.replace(
        "pub fn help_heading(icon: &'static str, title: &'static str) -> Line<'static> {",
        "pub fn help_heading(icon: &'static str, title: &'static str, theme: &crate::app_theme::AppThemeColors) -> Line<'static> {"
    )
    text = text.replace(
        "pub fn help_item_dyn(text: &str, key: Option<&str>) -> Vec<Line<'static>> {",
        "pub fn help_item_dyn(text: &str, key: Option<&str>, theme: &crate::app_theme::AppThemeColors) -> Vec<Line<'static>> {"
    )

    text = text.replace(
        "pub fn draw_template_popup(frame: &mut Frame, popup: &TemplatePopup, area: Rect) {",
        "pub fn draw_template_popup(frame: &mut Frame, popup: &TemplatePopup, area: Rect, theme: &crate::app_theme::AppThemeColors) {"
    )
    text = text.replace(
        "pub fn draw_confirm_popup(frame: &mut Frame, popup: &ConfirmPopup, area: Rect) {",
        "pub fn draw_confirm_popup(frame: &mut Frame, popup: &ConfirmPopup, area: Rect, theme: &crate::app_theme::AppThemeColors) {"
    )

    # 2. Update calls in help_page_text
    text = text.replace(
        "pub fn help_heading(icon: &'static str, title: &'static str, theme: &crate::app_theme::AppThemeColors) -> Line<'static> {",
        "%%HELP_HEADING_DEF%%"
    )
    text = text.replace(
        "pub fn help_item_dyn(text: &str, key: Option<&str>, theme: &crate::app_theme::AppThemeColors) -> Vec<Line<'static>> {",
        "%%HELP_ITEM_DYN_DEF%%"
    )
    
    text = re.sub(r'help_heading\(([^,]+),\s*([^)]+)\)', r'help_heading(\1, \2, theme)', text)
    
    text = re.sub(r'(help_item_dyn\([^,]+,\s*None)\)', r'\1, theme)', text)
    text = re.sub(r'(help_item_dyn\([^,]+,\s*None),\n\s*\)\);', r'\1, theme\n    ));', text)
    text = re.sub(r'(help_item_dyn\([^,]+,\s*Some\([^)]+\))\)', r'\1, theme)', text)
    text = re.sub(r'(help_item_dyn\([^,]+,\s*Some\([^)]+\)),\n\s*\)\);', r'\1, theme\n    ));', text)

    text = re.sub(r'(Some\([^)]+\)),\n(\s*)\)\);', r'\1,\n\2    theme,\n\2));', text)
    text = re.sub(r'(None),\n(\s*)\)\);', r'\1,\n\2    theme,\n\2));', text)
    
    text = text.replace("%%HELP_HEADING_DEF%%", "pub fn help_heading(icon: &'static str, title: &'static str, theme: &crate::app_theme::AppThemeColors) -> Line<'static> {")
    text = text.replace("%%HELP_ITEM_DYN_DEF%%", "pub fn help_item_dyn(text: &str, key: Option<&str>, theme: &crate::app_theme::AppThemeColors) -> Vec<Line<'static>> {")

    # 3. Update calls to draw_*_popup
    text = text.replace(
        "pub fn draw_template_popup(frame: &mut Frame, popup: &TemplatePopup, area: Rect, theme: &crate::app_theme::AppThemeColors) {",
        "%%DRAW_TEMPLATE_POPUP_DEF%%"
    )
    text = text.replace(
        "pub fn draw_confirm_popup(frame: &mut Frame, popup: &ConfirmPopup, area: Rect, theme: &crate::app_theme::AppThemeColors) {",
        "%%DRAW_CONFIRM_POPUP_DEF%%"
    )
    
    text = re.sub(r'draw_template_popup\(([^,]+),\s*([^,]+),\s*([^)]+)\)', r'draw_template_popup(\1, \2, \3, &app.app_theme)', text)
    text = re.sub(r'draw_confirm_popup\(([^,]+),\s*([^,]+),\s*([^)]+)\)', r'draw_confirm_popup(\1, \2, \3, &app.app_theme)', text)

    text = text.replace("%%DRAW_TEMPLATE_POPUP_DEF%%", "pub fn draw_template_popup(frame: &mut Frame, popup: &TemplatePopup, area: Rect, theme: &crate::app_theme::AppThemeColors) {")
    text = text.replace("%%DRAW_CONFIRM_POPUP_DEF%%", "pub fn draw_confirm_popup(frame: &mut Frame, popup: &ConfirmPopup, area: Rect, theme: &crate::app_theme::AppThemeColors) {")

    # 4. Replace Color::* with __THEME__.*
    text = text.replace("Color::Cyan", "__THEME__.accent")
    text = text.replace("Color::Green", "__THEME__.success")
    text = text.replace("Color::Red", "__THEME__.destructive")
    text = text.replace("Color::Blue", "__THEME__.folder")
    text = text.replace("Color::LightMagenta", "__THEME__.tag")
    text = text.replace("Color::DarkGray", "__THEME__.muted")
    text = text.replace("Color::White", "__THEME__.fg")
    text = text.replace(".fg(Color::Black)", ".fg(__THEME__.highlight_fg)")
    text = text.replace(".bg(__THEME__.accent)", ".bg(__THEME__.highlight_bg)") 
    text = text.replace("Color::Yellow", "__THEME__.heading")

    lines = text.split('\n')
    out_lines = []
    current_theme_var = "theme"
    
    for line in lines:
        if line.startswith("pub fn ") or line.startswith("fn "):
            if "app: &mut App" in line or "app: &App" in line:
                current_theme_var = "app.app_theme"
            elif "theme: &" in line:
                current_theme_var = "theme"
        line = line.replace("__THEME__", current_theme_var)
        out_lines.append(line)

    text = '\n'.join(out_lines)

    with open('src/ui.rs', 'w') as f:
        f.write(text)

process_file()
