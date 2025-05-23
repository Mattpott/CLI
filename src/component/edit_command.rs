use ratatui::{
    layout::Constraint,
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Cell, Row, Table, TableState},
};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, strum_macros::Display)]
pub enum EditCommand {
    Add,
    Modify,
    Delete,
    Reorder,
    Swap,
}

impl EditCommand {
    pub fn num_selections(&self) -> usize {
        match self {
            Self::Add => 0,
            Self::Modify => 1,
            Self::Delete => 1,
            Self::Reorder => 4,
            Self::Swap => 2,
        }
    }

    pub fn uses_rows(&self) -> bool {
        !matches!(self, Self::Modify)
    }
}

pub struct EditCommandComp {
    commands: Vec<EditCommand>,
    state: TableState,
    selected: Option<usize>,
    prev_selected: Option<usize>,
}

impl EditCommandComp {
    pub fn new(commands: Vec<EditCommand>) -> Self {
        Self {
            commands,
            state: TableState::new().with_selected_column(Some(0)),
            selected: Some(0),
            prev_selected: None,
        }
    }

    pub fn selected(&self) -> Option<EditCommand> {
        self.selected.map(|ind| self.commands[ind].clone())
    }

    pub fn change_commands(&mut self, commands: Vec<EditCommand>) {
        // ensure no reading of empty list of commands when changing a default component
        let highlit_opt = self.state.selected_column();
        // highlit_opt must be Some as in order to select there must be some highlit component
        if !self.commands.is_empty() && highlit_opt.is_some() {
            let highlit_ind = highlit_opt.unwrap();
            let highlit_command = &self.commands[highlit_ind];
            // highlight the same command in the new list of commands if it exists,
            // or reset to first column if the highlit command is not in the new list
            let assoc_ind = commands
                .iter()
                .position(|com| com == highlit_command)
                .unwrap_or(0);
            self.state.select_column(Some(assoc_ind));

            // select either previously selected option or just the first option, which is default
            self.selected = self.selected.map_or(Some(0), |ind| {
                let selected_command = &self.commands[ind];
                commands
                    .iter()
                    .position(|com| com == selected_command)
                    .or(Some(0))
            });
        }
        self.commands = commands;
    }

    /// Makes the current selection be the previously selected item
    pub fn revert_selection(&mut self) {
        self.selected = self.prev_selected;
    }

    /// Change highlit option to be the currently selected item
    pub fn highlight_current_selection(&mut self) {
        if let Some(selected) = self.selected {
            self.state.select_column(Some(selected));
        }
    }

    fn scroll_left_by(&mut self, amount: u16) {
        if let Some(x) = self.state.selected_column() {
            if x == 0 {
                self.state.select_last_column();
                return;
            }
        }
        self.state.scroll_left_by(amount);
    }

    fn scroll_right_by(&mut self, amount: u16) {
        if let Some(x) = self.state.selected_column() {
            if x == self.commands.len() - 1 {
                self.state.select_first_column();
                return;
            }
        }
        self.state.scroll_right_by(amount);
    }
}

impl Component for EditCommandComp {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        // ignore key releases
        if key.kind == KeyEventKind::Release {
            return Ok(vec![Action::Noop]);
        }
        match key.code {
            KeyCode::Esc => Ok(vec![Action::Quit]), // terminate on encountering Esc
            KeyCode::Enter => {
                // needed in cases where the action shouldn't actually stay selected
                self.prev_selected = self.selected;
                self.selected = self.state.selected_column();
                Ok(vec![Action::ChangeEditCommand])
            }
            KeyCode::Left => {
                self.scroll_left_by(1);
                Ok(vec![Action::Noop])
            }
            KeyCode::Right => {
                self.scroll_right_by(1);
                Ok(vec![Action::Noop])
            }
            _ => Ok(vec![Action::Noop]),
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        let commands = if !self.commands.is_empty() {
            let highlight_style = Style::new().reversed();
            let strings: Vec<String> = self
                .commands
                .iter()
                .map(|command| command.to_string())
                .collect();
            Table::default()
                .fg(DEFAULT_APP_COLORS.main_fg)
                .bg(DEFAULT_APP_COLORS.main_bg)
                .column_highlight_style(highlight_style)
                .cell_highlight_style(highlight_style)
                .block(block)
                .widths(
                    strings
                        .iter()
                        .map(|s| Constraint::Max(s.len() as u16 + 4))
                        .collect::<Vec<Constraint>>(),
                )
                .rows([Row::from_iter(strings.into_iter().enumerate().map(
                    |(ind, s)| {
                        let mut cell = Cell::new(Text::from(s).centered());
                        if Some(ind) == self.selected {
                            cell = cell.bg(DEFAULT_APP_COLORS.selection_one_bg);
                        }
                        cell
                    },
                ))])
        } else {
            Table::default()
                .fg(DEFAULT_APP_COLORS.main_fg)
                .bg(DEFAULT_APP_COLORS.main_bg)
                .cell_highlight_style(Color::LightBlue)
                .block(block)
                .rows([Row::new(vec!["No", "Items", "Present"])])
        };
        f.render_stateful_widget(commands, rect, &mut self.state);
    }
}
