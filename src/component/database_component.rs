use super::*;
use crate::{
    component::{
        add_component::AddComponent,
        command_list::{CommandListComponent, EditCommand},
        selected_table::TableMetadata,
        table_display::MultiTableSelection,
    },
    connection::{ColumnInfo, Connection},
    value::Value,
};
use editable_text::EditableText;
use table_display::TableDisplay;

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin},
    widgets::Paragraph,
};
use rusqlite::{params_from_iter, types::Value as RsqValue};

#[derive(PartialEq)]
enum FocusArea {
    Commands,
    Editor,
    Table,
}

pub struct DatabaseComp {
    add_component: Option<AddComponent>,
    cell_display: Option<EditableText>,
    column_info: Vec<ColumnInfo>,
    command_list: CommandListComponent,
    connection: Connection,
    focus: FocusArea,
    max_selections: usize,
    query: Option<String>,
    table: Option<TableDisplay>,
    table_name: String,
    uses_rows: bool,
}

impl DatabaseComp {
    /// Creates a new database viewing component with its table data
    /// uninstantiated. To query the table initially,
    /// `BaseDatabaseComponent.filter` must be called.
    pub fn new(
        table_name: &str,
        max_selections: usize,
        uses_rows: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let connection = Connection::new()?;
        Ok(Self {
            add_component: None,
            cell_display: None,
            column_info: Vec::new(),
            command_list: CommandListComponent::new(Vec::new()),
            connection,
            focus: FocusArea::Table,
            max_selections,
            query: None,
            table: None,
            table_name: table_name.to_owned(),
            uses_rows,
        })
    }

    /// Updates the passed components of the app to display the passed table
    /// and its associated edit commands.
    pub fn change_table_used(&mut self, table: &TableMetadata) -> Result<(), Box<dyn Error>> {
        self.command_list.change_commands(table.commands.clone());
        if let Some(table) = &mut self.table {
            table.reset_selections();
            // TODO: MAY WANT TO CHANGE THIS SO THAT STATE FROM THE ADD SCREEN IS STORED
            //       INSTEAD OF DESTROYED WHEN EDIT CHOICES ARE CHANGED
            self.add_component = None;
        }
        if let Some(command) = self.command_list.selected() {
            self.set_max_selections(command.num_selections());
        }
        self.change_stored_table(table.table_name)?;
        // initially there is no filtering query, so just refresh and select all
        self.refresh()?;
        // now that the table is setup, make the reader show cell (0, 0)
        self.update_cell_display();
        Ok(())
    }

    /// Calls the previously stored query again if there is one present,
    /// otherwise simply queries to select all rows from the table
    pub fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
        let (query, selections_opt): (&String, Option<&[MultiTableSelection]>) =
            if let Some(stored_query) = self.query.as_ref() {
                // as refresh is calling the stored query and not a new one
                // we can guarantee that the selections should stay the same
                // as we update selections within any modifying function

                // TODO: DETERMINE HOW I WANT THIS TO BE DONE AS THE ADD COMPONENT
                //       HAS NO NOTION OF WHAT SELECTIONS ARE PRESENT AND SO CANNOT
                //       SHIFT ANY ONES WHICH OCCUR AFTER IT AS OF RIGHT NOW.
                //       MAYBE ADD AN ACTION TO SHIFT THE SELECTIONS WHICH OCCUR AFTER
                //       THE INDEX RETURNED BY THE CALL TO INSERT (doesn't work with ORDER BY)
                // let prev_selections = self.table.as_ref().map(|table| table.selections());
                // (stored_query, prev_selections)
                (stored_query, None)
            } else {
                // reset the query to the default one, and do not carry over selections
                self.query = Some(format!("SELECT * FROM {};", self.table_name));
                (self.query.as_ref().unwrap(), None)
            };
        let mut new_table = TableDisplay::from_table(
            self.connection.query(query, [])?,
            self.uses_rows,
            self.max_selections,
        )?;
        if let Some(selections) = selections_opt {
            // if there are selections to carry over, select each one with the new table
            selections
                .iter()
                .for_each(|selection| new_table.select(*selection));
        }
        self.table = Some(new_table);
        Ok(())
    }

    /// Creates a string denoting the positional arguments which specify
    /// the primary keys for the table in the format of
    ///
    ///     "COL_NAME = ?IND AND COL_NAME = ?IND AND ..."
    ///
    /// alongside the list of Rusqlite Values for the passed row which
    /// may be bound to the positional args in a prepared statement.
    ///
    /// It is an error to call this with no table present
    fn pk_positional_args(&self, row: usize, start_offset: usize) -> (String, Vec<RsqValue>) {
        assert!(
            self.table.is_some(),
            "Attempting to get positional args for a table which doesn't exist"
        );

        let (pos, params): (Vec<String>, Vec<RsqValue>) = self
            .column_info
            .iter()
            .enumerate()
            .filter_map(|(ind, info)| {
                if info.is_primary_key {
                    // as the column name is taken directly from pragma_table_info,
                    // the column should be present within the columns
                    // create positional argument in the form of "COL_NAME = ?IND"
                    Some((
                        format!("{} = ?{}", info.name, ind + start_offset + 1),
                        self.table
                            .as_ref()
                            .unwrap()
                            .table
                            .row_get(row, &info.name)
                            .expect("Somehow pragma_table_info has a bad column name")
                            .into(),
                    ))
                } else {
                    None
                }
            })
            .unzip();
        (pos.join(" AND "), params)
    }

    /// Deletes the currently selected row from the table within the database.
    /// Only works if there is 1 selected row for now.
    /// Returns true if a row was removed, false if not
    fn delete(&mut self) -> Result<bool, Box<dyn Error>> {
        // only allow removal of a row, not a cell
        assert!(self.uses_rows);

        if let Some(table) = &self.table {
            if table.selections().len() != 1 {
                return Ok(false);
            }
            if let MultiTableSelection::Row(row) = table.selections()[0] {
                let (pos, params) = self.pk_positional_args(row, 0);

                // DELETE FROM table WHERE col_name1 = value1 AND col_name2 = value2 LIMIT num;
                let query = format!(
                    // "DELETE FROM {} WHERE {} LIMIT 1;",
                    "DELETE FROM {} WHERE {};",
                    self.table_name, pos
                );
                // TODO: maybe store the response to show as a thingy
                self.connection.delete(&query, params_from_iter(params))?;
                // refresh the database and update the command list
                self.command_list.highlight_current_selection();
                self.refresh()?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Filters the table's retrieved rows depending on the passed filter.
    /// Filters should take the form of "WHERE ..." or "GROUP BY ...",
    /// as those keywords are not included in the default filter.
    /// Passing an empty filter will simply select all rows from the table.
    fn filter(&mut self, filter: &str) -> Result<(), Box<dyn Error>> {
        let query = format!("SELECT * FROM {} {};", self.table_name, filter);
        let table = self.connection.query(&query, [])?;
        // store the expanded_sql query for reuse if possible
        self.query = table.query.clone();
        self.table = Some(TableDisplay::from_table(
            table,
            self.uses_rows,
            self.max_selections,
        )?);
        Ok(())
    }

    /// Updates the currently selected cell to have the value currently stored
    /// in the editor, if that value is valid.
    /// Requires there only be 1 selected cell.
    /// Returns true if the cell was updated, false if not
    fn submit_modify(&mut self) -> Result<bool, Box<dyn Error>> {
        assert!(
            self.table.is_some(),
            "Attempting to modify a table which doesn't exist"
        );
        assert!(
            self.cell_display.is_some(),
            "Trying to submit modification from an editor which doesn't exist"
        );
        let table = self.table.as_ref().unwrap();
        let mut to_update: Option<(usize, usize, Value)> = None;
        match table.selections() {
            [MultiTableSelection::Cell((y, x))] => {
                let (y, x) = (*y, *x);
                let (pos, params) = self.pk_positional_args(y, 1);
                // UPDATE table SET col_name = value WHERE pk_name = pk_val;
                let query = format!(
                    "UPDATE {} SET {} = ?1 WHERE {};",
                    self.table_name,
                    table.columns()[x],
                    pos
                );

                let editor = self.cell_display.as_ref().unwrap();
                if self.column_info[x].is_not_null && editor.is_empty() {
                    // there is a required field that is empty, so don't allow change
                    return Ok(false);
                }
                // validate the column has a proper value
                if let Ok(new_val) =
                    Value::parse_column(&self.column_info[x].data_type, &editor.text())
                {
                    // do nothing if the value wasn't changed
                    if new_val == table.rows()[y][x] {
                        return Ok(true);
                    }
                    self.connection.modify(
                        &query,
                        params_from_iter(std::iter::once((&new_val).into()).chain(params)),
                    )?;
                    to_update = Some((y, x, new_val));
                } else {
                    return Ok(false);
                }
            }
            _ => panic!("Trying to edit a whole row or multiple cells at once"),
        }

        // update the content of the stored cell instead of refreshing the whole table
        let table = self.table.as_mut().unwrap();
        if let Some((y, x, val)) = to_update {
            table.table.rows[y][x] = val;
        }
        return Ok(true);
    }

    /// Shifts focus to the next focusable component.
    /// Returns true if at the end of its selection of focusable components
    /// and its containing component should move to its next component,
    /// false if this was able to change focus
    pub fn next_focus(&mut self) -> bool {
        match self.focus {
            FocusArea::Commands => {
                self.focus = FocusArea::Table;
                false
            }
            FocusArea::Table | FocusArea::Editor => true,
        }
    }

    /// Shifts focus to the previous focusable component.
    /// Returns true if at the end of its selection of focusable components
    /// and its containing component should move to its previous component,
    /// false if this was able to change focus
    pub fn prev_focus(&mut self) -> bool {
        match self.focus {
            FocusArea::Table | FocusArea::Editor => {
                self.focus = FocusArea::Commands;
                false
            }
            FocusArea::Commands => true,
        }
    }

    pub fn focus_first(&mut self) {
        self.focus = FocusArea::Commands;
    }

    pub fn focus_last(&mut self) {
        self.focus = FocusArea::Table;
    }

    /// Updates the number of selections to hold the new max number.
    /// Truncates the list, removing the more recent selections, if new_max is
    /// less than the current max selections.
    fn set_max_selections(&mut self, new_max: usize) {
        if self.max_selections == new_max {
            return;
        }
        if let Some(table) = &mut self.table {
            table.set_max_selections(new_max);
        }
        self.max_selections = new_max;
    }

    /// Updates the selection type to be the new type.
    /// Removes selections of the old type if it is changed.
    fn set_selection_type(&mut self, use_rows: bool) {
        if self.uses_rows == use_rows {
            return;
        }
        if let Some(table) = &mut self.table {
            table.set_selection_type(use_rows);
        }
        self.uses_rows = use_rows;
    }

    /// Changes the table stored to be the passed one, and reverts the
    /// stored query to the default one.
    fn change_stored_table(&mut self, table_name: &str) -> Result<(), Box<dyn Error>> {
        if table_name != self.table_name {
            self.table_name = table_name.to_owned();
            self.query = None;
            // update column info
            self.column_info = self.connection.get_column_info(table_name)?;
        }
        Ok(())
    }

    fn handle_edit_command_change(&mut self) {
        if let Some(command) = self.command_list.selected() {
            match command {
                EditCommand::Add => match AddComponent::new(&self.table_name) {
                    Err(err) => panic!("{:?}", err),
                    Ok(add_comp) => self.add_component = Some(add_comp),
                },
                _ => {
                    // TODO: MAY WANT TO CHANGE THIS SO THAT STATE FROM THE ADD SCREEN IS STORED
                    //       INSTEAD OF DESTROYED WHEN EDIT CHOICES ARE CHANGED
                    self.add_component = None;
                    self.set_max_selections(command.num_selections());
                    self.set_selection_type(command.uses_rows());
                }
            }
        }
    }

    /// Runs upon handling a SelectionChanged Action
    fn handle_table_selection(&mut self) -> Result<(), Box<dyn Error>> {
        let command = self
            .command_list
            .selected()
            .expect("Should be unable to change selection without an edit mode selected");
        match command {
            EditCommand::Delete => {
                // delete the selected item
                self.delete()?;
                Ok(())
            }
            EditCommand::Modify => {
                // TODO: more with this?
                self.focus = FocusArea::Editor;
                if let Some(editor) = &mut self.cell_display {
                    editor.toggle_focus();
                }
                Ok(())
            }
            _ => Ok(()), // do nothing for most
        }
    }

    // Runs when the highlit cell within the table changes
    fn update_cell_display(&mut self) {
        if let Some(table) = &self.table {
            if let Some(highlit_cell) = table.highlit_cell_value() {
                self.cell_display = Some(EditableText::new(&highlit_cell));
            }
        }
    }

    fn handle_actions(&mut self, actions: Vec<Action>) -> Vec<Action> {
        // handle the actions which may be returned by the add component or the commandlist
        let mut actions = actions;
        // loops over the actions in order, removing any which return false (which are handled),
        // returning the list of actions which weren't handled
        actions.retain(|action| match action {
            Action::ChangeEditCommand => {
                self.handle_edit_command_change();
                false
            }
            Action::RevertEditHighlight => {
                self.command_list.highlight_current_selection();
                false
            }
            Action::RevertEditSelection => {
                self.command_list.revert_selection();
                false
            }
            Action::RevertToMain => {
                // TODO: MAY WANT TO CHANGE THIS SO THAT STATE FROM THE ADD SCREEN IS STORED
                //       INSTEAD OF DESTROYED WHEN EDIT CHOICES ARE CHANGED
                self.add_component = None;
                false
            }
            _ => true,
        });
        actions
    }
}

impl Component for DatabaseComp {
    fn handle_event(&mut self, event: Action) -> Result<Vec<Action>, Box<dyn Error>> {
        match self.focus {
            FocusArea::Commands => {
                let actions = self.command_list.handle_event(event)?;
                Ok(self.handle_actions(actions))
            }
            FocusArea::Table | FocusArea::Editor => {
                // handle the add component if there is one showing
                if let Some(add_comp) = &mut self.add_component {
                    let actions = add_comp.handle_event(event)?;
                    return Ok(self.handle_actions(actions));
                }
                match event {
                    Action::Noop => Ok(vec![Action::Noop]),
                    Action::Quit => Ok(vec![Action::Quit]),
                    Action::KeyEvent(key_event) => {
                        if self.focus == FocusArea::Table {
                            self.handle_key_event(key_event)
                        } else {
                            match key_event.code {
                                KeyCode::Esc => {
                                    if let Some(editor) = &mut self.cell_display {
                                        editor.toggle_focus();
                                    }
                                    self.focus = FocusArea::Table;
                                    if let Some(table) = &mut self.table {
                                        table.reset_selections();
                                    }
                                    Ok(vec![Action::Noop])
                                }
                                KeyCode::Enter => {
                                    // TODO: as the editor is only active during modify,
                                    //       this should run the SQL to validate and modify the cell,
                                    //       then change focus back to the table, removing the
                                    //       selection as well
                                    if self.submit_modify()? {
                                        if let Some(editor) = &mut self.cell_display {
                                            editor.toggle_focus();
                                        }
                                        self.focus = FocusArea::Table;
                                        if let Some(table) = &mut self.table {
                                            table.reset_selections();
                                        }
                                        Ok(vec![Action::Noop])
                                    } else {
                                        Ok(vec![Action::VeryLoudWrongBuzzer])
                                    }
                                }
                                _ => {
                                    if let Some(editor) = &mut self.cell_display {
                                        editor.handle_key_event(key_event)
                                    } else {
                                        panic!("Somehow focusing editor without editor present");
                                    }
                                }
                            }
                        }
                    }
                    Action::OtherEvent(other_event) => self.handle_other_event(other_event),
                    // Action::Filter(filter) => {
                    //     self.filter(&filter)?;
                    //     Ok(vec![Action::Noop])
                    // }
                    unhandled => Err(Box::new(UnhandledActionError::new(unhandled))),
                }
            }
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        // TODO: also make the column_info be used here and passed somehow to the add component, likely requiring lifetimes but w/e

        if let Some(table) = &mut self.table {
            let mut actions = table.handle_key_event(key)?;
            // handle any changes of highlight or selection in the table within this component
            let mut highlight_changed = false;
            let mut selection_changed = false;
            actions.retain(|a| match a {
                Action::HighlightChanged => {
                    highlight_changed = true;
                    false
                }
                Action::SelectionChanged => {
                    selection_changed = true;
                    false
                }
                _ => true,
            });
            if highlight_changed {
                self.update_cell_display();
            }
            if selection_changed {
                self.handle_table_selection()?;
            }
            Ok(actions)
        } else {
            Ok(vec![Action::Noop])
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        // split the passed rect for the edits commands and the table itself
        let [commands_rect, main_rect, ..] = *Layout::default()
            .margin(0)
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // 3 pixels of height for the list of commands
                Constraint::Min(7),    // At least 7 pixels of height for the rest
            ])
            .split(rect)
        else {
            panic!("Not enough size to create the necessary rects");
        };

        if let Some(table) = &mut self.table {
            // uses the passed block for the potentially focused component as
            // the block will be unfocused if this component is not focused
            let (commands_block, main_block) = match self.focus {
                FocusArea::Commands => (block, DEFAULT_APP_COLORS.default_block()),
                FocusArea::Table | FocusArea::Editor => (DEFAULT_APP_COLORS.default_block(), block),
            };
            self.command_list.render(f, commands_rect, commands_block);
            if let Some(add_comp) = &mut self.add_component {
                // render the add component if it is shown
                add_comp.render(f, main_rect, main_block);
            } else {
                if let Some(cell_display) = &mut self.cell_display {
                    // split the main_rect to show the cell display
                    let [table_rect, cell_display_rect, ..] = *Layout::default()
                        .margin(1) // 1 margin to account for border
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(75), // table takes up 75% of main area
                            Constraint::Min(8), // cell display requires at least 8 cols width
                        ])
                        .split(main_rect)
                    else {
                        panic!("Not enough size to create the necessary rects");
                    };
                    // TODO: Instead of offsetting this with a 1px margin,
                    //       add a title denoting that it is the reader
                    //       and border it better (make it look good)
                    cell_display.render(
                        f,
                        cell_display_rect.inner(Margin::new(1, 1)),
                        Block::new(),
                    );
                    table.render(f, table_rect, Block::new());
                    // render the main border block separately
                    f.render_widget(main_block, main_rect);
                } else {
                    table.render(f, main_rect, main_block);
                }
            }
        } else {
            f.render_widget(
                Paragraph::new("No table queried").centered().block(block),
                rect,
            );
        }
    }
}
