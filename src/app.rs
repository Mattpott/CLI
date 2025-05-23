use std::error::Error;

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    prelude::Backend,
    Frame, Terminal,
};

use crate::{
    action::Action,
    component::{
        add_component::AddComponent,
        database_component::DatabaseComp,
        edit_command::{EditCommand, EditCommandComp},
        popup::PopUpComponent,
        selected_table::{TableMetadata, TableSelection},
        Component,
    },
    config::DEFAULT_APP_COLORS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum FocusArea {
    Tables,
    Edits,
    Main,
}

impl FocusArea {
    pub fn next(&self) -> FocusArea {
        match self {
            FocusArea::Tables => FocusArea::Edits,
            FocusArea::Edits => FocusArea::Main,
            FocusArea::Main => FocusArea::Tables,
        }
    }

    pub fn prev(&self) -> FocusArea {
        match self {
            FocusArea::Edits => FocusArea::Tables,
            FocusArea::Tables => FocusArea::Main,
            FocusArea::Main => FocusArea::Edits,
        }
    }
}

/// The collection of state which the app runs off of
pub struct App {
    add_component: Option<AddComponent>,
    database_component: DatabaseComp,
    edits_component: EditCommandComp,
    focusing: FocusArea,
    popup: Option<PopUpComponent>,
    tables_component: TableSelection,
}

impl App {
    /// Constructs the default app state for the CLI
    pub fn new() -> Result<App, Box<dyn Error>> {
        let mut proto_app = Self {
            add_component: None,
            database_component: DatabaseComp::new("", 2, false)?,
            edits_component: EditCommandComp::new(Vec::with_capacity(0)),
            focusing: FocusArea::Tables,
            popup: None,
            tables_component: TableSelection::new(),
        };
        if let Some(starting_table) = proto_app.tables_component.selected() {
            App::change_table_used(
                &mut proto_app.edits_component,
                &mut proto_app.database_component,
                starting_table,
            )?;
        }
        Ok(proto_app)
    }

    fn change_table_used(
        edits_comp: &mut EditCommandComp,
        database_comp: &mut DatabaseComp,
        table: &TableMetadata,
    ) -> Result<(), Box<dyn Error>> {
        let commands = table.commands.clone();
        let table_name = table.table_name;
        edits_comp.change_commands(commands);
        database_comp.reset_selections();
        if let Some(command) = edits_comp.selected() {
            database_comp.set_max_selections(command.num_selections());
        }
        database_comp.change_stored_table(table_name)?;
        // select all things initially
        database_comp.refresh()?;
        Ok(())
    }

    fn handle_edit_command_change(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(command) = self.edits_component.selected() {
            match command {
                EditCommand::Add => {
                    if let Some(table) = self.tables_component.selected() {
                        self.add_component = Some(AddComponent::new(table.table_name)?);
                        self.focusing = FocusArea::Main;
                    }
                }
                _ => {
                    // TODO: MAY WANT TO CHANGE THIS SO THAT STATE FROM THE ADD SCREEN IS STORED
                    //       INSTEAD OF DESTROYED WHEN EDIT CHOICES ARE CHANGED
                    self.add_component = None;
                    self.database_component
                        .set_max_selections(command.num_selections());
                    self.database_component
                        .set_selection_type(command.uses_rows());
                }
            }
        }
        Ok(())
    }

    fn handle_selection(&mut self) -> Result<(), Box<dyn Error>> {
        let command = self
            .edits_component
            .selected()
            .expect("Should be unable to change selection without an edit mode selected");
        match command {
            EditCommand::Delete => {
                // notify database component to delete and handle returned actions
                let actions = self.database_component.delete()?;
                match self.handle_actions(actions) {
                    Ok(true) => panic!("Handling a deletion should never result in a quit action"),
                    _ => Ok(()),
                }
            }
            _ => Ok(()), // do nothing for most
        }
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
                        App::change_table_used(
                            &mut self.edits_component,
                            &mut self.database_component,
                            table,
                        )?;
                    }
                }
                Action::ChangeEditCommand => self.handle_edit_command_change()?,
                Action::Refresh => {
                    if self.add_component.is_none() {
                        self.database_component.refresh()?;
                    }
                }
                Action::RevertEditHighlight => self.edits_component.highlight_current_selection(),
                Action::RevertEditSelection => self.edits_component.revert_selection(),
                Action::RevertToMain => {
                    // TODO: MAY WANT TO CHANGE THIS SO THAT STATE FROM THE ADD SCREEN IS STORED
                    //       INSTEAD OF DESTROYED WHEN EDIT CHOICES ARE CHANGED
                    self.add_component = None;
                }
                Action::SelectionChanged => self.handle_selection()?,
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
                        self.focusing = self.focusing.next();
                        vec![Action::Noop]
                    }
                    KeyEvent {
                        code: KeyCode::Left,
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        // ctrl+left moves the focus to the prev component
                        self.focusing = self.focusing.prev();
                        vec![Action::Noop]
                    }
                    _ => match self.focusing {
                        // pass non-hardcoded key events to focused component
                        FocusArea::Edits => {
                            self.edits_component.handle_event(Action::KeyEvent(key))?
                        }
                        FocusArea::Main => {
                            if let Some(add_component) = &mut self.add_component {
                                add_component.handle_event(Action::KeyEvent(key))?
                            } else {
                                self.database_component
                                    .handle_event(Action::KeyEvent(key))?
                            }
                        }
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
        let [edits_rect, db_rect, ..] = *Layout::default()
            .margin(0)
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // 3 pixels of height for the list of commands
                Constraint::Min(7),    // At least 7 pixels of height for the rest
            ])
            .split(main_section_rect)
        else {
            panic!("Not enough size to create the necessary rects or something");
        };

        // let popup_rect = db_rect.inner(Margin {
        //     horizontal: db_rect.width / 5,
        //     vertical: db_rect.height / 5,
        // });

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
        self.edits_component
            .render(frame, edits_rect, get_block(FocusArea::Edits));
        if let Some(add_component) = &mut self.add_component {
            add_component.render(frame, db_rect, get_block(FocusArea::Main));
        } else {
            self.database_component
                .render(frame, db_rect, get_block(FocusArea::Main));
        }

        // render the popup if one exists
        if let Some(popup) = &mut self.popup {
            popup.render(frame, db_rect, DEFAULT_APP_COLORS.default_block());
        }
    }
}
