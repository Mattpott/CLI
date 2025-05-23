use crate::{component::table_display::MultiTableSelection, connection::Connection};

use super::{popup::PopUpComponent, *};

use ratatui::widgets::Paragraph;
use rusqlite::{params_from_iter, types::Value as RsqValue};
use table_display::TableDisplay;

pub struct DatabaseComp {
    // column_info: Vec<ColumnInfo>,
    connection: Connection,
    max_selections: usize,
    popup: Option<Box<PopUpComponent>>, // boxed as popups are dynamically allocated
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
            // column_info: connection.get_column_info(table_name)?,
            connection,
            max_selections,
            popup: None,
            query: None,
            table: None,
            table_name: table_name.to_owned(),
            uses_rows,
        })
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
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

    /// Clears all selections
    pub fn reset_selections(&mut self) {
        if let Some(table) = &mut self.table {
            table.reset_selections();
        }
    }

    /// Updates the number of selections to hold the new max number.
    /// Truncates the list, removing the more recent selections, if new_max is
    /// less than the current max selections.
    pub fn set_max_selections(&mut self, new_max: usize) {
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
    pub fn set_selection_type(&mut self, use_rows: bool) {
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
    pub fn change_stored_table(&mut self, table_name: &str) -> Result<(), Box<dyn Error>> {
        if table_name != self.table_name {
            self.table_name = table_name.to_owned();
            self.query = None;
            // update column info
            // self.column_info = self.connection.get_column_info(table_name)?;
        }
        Ok(())
    }

    /// Deletes the currently selected row from the table within the database.
    /// Only works if there is 1 selected row for now.
    pub fn delete(&mut self) -> Result<Vec<Action>, Box<dyn Error>> {
        // only allow removal of a row, not a cell
        assert!(self.uses_rows);

        if let Some(table) = &self.table {
            if table.selections().len() != 1 {
                return Ok(vec![Action::Noop]);
            }
            if let MultiTableSelection::Row(row) = table.selections()[0] {
                // create a list of the positional arguments for joining into the query,
                // using the column name to fill before each positional argument,
                // and convert each value into a Rusqlite Value to bind
                // them as params within the prepared statement

                // TODO: column info here for now, may replace with self.column_info at some point

                let (pos, params): (Vec<String>, Vec<RsqValue>) = self
                    .connection
                    .get_column_info(&self.table_name)?
                    .iter()
                    .enumerate()
                    .filter_map(|(ind, info)| {
                        if info.is_primary_key {
                            // as the column name is taken directly from pragma_table_info,
                            // the column should be present within the columns
                            // create positional argument in the form of "COL_NAME = ?IND"
                            Some((
                                format!("{} = ?{}", info.name, ind + 1),
                                table
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
                // DELETE FROM table WHERE col_name1 = value1 AND col_name2 = value2 LIMIT num;
                let query = format!(
                    // "DELETE FROM {} WHERE {} LIMIT 1;",
                    "DELETE FROM {} WHERE {};",
                    self.table_name,
                    pos.join(" AND ")
                );
                // TODO: maybe store the response to show as a thingy
                self.connection.delete(&query, params_from_iter(params))?;
                return Ok(vec![Action::Refresh, Action::RevertEditHighlight]);
            }
        }
        Ok(vec![Action::Noop])
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
}

impl Component for DatabaseComp {
    fn handle_event(&mut self, event: Action) -> Result<Vec<Action>, Box<dyn Error>> {
        match event {
            Action::Noop => Ok(vec![Action::Noop]),
            Action::Quit => Ok(vec![Action::Quit]),
            Action::KeyEvent(key_event) => self.handle_key_event(key_event),
            Action::OtherEvent(other_event) => self.handle_other_event(other_event),
            // Action::Filter(filter) => {
            //     self.filter(&filter)?;
            //     Ok(vec![Action::Noop])
            // }
            unhandled => Err(Box::new(UnhandledActionError::new(unhandled))),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        if let Some(table) = &mut self.table {
            table.handle_key_event(key)
        } else {
            Ok(vec![Action::Noop])
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        // create a Rect which doesn't include the block/border
        // let borderless = rect.inner(Margin::new(1, 1));
        if let Some(table) = &mut self.table {
            table.render(f, rect, block);
        } else {
            f.render_widget(
                Paragraph::new("No table queried").centered().block(block),
                rect,
            );
        }
    }
}
