use ratatui::crossterm::event::{Event, KeyEvent};

/// Actions to be done by some component or by the app if returned
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Noop,
    Quit,
    KeyEvent(KeyEvent),
    OtherEvent(Event),
    ChangeEditCommand,
    ChangeSelectedTable,
    NotifyCompletion,
    Refresh,
    RevertEditHighlight,
    RevertEditSelection,
    RevertToMain,
    HighlightChanged,
    SelectionChanged,
    VeryLoudWrongBuzzer,
}

/// Error for unhandled actions
#[derive(Debug, Clone)]
pub struct UnhandledActionError {
    action: Action,
}

impl UnhandledActionError {
    pub fn new(action: Action) -> UnhandledActionError {
        UnhandledActionError { action }
    }
}

impl std::error::Error for UnhandledActionError {}

impl std::fmt::Display for UnhandledActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trying to handle unhandled event: {:?}", self.action)
    }
}
