use std::{borrow::Cow, error::Error};

use ratatui::{
    text::Text,
    widgets::{
        Cell, Row, Scrollbar, ScrollbarState, Table as TuiTable, TableState as TuiTableState,
    },
};

use super::*;

use crate::{connection::Table, value::Value};

const ROW_HEIGHT: usize = 2;

/// Component which wraps over a [`crate::connection::Table`] and a ratatui
/// Table widget in order to allow for selecting multiple items within a
/// table and display them properly
pub struct TableDisplay {
    pub(crate) table: Table,
    pub(crate) uses_rows: bool,
    state: MultiTableState,
    table_state: TuiTableState,
    scroll_state: ScrollbarState,
}

impl TableDisplay {
    pub fn from_table(
        table: Table,
        uses_rows: bool,
        max_selections: usize,
    ) -> Result<Self, Box<dyn Error>> {
        let num_items = table.rows.len();
        Ok(Self {
            table,
            uses_rows,
            state: MultiTableState::new(max_selections),
            table_state: TuiTableState::new().with_selected_cell(Some((0, 0))),
            scroll_state: ScrollbarState::new((num_items.saturating_sub(1)) * ROW_HEIGHT),
        })
    }

    pub fn clone_from_table(
        table: &Table,
        uses_rows: bool,
        max_selections: usize,
    ) -> Result<Self, Box<dyn Error>> {
        let num_items = table.rows.len();
        Ok(Self {
            uses_rows,
            state: MultiTableState::new(max_selections),
            table: table.clone(),
            table_state: TuiTableState::new().with_selected_cell(Some((0, 0))),
            scroll_state: ScrollbarState::new((num_items.saturating_sub(1)) * ROW_HEIGHT),
        })
    }

    pub fn highlit_cell_value(&self) -> Option<String> {
        self.table_state.selected_cell().map(|(y, x)| {
            // ensure clamping of values as the state doesn't update to proper
            // selected row until rendering occurs, which is too late
            let y = if y == usize::MAX {
                self.table.rows.len() - 1
            } else {
                y
            };
            let x = if x == usize::MAX {
                self.table.columns.len() - 1
            } else {
                x
            };
            self.table.rows[y][x].to_string()
        })
    }

    /// Returns the MultiTable's current set of selections
    pub fn selections(&self) -> &[MultiTableSelection] {
        self.state.selections.as_slice()
    }

    /// Simple wrapped getter for the underlying table's columns.
    /// Shorthand for calling TableDisplay.table.columns
    pub fn columns(&self) -> &[String] {
        self.table.columns.as_slice()
    }

    /// Simple wrapped getter for the underlying table's rows
    /// Shorthand for calling TableDisplay.table.rows
    pub fn rows(&self) -> &[Vec<Value>] {
        self.table.rows.as_slice()
    }

    /// Clears all selections, leaving allocated capacity the same
    pub fn reset_selections(&mut self) {
        self.state.selections.clear();
    }

    /// Updates the number of selections to hold the new max number.
    /// Truncates the list, removing the more recent selections, if new_max is
    /// less than the current max selections.
    pub fn set_max_selections(&mut self, new_max: usize) {
        self.state.selections.truncate(new_max);
        self.state.max_selections = new_max;
    }

    /// Updates the selection type to be the new type.
    /// Removes selections of the old type if it is changed.
    pub fn set_selection_type(&mut self, use_rows: bool) {
        if use_rows != self.uses_rows {
            // since we change the type, clear all selections
            self.reset_selections();
        }
        self.uses_rows = use_rows;
    }

    /// Simple wrapper over the MultiTableState method of the same name,
    /// used for setting selections separate from user action
    pub fn select(&mut self, selection: MultiTableSelection) {
        self.state.select(selection);
    }

    /// Moves the selected cell to the left by amount.
    /// Wraps selection to the last column if we are at column 0.
    /// Light wrapper of TableState's same-named function.
    fn scroll_left_by(&mut self, amount: u16) {
        // if self.uses_rows {
        //     return;
        // }
        if let Some((_, x)) = self.table_state.selected_cell() {
            if x == 0 {
                self.table_state.select_last_column();
                return;
            }
        }
        self.table_state.scroll_left_by(amount);
    }

    /// Moves the selected cell to the right by amount.
    /// Wraps selection to the first column if we are at the last one.
    /// Light wrapper of TableState's same-named function.
    fn scroll_right_by(&mut self, amount: u16) {
        // if self.uses_rows {
        //     return;
        // }
        if let Some((_, x)) = self.table_state.selected_cell() {
            if x == self.table.columns.len() - 1 {
                self.table_state.select_first_column();
                return;
            }
        }
        self.table_state.scroll_right_by(amount);
    }

    /// Moves the selected row/cell up by amount.
    /// Wraps selection to the last row if we are at row 0.
    /// Light wrapper of TableState's same-named function.
    fn scroll_up_by(&mut self, amount: u16) {
        if let Some(y) = self.table_state.selected() {
            if y == 0 {
                self.table_state.select_last();
                self.scroll_state.last();
                return;
            }
        }
        self.table_state.scroll_up_by(amount);
        self.scroll_state = self
            .scroll_state
            .position(self.table_state.selected().unwrap() * ROW_HEIGHT);
    }

    /// Moves the selected row/cell down by amount.
    /// Wraps selection to the first row if we are at the last one.
    /// Light wrapper of TableState's same-named function.
    fn scroll_down_by(&mut self, amount: u16) {
        if let Some(y) = self.table_state.selected() {
            if y == self.table.rows.len() - 1 {
                self.table_state.select_first();
                self.scroll_state.first();
                return;
            }
        }
        self.table_state.scroll_down_by(amount);
        self.scroll_state = self
            .scroll_state
            .position(self.table_state.selected().unwrap() * ROW_HEIGHT);
    }
}

impl Component for TableDisplay {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        // ignore key releases
        if key.kind == KeyEventKind::Release {
            return Ok(vec![Action::Noop]);
        }

        match key.code {
            KeyCode::Esc => Ok(vec![Action::Quit]), // terminate on encountering Esc
            KeyCode::Enter => {
                let selection_opt: Option<MultiTableSelection> = if self.uses_rows {
                    self.table_state.selected().map(|row| row.into())
                } else {
                    self.table_state.selected_cell().map(|cell| cell.into())
                };
                if let Some(selection) = selection_opt {
                    // if selection was added, return SelectionChanged, else Noop
                    if self.state.select(selection) {
                        Ok(vec![Action::SelectionChanged])
                    } else {
                        Ok(vec![Action::Noop])
                    }
                } else {
                    Ok(vec![Action::Noop])
                }
            }
            KeyCode::Left => {
                self.scroll_left_by(1);
                Ok(vec![Action::HighlightChanged])
            }
            KeyCode::Right => {
                self.scroll_right_by(1);
                Ok(vec![Action::HighlightChanged])
            }
            KeyCode::Up => {
                self.scroll_up_by(1);
                Ok(vec![Action::HighlightChanged])
            }
            KeyCode::Down => {
                self.scroll_down_by(1);
                Ok(vec![Action::HighlightChanged])
            }
            _ => Ok(vec![Action::Noop]),
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        // map the column names into cells for the sake of the header row of the table
        let columns = Row::from_iter(
            self.table
                .columns
                .iter()
                .map(|column| Text::from(Cow::from(column)).centered()),
        );

        // define the style for each row
        let row_style = Style::default()
            .fg(DEFAULT_APP_COLORS.main_fg)
            .bg(DEFAULT_APP_COLORS.main_bg);

        let selection_colors = DEFAULT_APP_COLORS.selection_colors();
        // map the rows' cells into Ratatui rows for the sake of the display
        let rows: Vec<Row> = self
            .table
            .rows
            .iter()
            .enumerate()
            .map(|(y, row)| {
                // determine the color to use for the current selection
                let selected_style_base = Style::default().bold();
                // determine if this row needs to be selected as it overrides cell styles
                let row_selected_ind = if self.uses_rows {
                    self.state.index_of(MultiTableSelection::Row(y))
                } else {
                    None
                };
                // update highlighting depending on selection style and selected items
                Row::new(row.iter().enumerate().map(|(x, cell)| {
                    let mut cur_cell_style = if row_selected_ind.is_none() {
                        // current row is not selected, so column color is more complex
                        if self.uses_rows
                            && self.table_state.selected_cell().is_some_and(
                                |(highlit_row, highlit_col)| y < highlit_row && highlit_col == x,
                            )
                        {
                            // make highlit column have a special bg color
                            Style::new().bg(DEFAULT_APP_COLORS.highlit_bg)
                        } else if x % 2 == 0 {
                            // alternate color as column is not highlit
                            Style::new().bg(DEFAULT_APP_COLORS.alt_bg)
                        } else {
                            // just use no style as the row style acts as a default
                            Style::new()
                        }
                    } else {
                        // just use no style as the row style acts as a default
                        Style::new()
                    };
                    if !self.uses_rows {
                        // cell selection is used, so change style if this cell is selected
                        if let Some(i) = self.state.index_of(MultiTableSelection::Cell((y, x))) {
                            cur_cell_style = selected_style_base
                                .bg(selection_colors[i % selection_colors.len()]);
                        }
                    }
                    Cell::from(cell.to_string()).style(cur_cell_style)
                }))
                .style(if let Some(i) = row_selected_ind {
                    selected_style_base.bg(selection_colors[i % selection_colors.len()])
                } else {
                    row_style
                })
                .height(ROW_HEIGHT as u16)
            })
            .collect();
        // set up the styling of the table, its header, and its selections
        let header_style = Style::default()
            .fg(DEFAULT_APP_COLORS.header_fg)
            .bg(DEFAULT_APP_COLORS.header_bg);
        let highlight_style = Style::new().reversed();

        let mut table = TuiTable::default()
            .block(block)
            .bg(DEFAULT_APP_COLORS.main_bg)
            .highlight_symbol(
                // each item in the vec is a line, so 2 lines in accordance with ROW_HEIGHT
                Text::from(vec![" ╲ ".into(), " ╱ ".into()])
                    .fg(DEFAULT_APP_COLORS.main_fg)
                    .bold(),
            );

        if self.uses_rows {
            table = table.row_highlight_style(highlight_style);
        } else {
            table = table.cell_highlight_style(highlight_style);
        }
        // make it have the desired columns and rows
        table = table
            .header(columns.style(header_style).height(1))
            .rows(rows);
        f.render_stateful_widget(table, rect, &mut self.table_state);

        // render the scrollbar for the table
        let mut scrollbar_rect = rect.clone();
        scrollbar_rect.y += 1;
        scrollbar_rect.height = scrollbar_rect.height.saturating_sub(1);
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(DEFAULT_APP_COLORS.main_fg),
            scrollbar_rect,
            &mut self.scroll_state,
        );
    }
}

/// A collection of multiple selections, up to the passed amount,
/// defaulting to 1 max selection
struct MultiTableState {
    pub(crate) max_selections: usize,
    pub(crate) selections: Vec<MultiTableSelection>,
}

/// Enum storing selections depending on whether the MultiTable selects rows
/// or cells.
///
/// When storing cells, the values are stored in (y, x) order as it is in Ratatui
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum MultiTableSelection {
    /// Tuple storing a coordinate in (y, x) order
    Cell((usize, usize)),
    /// Offset of the row/the y value of any cell in a row
    Row(usize),
}

impl From<(usize, usize)> for MultiTableSelection {
    fn from(value: (usize, usize)) -> Self {
        MultiTableSelection::Cell(value)
    }
}

impl From<usize> for MultiTableSelection {
    fn from(value: usize) -> Self {
        MultiTableSelection::Row(value)
    }
}

impl Default for MultiTableState {
    fn default() -> Self {
        Self {
            max_selections: 1,
            selections: Vec::with_capacity(1),
        }
    }
}

impl MultiTableState {
    fn new(max_selections: usize) -> Self {
        Self {
            max_selections,
            selections: Vec::with_capacity(max_selections),
        }
    }

    /// Returns the index of the equivalent selection within the list of
    /// selections if present, else None
    fn index_of(&self, selection: MultiTableSelection) -> Option<usize> {
        self.selections.iter().position(|item| *item == selection)
    }

    /// Adds the passed selection to the Vec of selections,
    /// or removes it if it is already present
    ///
    /// Pushes new selections to the end of the list such that
    /// older selections will be at the front of the list.
    ///
    /// Returns true if the selection was added, false if not
    fn select(&mut self, selection: MultiTableSelection) -> bool {
        // search for item in reverse under the naive, but somewhat true
        // assumption that the selections which get removed most are those
        // which have been more recently added
        if let Some(ind) = self.selections.iter().rposition(|item| *item == selection) {
            self.selections.remove(ind);
        } else if self.selections.len() < self.max_selections {
            self.selections.push(selection);
            return true;
        }
        false
    }
}
