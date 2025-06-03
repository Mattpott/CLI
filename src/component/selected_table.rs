use std::{borrow::Cow, collections::HashMap};

use command_list::EditCommand;
use ratatui::widgets::{List, ListItem, ListState};

use crate::{autofill::AutoFillFn, config::editable_tables};

use super::*;

#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub(crate) commands: Vec<EditCommand>,
    pub(crate) display_name: &'static str,
    pub(crate) table_name: &'static str,
    pub(crate) autofill_funcs: HashMap<&'static str, AutoFillFn>,
}

impl std::fmt::Display for TableMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name)
    }
}

pub struct TableSelection {
    allowed_tables: Vec<TableMetadata>,
    selected_ind: usize,
    state: ListState,
}

impl TableSelection {
    pub fn new() -> Self {
        Self {
            allowed_tables: editable_tables(),
            selected_ind: 0,
            state: ListState::default().with_selected(Some(0)),
        }
    }

    pub fn selected(&self) -> Option<&TableMetadata> {
        if !self.allowed_tables.is_empty() {
            Some(&self.allowed_tables[self.selected_ind])
        } else {
            None
        }
    }

    fn scroll_up_by(&mut self, amount: u16) {
        if let Some(x) = self.state.selected() {
            if x == 0 {
                self.state.select_last();
                return;
            }
        }
        self.state.scroll_up_by(amount);
    }

    fn scroll_down_by(&mut self, amount: u16) {
        if let Some(x) = self.state.selected() {
            if x == self.allowed_tables.len() - 1 {
                self.state.select_first();
                return;
            }
        }
        self.state.scroll_down_by(amount);
    }
}

impl Component for TableSelection {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        let mut quit: bool = false;
        match key.code {
            KeyCode::Esc => quit = true, // terminate on encountering Esc
            KeyCode::Enter => {
                if let Some(x) = self.state.selected() {
                    // TODO: change the table and actions to match the ones allowed by the selected item
                    self.selected_ind = x;
                    // notify the app to change the selected table and revert
                    // to the main screen if on the add screen
                    return Ok(vec![Action::ChangeSelectedTable, Action::RevertToMain]);
                }
            }
            KeyCode::Up => self.scroll_up_by(1),
            KeyCode::Down => self.scroll_down_by(1),
            _ => {}
        }
        if quit {
            Ok(vec![Action::Quit])
        } else {
            Ok(vec![Action::Noop])
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        let highlight_style = Style::new().reversed();
        let tables = List::from_iter(self.allowed_tables.iter().enumerate().map(|(ind, tab)| {
            let mut item = ListItem::new(Cow::from(tab.display_name));
            if ind == self.selected_ind {
                item = item.bg(DEFAULT_APP_COLORS.selection_one_bg);
            }
            item
        }))
        .fg(DEFAULT_APP_COLORS.main_fg)
        .bg(DEFAULT_APP_COLORS.main_bg)
        .highlight_style(highlight_style)
        .direction(ratatui::widgets::ListDirection::TopToBottom)
        .block(block);
        f.render_stateful_widget(tables, rect, &mut self.state);
    }
}
