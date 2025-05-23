use ratatui::{crossterm::event::KeyModifiers, text::Text};
use unicode_width::UnicodeWidthStr;

use crate::wrap::{compute_character_width, wrap};

use super::*;

#[derive(Default)]
pub struct EditableText {
    chars: Vec<char>,
    cursor_offset: u16,
    focused: bool,
    insert_ind: usize,
}

impl EditableText {
    pub fn new(base_content: &str) -> Self {
        // input begins with base_content
        let chars: Vec<char> = base_content.chars().collect();
        let insert_ind = chars.len();
        Self {
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
    }

    /// Sets the base style of the widget
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// All text rendered by the widget will use this style, unless overridden by [`Block::style`],
    /// [`Row::style`], [`Cell::style`], or the styles of cell's content.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    pub fn render_with_style<S: Into<Style>>(
        &mut self,
        f: &mut Frame,
        rect: Rect,
        block: Block,
        style: S,
    ) {
        // get the lines of text to display and wrap them in the current rect
        let content = self.text();
        let lines = wrap(&content, rect.width);

        // update the cursor position and other things required when focusing
        if self.focused {
            // set the cursor to the intended position
            let (rel_x, rel_y) = compute_cursor_position(
                self.cursor_offset,
                compute_line_widths(lines.as_slice()).as_slice(),
            );
            if let Some((x, y)) = cursor_within_rect(rel_x, rel_y, rect.width, rect.height) {
                f.set_cursor_position((x + rect.x, y + rect.y));
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

        // as shift-enter doesn't work, ALT-\ is the key combo used for newlines
        if let KeyEvent {
            code: KeyCode::Char('\\'),
            modifiers: KeyModifiers::ALT,
            ..
        } = key
        {
            let c = '\n';
            self.chars.insert(self.insert_ind, c);
            self.insert_ind += 1;
            self.cursor_offset += 1;
            return Ok(vec![Action::Noop]);
        }

        match key.code {
            KeyCode::Char(c) => {
                self.chars.insert(self.insert_ind, c);
                self.insert_ind += 1;
                self.cursor_offset += compute_character_width(c);
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
        self.render_with_style(f, rect, block, Style::default())
    }
}
