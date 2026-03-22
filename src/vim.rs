use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Color, Style};
use tui_textarea::{CursorMove, Input, Key, TextArea};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Operator(char),           // 'd', 'c', 'y'
    InnerObjectPending(char), // 'd', 'c', 'y' pending inner object like 'i'
    Command,
}

impl VimMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "VISUAL LINE",
            Self::Operator(_) => "OPERATOR",
            Self::InnerObjectPending(_) => "INNER",
            Self::Command => "COMMAND",
        }
    }

    pub fn cursor_style(&self) -> Style {
        let bg = match self {
            Self::Insert => Color::Cyan,
            Self::Normal => Color::Yellow,
            Self::Visual | Self::VisualLine => Color::Green,
            _ => Color::Magenta,
        };
        Style::default().fg(Color::Black).bg(bg)
    }
}

pub enum VimOutput {
    Handled,
    Unhandled,
    Command(String),
}

pub struct Vim {
    pub mode: VimMode,
    pub command_buffer: String,
    pub count_buffer: String,
    pub pending_g: bool,
    visual_line_anchor: Option<u16>,
}

impl Vim {
    pub fn new() -> Self {
        Self {
            mode: VimMode::Normal,
            command_buffer: String::new(),
            count_buffer: String::new(),
            pending_g: false,
            visual_line_anchor: None,
        }
    }

    pub fn reset(&mut self) {
        self.mode = VimMode::Normal;
        self.command_buffer.clear();
        self.count_buffer.clear();
        self.pending_g = false;
        self.visual_line_anchor = None;
    }

    fn enter_normal(&mut self, textarea: &mut TextArea<'_>) {
        self.mode = VimMode::Normal;
        textarea.cancel_selection();
        self.count_buffer.clear();
        self.pending_g = false;
        self.visual_line_anchor = None;
    }

    pub fn transition(&mut self, key: KeyEvent, textarea: &mut TextArea<'_>) -> VimOutput {
        let input: Input = key.into();
        if input.key == Key::Null {
            return VimOutput::Unhandled;
        }

        // Always allow Esc to return to normal mode
        if key.code == KeyCode::Esc {
            if self.mode == VimMode::Insert {
                textarea.move_cursor(CursorMove::Back);
            }
            self.enter_normal(textarea);
            textarea.set_cursor_style(self.mode.cursor_style());
            return VimOutput::Handled;
        }

        let handled = match self.mode {
            VimMode::Normal => self.handle_normal(input, textarea),
            VimMode::Insert => self.handle_insert(input, textarea),
            VimMode::Visual => self.handle_visual(input, textarea),
            VimMode::VisualLine => self.handle_visual_line(input, textarea),
            VimMode::Operator(op) => self.handle_operator(input, op, textarea),
            VimMode::InnerObjectPending(op) => self.handle_inner_object(input, op, textarea),
            VimMode::Command => self.handle_command(input, textarea),
        };

        if let VimOutput::Unhandled = handled {
        } else {
            textarea.set_cursor_style(self.mode.cursor_style());
        }
        handled
    }

    fn handle_normal(&mut self, input: Input, textarea: &mut TextArea<'_>) -> VimOutput {
        if self.pending_g {
            self.pending_g = false;
            match input {
                Input {
                    key: Key::Char('g'),
                    ..
                } => {
                    let target_line = if self.count_buffer.is_empty() {
                        0
                    } else {
                        let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                        count.saturating_sub(1)
                    };
                    self.count_buffer.clear();
                    textarea.cancel_selection();
                    textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                    return VimOutput::Handled;
                }
                _ => {
                    self.count_buffer.clear();
                    return VimOutput::Handled;
                }
            }
        }

        if let Input {
            key: Key::Char(c @ '0'..='9'),
            ..
        } = input
            && (c != '0' || !self.count_buffer.is_empty())
        {
            self.count_buffer.push(c);
            return VimOutput::Handled;
        }

        let _count = if self.count_buffer.is_empty() {
            1
        } else {
            self.count_buffer.parse::<usize>().unwrap_or(1)
        };

        let _count = if self.count_buffer.is_empty() {
            1
        } else {
            self.count_buffer.parse::<usize>().unwrap_or(1)
        };

        match input {
            Input {
                key: Key::Char('h'),
                ..
            }
            | Input { key: Key::Left, .. } => textarea.move_cursor(CursorMove::Back),
            Input {
                key: Key::Char('j'),
                ..
            }
            | Input { key: Key::Down, .. } => textarea.move_cursor(CursorMove::Down),
            Input {
                key: Key::Char('k'),
                ..
            }
            | Input { key: Key::Up, .. } => textarea.move_cursor(CursorMove::Up),
            Input {
                key: Key::Char('l'),
                ..
            }
            | Input {
                key: Key::Right, ..
            } => textarea.move_cursor(CursorMove::Forward),
            Input {
                key: Key::PageDown, ..
            }
            | Input {
                key: Key::PageUp, ..
            } => {
                textarea.cancel_selection();
                textarea.input(input);
                return VimOutput::Handled;
            }
            Input {
                key: Key::Char('d'),
                ctrl: true,
                ..
            } => {
                textarea.cancel_selection();
                for _ in 0..20 {
                    textarea.move_cursor(CursorMove::Down);
                }
            }
            Input {
                key: Key::Char('u'),
                ctrl: true,
                ..
            } => {
                textarea.cancel_selection();
                for _ in 0..20 {
                    textarea.move_cursor(CursorMove::Up);
                }
            }
            Input { key: Key::Home, .. } => textarea.move_cursor(CursorMove::Head),
            Input { key: Key::End, .. } => textarea.move_cursor(CursorMove::End),
            Input {
                key: Key::Char('w'),
                ..
            } => textarea.move_cursor(CursorMove::WordForward),
            Input {
                key: Key::Char('b'),
                ..
            } => textarea.move_cursor(CursorMove::WordBack),
            Input {
                key: Key::Char('e'),
                ..
            } => textarea.move_cursor(CursorMove::WordEnd),
            Input {
                key: Key::Char('0'),
                ..
            }
            | Input {
                key: Key::Char('^'),
                ..
            } => textarea.move_cursor(CursorMove::Head),
            Input {
                key: Key::Char('$'),
                ..
            } => textarea.move_cursor(CursorMove::End),
            Input {
                key: Key::Char('i'),
                ..
            } => {
                self.mode = VimMode::Insert;
            }
            Input {
                key: Key::Char('G'),
                ..
            } => {
                let target_line = if self.count_buffer.is_empty() {
                    textarea.lines().len().saturating_sub(1)
                } else {
                    let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                    count.saturating_sub(1)
                };
                self.count_buffer.clear();
                textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
            }
            Input {
                key: Key::Char('g'),
                ..
            } => {
                if !self.count_buffer.is_empty() {
                    let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                    let target_line = count.saturating_sub(1);
                    self.count_buffer.clear();
                    textarea.cancel_selection();
                    textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                } else {
                    self.pending_g = true;
                    return VimOutput::Handled;
                }
            }
            Input {
                key: Key::Char('I'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Head);
                self.mode = VimMode::Insert;
            }
            Input {
                key: Key::Char('a'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Forward);
                self.mode = VimMode::Insert;
            }
            Input {
                key: Key::Char('A'),
                ..
            } => {
                textarea.move_cursor(CursorMove::End);
                self.mode = VimMode::Insert;
            }
            Input {
                key: Key::Char('o'),
                ..
            } => {
                textarea.move_cursor(CursorMove::End);
                textarea.insert_newline();
                self.mode = VimMode::Insert;
            }
            Input {
                key: Key::Char('O'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Head);
                textarea.insert_newline();
                textarea.move_cursor(CursorMove::Up);
                self.mode = VimMode::Insert;
            }
            Input {
                key: Key::Char('v'),
                ..
            } => {
                textarea.start_selection();
                self.mode = VimMode::Visual;
            }
            Input {
                key: Key::Char('V'),
                ..
            } => {
                let anchor = textarea.cursor().0 as u16;
                self.visual_line_anchor = Some(anchor);
                self.sync_visual_line(textarea, anchor);
                self.mode = VimMode::VisualLine;
            }
            Input {
                key: Key::Char('p'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.paste();
            }
            Input {
                key: Key::Char('P'),
                ..
            } => {
                textarea.paste();
            }
            Input {
                key: Key::Char('u'),
                ..
            } => {
                textarea.undo();
            }
            Input {
                key: Key::Char('r'),
                ctrl: true,
                ..
            } => {
                textarea.redo();
            }
            Input {
                key: Key::Char('x'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.delete_char();
            }
            Input {
                key: Key::Char('D'),
                ..
            }
            | Input {
                key: Key::Char('C'),
                ..
            } => {
                textarea.delete_line_by_end();
                if input.key == Key::Char('C') {
                    self.mode = VimMode::Insert;
                }
            }
            Input {
                key: Key::Char(op @ ('y' | 'd' | 'c')),
                ..
            } => {
                self.mode = VimMode::Operator(op);
            }
            Input {
                key: Key::Char(':'),
                ..
            } => {
                self.mode = VimMode::Command;
                self.command_buffer.clear();
                self.command_buffer.push(':');
                return VimOutput::Handled;
            }
            _ => return VimOutput::Handled,
        }
        self.count_buffer.clear();
        self.count_buffer.clear();
        VimOutput::Handled
    }

    fn handle_insert(&mut self, _input: Input, _textarea: &mut TextArea<'_>) -> VimOutput {
        // Handled by main.rs native textarea.input() unless Esc
        VimOutput::Unhandled
    }

    fn handle_command(&mut self, input: Input, _textarea: &mut TextArea<'_>) -> VimOutput {
        match input.key {
            Key::Char('w') if input.ctrl => {
                self.command_buffer.pop();
            }
            Key::Char('c') if input.ctrl => {
                self.mode = VimMode::Normal;
                self.command_buffer.clear();
            }
            Key::Char(c) => self.command_buffer.push(c),
            Key::Backspace => {
                self.command_buffer.pop();
                if self.command_buffer.is_empty() {
                    self.mode = VimMode::Normal;
                }
            }
            Key::Enter => {
                let cmd = self.command_buffer.clone();
                self.mode = VimMode::Normal;
                self.command_buffer.clear();
                return VimOutput::Command(cmd);
            }
            _ => {}
        }
        VimOutput::Handled
    }

    fn handle_visual(&mut self, input: Input, textarea: &mut TextArea<'_>) -> VimOutput {
        if self.pending_g {
            self.pending_g = false;
            match input {
                Input {
                    key: Key::Char('g'),
                    ..
                } => {
                    let target_line = if self.count_buffer.is_empty() {
                        0
                    } else {
                        let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                        count.saturating_sub(1)
                    };
                    self.count_buffer.clear();
                    // Do NOT cancel selection in visual mode!
                    textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                    return VimOutput::Handled;
                }
                _ => {
                    self.count_buffer.clear();
                    return VimOutput::Handled;
                }
            }
        }

        if let Input {
            key: Key::Char(c @ '0'..='9'),
            ..
        } = input
            && (c != '0' || !self.count_buffer.is_empty())
        {
            self.count_buffer.push(c);
            return VimOutput::Handled;
        }

        let _count = if self.count_buffer.is_empty() {
            1
        } else {
            self.count_buffer.parse::<usize>().unwrap_or(1)
        };

        let _count = if self.count_buffer.is_empty() {
            1
        } else {
            self.count_buffer.parse::<usize>().unwrap_or(1)
        };

        match input {
            Input {
                key: Key::Char('h'),
                ..
            }
            | Input { key: Key::Left, .. } => textarea.move_cursor(CursorMove::Back),
            Input {
                key: Key::Char('j'),
                ..
            }
            | Input { key: Key::Down, .. } => textarea.move_cursor(CursorMove::Down),
            Input {
                key: Key::Char('k'),
                ..
            }
            | Input { key: Key::Up, .. } => textarea.move_cursor(CursorMove::Up),
            Input {
                key: Key::Char('l'),
                ..
            }
            | Input {
                key: Key::Right, ..
            } => textarea.move_cursor(CursorMove::Forward),
            Input {
                key: Key::PageDown, ..
            }
            | Input {
                key: Key::PageUp, ..
            } => {
                textarea.input(input);
                return VimOutput::Handled;
            }
            Input {
                key: Key::Char('d'),
                ctrl: true,
                ..
            } => {
                for _ in 0..20 {
                    textarea.move_cursor(CursorMove::Down);
                }
            }
            Input {
                key: Key::Char('u'),
                ctrl: true,
                ..
            } => {
                for _ in 0..20 {
                    textarea.move_cursor(CursorMove::Up);
                }
            }
            Input { key: Key::Home, .. } => textarea.move_cursor(CursorMove::Head),
            Input { key: Key::End, .. } => textarea.move_cursor(CursorMove::End),
            Input {
                key: Key::Char('w'),
                ..
            } => textarea.move_cursor(CursorMove::WordForward),
            Input {
                key: Key::Char('b'),
                ..
            } => textarea.move_cursor(CursorMove::WordBack),
            Input {
                key: Key::Char('e'),
                ..
            } => textarea.move_cursor(CursorMove::WordEnd),
            Input {
                key: Key::Char('0'),
                ..
            }
            | Input {
                key: Key::Char('^'),
                ..
            } => textarea.move_cursor(CursorMove::Head),
            Input {
                key: Key::Char('$'),
                ..
            } => textarea.move_cursor(CursorMove::End),
            Input {
                key: Key::Char('G'),
                ..
            } => {
                let target_line = if self.count_buffer.is_empty() {
                    textarea.lines().len().saturating_sub(1)
                } else {
                    let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                    count.saturating_sub(1)
                };
                self.count_buffer.clear();
                textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
            }
            Input {
                key: Key::Char('g'),
                ..
            } => {
                if !self.count_buffer.is_empty() {
                    let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                    let target_line = count.saturating_sub(1);
                    self.count_buffer.clear();
                    textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                } else {
                    self.pending_g = true;
                    return VimOutput::Handled;
                }
            }
            Input {
                key: Key::Char('y'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.copy();
                self.enter_normal(textarea);
            }
            Input {
                key: Key::Char('d'),
                ..
            }
            | Input {
                key: Key::Char('x'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.cut();
                self.enter_normal(textarea);
            }
            Input {
                key: Key::Char('c'),
                ..
            } => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.cut();
                self.mode = VimMode::Insert;
            }
            _ => return VimOutput::Handled,
        }
        self.count_buffer.clear();
        self.count_buffer.clear();
        VimOutput::Handled
    }

    fn handle_visual_line(&mut self, input: Input, textarea: &mut TextArea<'_>) -> VimOutput {
        if self.pending_g {
            self.pending_g = false;
            match input {
                Input {
                    key: Key::Char('g'),
                    ..
                } => {
                    let target_line = if self.count_buffer.is_empty() {
                        0
                    } else {
                        let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                        count.saturating_sub(1)
                    };
                    self.count_buffer.clear();
                    textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                    let anchor_row = self
                        .visual_line_anchor
                        .unwrap_or(textarea.cursor().0 as u16);
                    self.sync_visual_line(textarea, anchor_row);
                    return VimOutput::Handled;
                }
                _ => {
                    self.count_buffer.clear();
                    return VimOutput::Handled;
                }
            }
        }

        if let Input {
            key: Key::Char(c @ '0'..='9'),
            ..
        } = input
            && (c != '0' || !self.count_buffer.is_empty())
        {
            self.count_buffer.push(c);
            return VimOutput::Handled;
        }

        let _count = if self.count_buffer.is_empty() {
            1
        } else {
            self.count_buffer.parse::<usize>().unwrap_or(1)
        };

        let _count = if self.count_buffer.is_empty() {
            1
        } else {
            self.count_buffer.parse::<usize>().unwrap_or(1)
        };

        let _anchor = self
            .visual_line_anchor
            .unwrap_or(textarea.cursor().0 as u16);
        match input {
            Input {
                key: Key::Char('j'),
                ..
            }
            | Input { key: Key::Down, .. } => {
                textarea.move_cursor(CursorMove::Down);
                let anchor_row = self
                    .visual_line_anchor
                    .unwrap_or(textarea.cursor().0 as u16);
                self.sync_visual_line(textarea, anchor_row);
            }
            Input {
                key: Key::Char('k'),
                ..
            }
            | Input { key: Key::Up, .. } => {
                textarea.move_cursor(CursorMove::Up);
                let anchor_row = self
                    .visual_line_anchor
                    .unwrap_or(textarea.cursor().0 as u16);
                self.sync_visual_line(textarea, anchor_row);
            }
            Input {
                key: Key::PageDown, ..
            }
            | Input {
                key: Key::PageUp, ..
            } => {
                textarea.input(input);
                let anchor_row = self
                    .visual_line_anchor
                    .unwrap_or(textarea.cursor().0 as u16);
                self.sync_visual_line(textarea, anchor_row);
                return VimOutput::Handled;
            }
            Input {
                key: Key::Char('d'),
                ctrl: true,
                ..
            } => {
                for _ in 0..20 {
                    textarea.move_cursor(CursorMove::Down);
                }
                let anchor_row = self
                    .visual_line_anchor
                    .unwrap_or(textarea.cursor().0 as u16);
                self.sync_visual_line(textarea, anchor_row);
            }
            Input {
                key: Key::Char('u'),
                ctrl: true,
                ..
            } => {
                for _ in 0..20 {
                    textarea.move_cursor(CursorMove::Up);
                }
                let anchor_row = self
                    .visual_line_anchor
                    .unwrap_or(textarea.cursor().0 as u16);
                self.sync_visual_line(textarea, anchor_row);
            }
            Input {
                key: Key::Char('G'),
                ..
            } => {
                let target_line = if self.count_buffer.is_empty() {
                    textarea.lines().len().saturating_sub(1)
                } else {
                    let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                    count.saturating_sub(1)
                };
                self.count_buffer.clear();
                textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                let anchor_row = self
                    .visual_line_anchor
                    .unwrap_or(textarea.cursor().0 as u16);
                self.sync_visual_line(textarea, anchor_row);
            }
            Input {
                key: Key::Char('g'),
                ..
            } => {
                if !self.count_buffer.is_empty() {
                    let count = self.count_buffer.parse::<usize>().unwrap_or(1);
                    let target_line = count.saturating_sub(1);
                    self.count_buffer.clear();
                    textarea.move_cursor(tui_textarea::CursorMove::Jump(target_line as u16, 0));
                    let anchor_row = self
                        .visual_line_anchor
                        .unwrap_or(textarea.cursor().0 as u16);
                    self.sync_visual_line(textarea, anchor_row);
                } else {
                    self.pending_g = true;
                    return VimOutput::Handled;
                }
            }
            Input {
                key: Key::Char('y'),
                ..
            } => {
                textarea.copy();
                self.enter_normal(textarea);
            }
            Input {
                key: Key::Char('d'),
                ..
            }
            | Input {
                key: Key::Char('x'),
                ..
            } => {
                textarea.cut();
                self.enter_normal(textarea);
            }
            Input {
                key: Key::Char('c'),
                ..
            } => {
                textarea.cut();
                self.mode = VimMode::Insert;
            }
            _ => return VimOutput::Handled,
        }
        self.count_buffer.clear();
        self.count_buffer.clear();
        VimOutput::Handled
    }

    fn handle_operator(
        &mut self,
        input: Input,
        op: char,
        textarea: &mut TextArea<'_>,
    ) -> VimOutput {
        if let Input {
            key: Key::Char('i'),
            ..
        } = input
        {
            self.mode = VimMode::InnerObjectPending(op);
            return VimOutput::Handled;
        }

        if let Input {
            key: Key::Char(c), ..
        } = input
        {
            if c == op {
                // Line operation: yy, dd, cc
                textarea.move_cursor(CursorMove::Head);
                textarea.start_selection();
                let initial = textarea.cursor();
                textarea.move_cursor(CursorMove::Down);
                if initial == textarea.cursor() {
                    textarea.move_cursor(CursorMove::End);
                }
                match op {
                    'y' => {
                        textarea.copy();
                        self.enter_normal(textarea);
                    }
                    'd' => {
                        textarea.cut();
                        self.enter_normal(textarea);
                    }
                    'c' => {
                        textarea.cut();
                        self.mode = VimMode::Insert;
                    }
                    _ => self.enter_normal(textarea),
                }
                return VimOutput::Handled;
            }

            textarea.start_selection();
            let moved = match c {
                'h' => {
                    textarea.move_cursor(CursorMove::Back);
                    true
                }
                'j' => {
                    textarea.move_cursor(CursorMove::Down);
                    true
                }
                'k' => {
                    textarea.move_cursor(CursorMove::Up);
                    true
                }
                'l' => {
                    textarea.move_cursor(CursorMove::Forward);
                    true
                }
                'w' => {
                    textarea.move_cursor(CursorMove::WordForward);
                    true
                }
                'b' => {
                    textarea.move_cursor(CursorMove::WordBack);
                    true
                }
                'e' => {
                    textarea.move_cursor(CursorMove::WordEnd);
                    textarea.move_cursor(CursorMove::Forward);
                    true
                }
                '0' | '^' => {
                    textarea.move_cursor(CursorMove::Head);
                    true
                }
                '$' => {
                    textarea.move_cursor(CursorMove::End);
                    true
                }
                _ => false,
            };

            if !moved {
                self.enter_normal(textarea);
                return VimOutput::Handled;
            }

            match op {
                'y' => {
                    textarea.copy();
                    self.enter_normal(textarea);
                }
                'd' => {
                    textarea.cut();
                    self.enter_normal(textarea);
                }
                'c' => {
                    textarea.cut();
                    self.mode = VimMode::Insert;
                }
                _ => self.enter_normal(textarea),
            }
            return VimOutput::Handled;
        }

        self.enter_normal(textarea);
        VimOutput::Handled
    }

    fn handle_inner_object(
        &mut self,
        input: Input,
        op: char,
        textarea: &mut TextArea<'_>,
    ) -> VimOutput {
        let Input {
            key: Key::Char(obj),
            ..
        } = input
        else {
            self.enter_normal(textarea);
            return VimOutput::Handled;
        };

        let text = textarea.lines().join("\n");
        let cursor =
            crate::helpers::cursor_to_offset(&text, textarea.cursor().0, textarea.cursor().1);

        let range = match obj {
            'w' => crate::helpers::inner_word_range(&text, cursor),
            '(' | ')' | 'b' => crate::helpers::inner_pair_range(&text, cursor, '(', ')'),
            '[' | ']' => crate::helpers::inner_pair_range(&text, cursor, '[', ']'),
            '{' | '}' | 'B' => crate::helpers::inner_pair_range(&text, cursor, '{', '}'),
            '<' | '>' => crate::helpers::inner_pair_range(&text, cursor, '<', '>'),
            '"' => crate::helpers::inner_quote_range(&text, cursor, '"'),
            '\'' => crate::helpers::inner_quote_range(&text, cursor, '\''),
            '`' => crate::helpers::inner_quote_range(&text, cursor, '`'),
            _ => None,
        };

        if let Some((start, end)) = range {
            let (start_row, start_col) =
                crate::helpers::offset_to_cursor(textarea.lines(), start.min(text.len()));
            let (end_row, end_col) =
                crate::helpers::offset_to_cursor(textarea.lines(), end.min(text.len()));

            textarea.cancel_selection();
            textarea.move_cursor(CursorMove::Jump(start_row as u16, start_col as u16));
            textarea.start_selection();
            textarea.move_cursor(CursorMove::Jump(end_row as u16, end_col as u16));

            match op {
                'y' => {
                    textarea.copy();
                    self.enter_normal(textarea);
                }
                'd' => {
                    textarea.cut();
                    self.enter_normal(textarea);
                }
                'c' => {
                    textarea.cut();
                    self.mode = VimMode::Insert;
                }
                _ => self.enter_normal(textarea),
            }
            return VimOutput::Handled;
        }

        self.enter_normal(textarea);
        VimOutput::Handled
    }

    fn sync_visual_line(&mut self, textarea: &mut TextArea<'_>, anchor_row: u16) {
        let current_row = textarea.cursor().0 as u16;
        textarea.cancel_selection();

        if current_row >= anchor_row {
            textarea.move_cursor(CursorMove::Jump(anchor_row, 0));
            textarea.start_selection();
            textarea.move_cursor(CursorMove::Jump(current_row, 0));
            textarea.move_cursor(CursorMove::End);
        } else {
            textarea.move_cursor(CursorMove::Jump(anchor_row, 0));
            textarea.move_cursor(CursorMove::End);
            textarea.start_selection();
            textarea.move_cursor(CursorMove::Jump(current_row, 0));
        }
    }
}
