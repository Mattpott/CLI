use std::{borrow::Cow, iter::zip};

use editable_text::EditableText;
use ratatui::{
    layout::Margin,
    widgets::{Padding, Paragraph},
};
use rusqlite::{params_from_iter, types::Value as RsqValue};

use crate::{
    connection::{ColumnInfo, Connection},
    value::Value,
};

use super::{popup::PopUpComponent, *};

#[derive(Debug, PartialEq, Eq)]
enum FocusArea {
    Main,
    Popup,
    Submit,
}

pub struct AddComponent {
    connection: Connection,
    column_info: Vec<ColumnInfo>,
    columns: Vec<String>,
    fields: Vec<EditableText>,
    focusing: FocusArea,
    hovering: usize,
    popup: PopUpComponent,
    selected_field: Option<usize>,
    table: String,
}

impl AddComponent {
    pub fn new(table: &str) -> Result<Self, Box<dyn Error>> {
        let connection = Connection::new()?;
        let column_info = connection.get_column_info(table)?;
        // collect column names and determine if that field is required (NOT NULL)
        let columns = connection.get_columns(table)?;
        // create an EditableTextComponent for each field
        let fields = columns.iter().map(|_| EditableText::default()).collect();
        Ok(Self {
            connection,
            column_info,
            columns,
            fields,
            focusing: FocusArea::Main,
            hovering: 0,
            popup: PopUpComponent::new(
                format!("Add this row to the {} table?", table),
                vec!["Yes".to_string(), "No".to_string()],
                None,
            ),
            selected_field: None,
            table: table.to_owned(),
        })
    }

    /// Simple check to ensure that the required fields are filled and
    /// each field contains the correct data type.
    fn requirements_filled(&self) -> bool {
        for (col, field) in zip(self.column_info.iter(), self.fields.iter()) {
            if !field.is_empty() {
                // ensure the value of the field can be properly parsed
                if Value::parse_column(&col.data_type, &field.text()).is_ok() {
                    // all is okay so far, so continue
                    continue;
                } else {
                    // improper data type in some field, so it is not valid
                    return false;
                }
            } else if col.is_not_null {
                // there is a required field that is empty, so it is not valid
                return false;
            }
        }
        true
    }

    /// Submits the current fields of the row for insertion into the table.
    /// Does not assume correctness of fields, and so will not submit if
    /// there are missing NON-NULL fields or invalid data types.
    fn submit(&mut self) -> Result<Vec<Action>, Box<dyn Error>> {
        let mut cols: Vec<String> = Vec::with_capacity(self.column_info.len());
        let mut values: Vec<Value> = Vec::with_capacity(self.fields.len());
        // pair columns and fields together, ignoring empty fields,
        // and also ensure required fields are filled
        for (col, field) in zip(self.column_info.iter(), self.fields.iter()) {
            if !field.is_empty() {
                // ensure the value of the field can be properly parsed
                if let Ok(val) = Value::parse_column(&col.data_type, &field.text()) {
                    // add the column name and associated value to the list
                    cols.push(col.name.to_owned());
                    values.push(val);
                } else {
                    return Ok(vec![Action::VeryLoudWrongBuzzer]);
                }
            } else if col.is_not_null {
                // there is a required field that is empty, so don't submit
                return Ok(vec![Action::VeryLoudWrongBuzzer]);
            }
        }
        if !cols.is_empty() {
            // create a list of the positional arguments for joining into the query
            // as well as parse each value into a Rusqlite Value in order to bind
            // them as params within a prepared statement
            let (pos, params): (Vec<String>, Vec<RsqValue>) = values
                .into_iter()
                .enumerate()
                .map(|(ind, val)| (format!("?{}", ind + 1), val.into()))
                .unzip();
            // create the query with positional params as placeholders for the values
            let query = format!(
                "INSERT INTO {} ({}) VALUES ({});",
                self.table,
                cols.join(", "),
                pos.join(", ")
            );
            // run the query to insert the value with the intended params
            // TODO: STORE RESULT SOMEWHERE PROBABLY AS IT RETURNS THE ROW
            //       INDEX OF THE INSERTED ROW WHICH CAN BE USED FOR KEEPING
            //       THAT ROW SHOWN OR SOMETHING
            self.connection.insert(&query, params_from_iter(params))?;
        }
        Ok(vec![
            Action::RevertToMain,
            Action::RevertEditSelection,
            Action::Refresh,
        ])
    }

    fn handle_submit_keys(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        match key.code {
            KeyCode::Esc => Ok(vec![Action::Quit]), // terminate on encountering Esc
            KeyCode::Up => {
                if self.selected_field.is_none() {
                    // move up to the main section
                    self.focusing = FocusArea::Main;
                }
                Ok(vec![Action::Noop])
            }
            KeyCode::Enter => {
                if self.requirements_filled() {
                    self.focusing = FocusArea::Popup; // show popup to confirm
                    Ok(vec![Action::Noop])
                } else {
                    Ok(vec![Action::VeryLoudWrongBuzzer])
                }
            }
            _ => Ok(vec![Action::Noop]),
        }
    }

    fn handle_main_keys(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        match key.code {
            KeyCode::Esc => {
                // if a field is focused when Esc is pressed,
                // defocus it, else terminate
                if let Some(focus_ind) = self.selected_field {
                    self.fields[focus_ind].toggle_focus();
                    self.selected_field = None;
                    Ok(vec![Action::Noop])
                } else {
                    self.selected_field = Some(self.hovering);
                    self.fields[self.hovering].toggle_focus();
                    Ok(vec![Action::Quit])
                }
            }
            KeyCode::Left => {
                if let Some(focus_ind) = self.selected_field {
                    self.fields[focus_ind].handle_key_event(key)
                } else {
                    self.hovering = self.hovering.saturating_sub(1);
                    Ok(vec![Action::Noop])
                }
            }
            KeyCode::Right => {
                if let Some(focus_ind) = self.selected_field {
                    self.fields[focus_ind].handle_key_event(key)
                } else {
                    self.hovering = (self.hovering.saturating_add(1)).min(self.fields.len() - 1);
                    Ok(vec![Action::Noop])
                }
            }
            KeyCode::Down => {
                if self.selected_field.is_none() {
                    // move down to the submit button
                    self.focusing = FocusArea::Submit;
                }
                Ok(vec![Action::Noop])
            }
            KeyCode::Enter => {
                // enter will toggle the focus of the text component, and,
                // as shift-enter is not valid, newlines are added using some
                // other key combination determined in EditableTextComponent
                if let Some(focus_ind) = self.selected_field {
                    self.fields[focus_ind].toggle_focus();
                    self.selected_field = None;
                } else {
                    self.selected_field = Some(self.hovering);
                    self.fields[self.hovering].toggle_focus();
                }
                Ok(vec![Action::Noop])
            }
            _ => {
                // get the focused text component to funnel events to it
                if let Some(focus_ind) = self.selected_field {
                    self.fields[focus_ind].handle_key_event(key)
                } else {
                    Ok(vec![Action::Noop])
                }
            }
        }
    }
}

impl Component for AddComponent {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        // ignore key releases
        if key.kind == KeyEventKind::Release {
            return Ok(vec![Action::Noop]);
        }
        match self.focusing {
            FocusArea::Main => self.handle_main_keys(key),
            FocusArea::Submit => self.handle_submit_keys(key),
            FocusArea::Popup => {
                let actions = self.popup.handle_key_event(key)?;
                let mut handled = false;
                let handled_actions = match actions[..] {
                    [Action::NotifyCompletion] => {
                        handled = true;
                        // index 0 is the yes choice, so confirm submission
                        if self.popup.get_choice() == 0 {
                            self.submit()
                        } else {
                            // close the popup since they chose cancel
                            self.focusing = FocusArea::Submit;
                            Ok(vec![Action::Noop])
                        }
                    }
                    [Action::Quit] => {
                        handled = true;
                        self.focusing = FocusArea::Submit; // hide popup
                        Ok(vec![Action::Noop])
                    }
                    _ => Ok(vec![Action::Noop]),
                };
                if handled {
                    handled_actions
                } else {
                    Ok(actions)
                }
            }
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        // create a Rect which doesn't include the block/border
        let borderless = rect.inner(Margin::new(1, 1));
        // set up styles
        let header_style = Style::new()
            .fg(DEFAULT_APP_COLORS.header_fg)
            .bg(DEFAULT_APP_COLORS.header_bg);
        let header_hover_style = Style::new().bg(DEFAULT_APP_COLORS.selection_one_bg);
        let field_height = borderless.height - 3; // -1 for the header, -1 for column info, and -1 for submit
        let field_width = borderless.width / self.fields.len() as u16;

        let base_style = Style::new()
            .fg(DEFAULT_APP_COLORS.main_fg)
            .bg(DEFAULT_APP_COLORS.main_bg);
        let alt_style = Style::new()
            .fg(DEFAULT_APP_COLORS.main_fg)
            .bg(DEFAULT_APP_COLORS.alt_bg);

        // render an empty paragraph for external border and background
        f.render_widget(Paragraph::new("").style(base_style).block(block), rect);

        // render the info for each column above each column name
        for (ind, info) in self.column_info.iter().enumerate() {
            f.render_widget(
                Paragraph::new(info.to_string())
                    .centered()
                    .style(header_style)
                    .block(Block::new().padding(Padding::symmetric(1, 0))),
                Rect::new(
                    borderless.x + (field_width * ind as u16),
                    borderless.y,
                    field_width,
                    1,
                ),
            );
        }

        // render the header as a series of paragraphs
        for (ind, column) in self.columns.iter().enumerate() {
            let col_style = if self.focusing == FocusArea::Main && ind == self.hovering {
                header_hover_style
            } else {
                header_style
            };
            f.render_widget(
                Paragraph::new(Cow::from(column))
                    .centered()
                    .style(col_style),
                Rect::new(
                    borderless.x + (field_width * ind as u16),
                    borderless.y + 1,
                    field_width,
                    1,
                ),
            );
        }

        // render each field's input location
        for (ind, text_component) in self.fields.iter_mut().enumerate() {
            text_component.render_with_style(
                f,
                Rect::new(
                    borderless.x + (field_width * ind as u16),
                    borderless.y + 2,
                    field_width,
                    field_height,
                ),
                Block::new(),
                if ind % 2 == 0 { alt_style } else { base_style },
            );
        }

        // render the submit button
        f.render_widget(
            Paragraph::new("Submit")
                .centered()
                .style(if self.focusing == FocusArea::Submit {
                    header_hover_style
                } else {
                    header_style
                }),
            Rect::new(
                borderless.x,
                borderless.y + borderless.height - 1,
                borderless.width,
                1,
            ),
        );

        // if the popup is focused, also show that
        if self.focusing == FocusArea::Popup {
            self.popup.render(
                f,
                borderless.inner(Margin {
                    horizontal: borderless.width / 5,
                    vertical: borderless.height / 5,
                }),
                DEFAULT_APP_COLORS.default_block(),
            );
        }
    }
}
