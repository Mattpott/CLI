// module file for the components folder
// defines the shared component definitions and some basic utility functions

// make all components public to the UI as a barrel file
pub mod add_component;
pub mod database_component;
pub mod edit_command;
pub mod editable_text;
pub mod popup;
pub mod selected_table;
pub mod table_display;

// common imports for the module
use std::{borrow::Cow, error::Error, fmt::Debug};

use ratatui::{
    crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    prelude::Frame,
    style::{Style, Stylize},
    widgets::Block,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    action::{Action, UnhandledActionError},
    config::DEFAULT_APP_COLORS,
};

pub trait Component {
    /// Event handler for the component, should mutate self in response and
    /// potentially bubble up an action for the app to take if needed
    fn handle_event(&mut self, event: Action) -> Result<Vec<Action>, Box<dyn Error>> {
        match event {
            Action::Noop => Ok(vec![Action::Noop]),
            Action::Quit => Ok(vec![Action::Quit]),
            Action::KeyEvent(key_event) => self.handle_key_event(key_event),
            Action::OtherEvent(other_event) => self.handle_other_event(other_event),
            unhandled => Err(Box::new(UnhandledActionError::new(unhandled))),
        }
    }

    fn handle_key_event(&mut self, _key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        Ok(vec![Action::Noop])
    }

    fn handle_other_event(&mut self, _event: Event) -> Result<Vec<Action>, Box<dyn Error>> {
        Ok(vec![Action::Noop])
    }

    // renders the component as needed
    // fn render(&mut self, f: &mut Frame, rect: Rect) {
    //     self.render_with_block(f, rect, DEFAULT_APP_COLORS.default_block());
    // }

    /// Renders the component within the passed [`Rect`] and using the passed [`Block`]
    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block);
}

impl Debug for dyn Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Some component")
    }
}

struct LineWidth(u16, bool);

/// Computes the display length of each line as a vector of u16 indicating
/// the width of each line, including any trailing whitespace that may be
/// truncated by textwrap
#[inline]
fn compute_line_widths(lines: &[Cow<str>]) -> Vec<LineWidth> {
    lines
        .iter()
        .map(|line| LineWidth(line.width() as u16, line.ends_with('\n')))
        .collect()
}

/// Computes the position for the cursor to be at in the form of an (x, y)
/// coordinate pair, where (0, 0) is the top-left corner, depending on the
/// displayed width of each line and the cursor offset
fn compute_cursor_position(cursor_offset: u16, widths: &[LineWidth]) -> (u16, u16) {
    if widths.is_empty() {
        return (0, 0);
    }
    let mut x = cursor_offset;
    let mut y = 0u16;
    let mut i = 0;
    while i < widths.len() {
        let LineWidth(width, _) = widths[i];
        i += 1;
        if x > width {
            y += 1;
            x -= width;
        } else {
            break;
        }
    }
    // if x is at the end of its line and there is either another line after
    // x's in widths or x's line ends on a newline, wrap x to the next line
    let prev_i = i.saturating_sub(1);
    if x == widths[prev_i].0 && (i < widths.len() || widths[prev_i].1) {
        y += 1;
        x = 0;
    }
    (x, y)
}

/// Given a coordinate pair and the width and height of some rectangle, this
/// will return a cursor position that is within the bounds of the rectangle
/// or None if the cursor would extend beyond the bounds of the rectangle
fn cursor_within_rect(x: u16, y: u16, width: u16, height: u16) -> Option<(u16, u16)> {
    if width == 0 || height == 0 {
        return None;
    }
    let new_y = y + (x / width);
    if new_y >= height {
        return None;
    }
    let new_x = x % width;
    Some((new_x, new_y))
}
