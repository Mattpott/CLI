use std::error::Error;

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    prelude::Backend,
    Frame, Terminal,
};

use crate::{
    action::Action,
    component::{database_component::DatabaseComp, selected_table::TableSelection, Component},
    config::DEFAULT_APP_COLORS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum FocusArea {
    Tables,
    Main,
}

/// The collection of state which the app runs off of
pub struct App {
    database_component: DatabaseComp,
    focusing: FocusArea,
    tables_component: TableSelection,
}

impl App {
    /// Constructs the default app state for the CLI
    pub fn new() -> Result<App, Box<dyn Error>> {
        let mut app = Self {
            database_component: DatabaseComp::new("", 2, false)?,
            focusing: FocusArea::Tables,
            tables_component: TableSelection::new(),
        };
        if let Some(starting_table) = app.tables_component.selected() {
            app.database_component.change_table_used(starting_table)?;
        }
        Ok(app)
    }

    /// Handles actions which get passed to the app.
    /// Returns true if the app should quit, false otherwise
    fn handle_actions(&mut self, actions: Vec<Action>) -> Result<bool, Box<dyn Error>> {
        // loop over all actions in order
        for action in actions {
            match action {
                Action::Quit => return Ok(true),
                Action::ChangeSelectedTable => {
                    if let Some(table) = self.tables_component.selected() {
                        self.database_component.change_table_used(table)?;
                    }
                }
                Action::Refresh => {
                    self.database_component.refresh()?;
                }
                Action::VeryLoudWrongBuzzer => print!("\x07"),
                _ => {}
            }
        }
        Ok(false)
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
        loop {
            // draw the thing
            terminal.draw(|frame: &mut Frame| self.render(frame))?;

            // poll keypress event with an ~1 frame at ~60fps timeout on
            // encountering an event to prevent infinite blocking, allowing
            // any moving components of the UI to progress
            if !event::poll(std::time::Duration::from_millis(16))? {
                continue;
            }
            if let Event::Key(key) = event::read()? {
                // ignore key releases
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                let actions = match key {
                    KeyEvent {
                        code: KeyCode::Right,
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        // ctrl+right moves the focus to the next component
                        match self.focusing {
                            FocusArea::Tables => {
                                self.database_component.focus_first();
                                self.focusing = FocusArea::Main;
                            }
                            FocusArea::Main => {
                                if self.database_component.next_focus() {
                                    self.focusing = FocusArea::Tables;
                                }
                            }
                        }
                        vec![Action::Noop]
                    }
                    KeyEvent {
                        code: KeyCode::Left,
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        // ctrl+left moves the focus to the prev component
                        match self.focusing {
                            FocusArea::Tables => {
                                self.database_component.focus_last();
                                self.focusing = FocusArea::Main;
                            }
                            FocusArea::Main => {
                                if self.database_component.prev_focus() {
                                    self.focusing = FocusArea::Tables;
                                }
                            }
                        }
                        vec![Action::Noop]
                    }
                    _ => match self.focusing {
                        // pass non-hardcoded key events to focused component
                        FocusArea::Main => self
                            .database_component
                            .handle_event(Action::KeyEvent(key))?,
                        FocusArea::Tables => {
                            self.tables_component.handle_event(Action::KeyEvent(key))?
                        }
                    },
                };
                // handle the actions returned by the focused component
                if self.handle_actions(actions)? {
                    return Ok(());
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        // use the top of the screen for the tables tabs
        let [tables_rect, main_section_rect, ..] = *Layout::default()
            .margin(0)
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(15), // 15% width for the list of tables to edit
                Constraint::Percentage(85), // 85% width for the rest
            ])
            .split(frame.area())
        else {
            panic!("Not enough size to create the necessary rects or something");
        };

        // determine the blocks used by each component depending on focus
        let get_block = |focus: FocusArea| {
            if self.focusing == focus {
                DEFAULT_APP_COLORS.focused_block()
            } else {
                DEFAULT_APP_COLORS.default_block()
            }
        };

        self.tables_component
            .render(frame, tables_rect, get_block(FocusArea::Tables));
        self.database_component
            .render(frame, main_section_rect, get_block(FocusArea::Main));
    }
}
