use ratatui::{
    crossterm::event::KeyModifiers,
    style::Styled,
    text::{Line, Text},
    widgets::Clear,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    autofill::AutoFillFn,
    wrap::{compute_character_width, wrap},
};

use super::*;

#[derive(Default)]
pub struct EditableText {
    autofill_func: Option<AutoFillFn>,
    autofill_text: Option<String>,
    chars: Vec<char>,
    cursor_offset: u16,
    focused: bool,
    insert_ind: usize,
}

impl EditableText {
    pub fn new(base_content: &str, autofill_func: Option<AutoFillFn>) -> Self {
        // input begins with base_content
        let chars: Vec<char> = base_content.chars().collect();
        let insert_ind = chars.len();
        Self {
            autofill_func,
            autofill_text: None,
            chars,
            cursor_offset: base_content.width() as u16,
            focused: false,
            insert_ind,
        }
    }

    /// Collects the stored collection of UTF-32 characters into a UTF-8 String
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Returns true if there are no UTF-32 characters present in the input
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn toggle_focus(&mut self) {
        self.focused = !self.focused;
        self.autofill_text = None;
    }

    pub fn render_with_style<S: Into<Style>>(
        &mut self,
        f: &mut Frame,
        rect: Rect,
        _block: Block,
        style: S,
    ) {
        // clear previous text off the screen
        f.render_widget(Clear, rect);

        // get the lines of text to display and wrap them in the current rect
        let content = self.text();
        let mut lines = wrap(&content, rect.width);

        // update the cursor position and other things required when focusing
        if self.focused {
            let line_widths = compute_line_widths(lines.as_slice());
            // set the cursor to the intended position
            let (rel_x, rel_y) =
                compute_cursor_position(self.cursor_offset, line_widths.as_slice());
            if let Some((x, y)) = cursor_within_rect(rel_x, rel_y, rect.width, rect.height) {
                f.set_cursor_position((x + rect.x, y + rect.y));
            }
            if let Some(autofill) = &self.autofill_text {
                if lines.is_empty() {
                    // simply wrap and render the autofill content
                    let autofill = wrap(autofill, rect.width);
                    f.render_widget(
                        Text::from_iter(autofill)
                            .style(Style::new().fg(DEFAULT_APP_COLORS.selection_one_bg)),
                        rect,
                    );
                } else {
                    let final_line = lines.pop().unwrap();
                    let combined = format!("{}{}", final_line, autofill);
                    let autofill_lines = wrap(&combined, rect.width);
                    let (orig, auto) = autofill_lines[0].split_at(final_line.len());
                    let style: Style = style.into();
                    let line = Line::from(vec![
                        orig.set_style(style),
                        auto.set_style(style.fg(DEFAULT_APP_COLORS.selection_one_bg)),
                    ]);
                    f.render_widget(
                        Text::from_iter(
                            lines
                                .into_iter()
                                .map(|s| Line::from(s).style(style))
                                .chain(std::iter::once(line))
                                .chain(autofill_lines.iter().skip(1).map(|s| {
                                    Line::from(s.clone())
                                        .style(style.fg(DEFAULT_APP_COLORS.selection_one_bg))
                                })),
                        )
                        .style(style),
                        rect,
                    );
                }
                // don't allow further rendering as it would overwrite this change
                return;
            }
        }
        f.render_widget(Text::from_iter(lines).style(style), rect);
    }
}

impl Component for EditableText {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        // ignore key releases
        if key.kind == KeyEventKind::Release {
            return Ok(vec![Action::Noop]);
        }

        match key {
            // as shift+enter doesn't work, ALT+\ is the key combo used for newlines
            KeyEvent {
                code: KeyCode::Char('\\'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                let c = '\n';
                self.chars.insert(self.insert_ind, c);
                self.insert_ind += 1;
                self.cursor_offset += 1;
                return Ok(vec![Action::Noop]);
            }
            // have ctrl+space set the autofill suggestion string
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.autofill_text = if let Some(func) = &self.autofill_func {
                    let text = self.text();
                    func(text.as_str())
                } else {
                    None
                };
                return Ok(vec![Action::Noop]);
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(autofill) = self.autofill_text.take() {
                    // accept the autofill suggestion
                    self.chars.extend(autofill.chars());
                    self.cursor_offset += autofill.width() as u16;
                    self.insert_ind = self.chars.len();
                }
                return Ok(vec![Action::Noop]);
            }
            _ => {}
        }

        match key.code {
            KeyCode::Char(c) => {
                self.chars.insert(self.insert_ind, c);
                self.insert_ind += 1;
                self.cursor_offset += compute_character_width(c);
                // hide the autofill suggestion
                self.autofill_text = None;
            }
            KeyCode::Backspace | KeyCode::Delete => {
                if !self.chars.is_empty() && self.insert_ind > 0 {
                    let c = self.chars.remove(self.insert_ind - 1);
                    self.insert_ind -= 1;
                    self.cursor_offset -= if c == '\n' {
                        1
                    } else {
                        compute_character_width(c)
                    };
                    // hide the autofill suggestion
                    self.autofill_text = None;
                }
            }
            KeyCode::Left => {
                if !self.chars.is_empty() && self.insert_ind > 0 {
                    self.insert_ind -= 1;
                    let c = self.chars[self.insert_ind];
                    self.cursor_offset = if c == '\n' {
                        self.cursor_offset.saturating_sub(1)
                    } else {
                        self.cursor_offset
                            .saturating_sub(compute_character_width(c))
                    };
                }
            }
            KeyCode::Right => {
                if self.insert_ind < self.chars.len() {
                    let c = self.chars[self.insert_ind];
                    self.insert_ind += 1;
                    self.cursor_offset += if c == '\n' {
                        1
                    } else {
                        compute_character_width(c)
                    };
                }
            }
            _ => {}
        }
        Ok(vec![Action::Noop])
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        self.render_with_style(
            f,
            rect,
            block,
            Style::new()
                .fg(DEFAULT_APP_COLORS.main_fg)
                .bg(DEFAULT_APP_COLORS.main_bg),
        )
    }
}

impl From<&str> for EditableText {
    fn from(value: &str) -> Self {
        Self::new(value, None)
    }
}
