use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Clear, Paragraph},
};

use super::*;

pub struct PopUpComponent {
    prompt: String,
    choices: Vec<String>,
    highlit: u16,
}

impl PopUpComponent {
    pub fn new(prompt: String, choices: Vec<String>, initial_ind: Option<u16>) -> Self {
        Self {
            prompt,
            choices,
            highlit: initial_ind.unwrap_or(0),
        }
    }

    pub fn get_choice(&self) -> u16 {
        self.highlit
    }
}

impl Component for PopUpComponent {
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Vec<Action>, Box<dyn Error>> {
        // ignore key releases
        if key.kind == KeyEventKind::Release {
            return Ok(vec![Action::Noop]);
        }

        match key.code {
            KeyCode::Esc => Ok(vec![Action::Quit]), // close popup
            KeyCode::Enter => Ok(vec![Action::NotifyCompletion]), // notify container
            KeyCode::Left => {
                self.highlit = self.highlit.saturating_sub(1);
                Ok(vec![Action::Noop])
            }
            KeyCode::Right => {
                self.highlit = (self.highlit + 1).min(self.choices.len() as u16 - 1);
                Ok(vec![Action::Noop])
            }
            _ => Ok(vec![Action::Noop]),
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect, block: Block) {
        let [prompt_rect, choices_rect] = *Layout::default()
            .margin(1)
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
            .split(rect)
        else {
            todo!()
        };
        let prompt = Paragraph::new(Cow::from(&self.prompt))
            .centered()
            .fg(DEFAULT_APP_COLORS.main_fg);
        // generate the Rects that each option will use based on constraints
        // derived from the width of each option
        let choice_rects = Layout::default()
            .margin(0)
            .direction(Direction::Horizontal)
            .constraints(self.choices.iter().map(|choice| {
                Constraint::Min(choice.width().min(rect.width as usize / self.choices.len()) as u16)
            }))
            .split(choices_rect);
        let choices: Vec<Paragraph> = self
            .choices
            .iter()
            .enumerate()
            .map(|(ind, choice)| {
                let mut paragraph = Paragraph::new(Cow::from(choice))
                    .centered()
                    .fg(DEFAULT_APP_COLORS.main_fg)
                    .bg(DEFAULT_APP_COLORS.main_bg);
                if self.highlit == ind as u16 {
                    paragraph = paragraph.reversed();
                }
                paragraph
            })
            .collect();
        // clear the rendered content behind the popup
        f.render_widget(Clear, rect);
        // render the border, clearing the background behind it
        f.render_widget(block.bg(DEFAULT_APP_COLORS.alt_bg), rect);
        // render the prompt
        f.render_widget(prompt, prompt_rect);
        // render each choice
        for (choice, choice_rect) in std::iter::zip(choices, choice_rects.iter()) {
            f.render_widget(choice, *choice_rect);
        }
    }
}
