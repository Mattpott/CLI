// #![allow(dead_code, unused)]
// define modules within this crate
mod action;
mod app;
mod autofill;
mod component;
mod config;
mod connection;
mod value;
mod wrap;

use ratatui::crossterm::execute;
use std::{error::Error, io};
// import external crates
use ratatui::{
    Terminal,
    crossterm::terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
    prelude::*,
};

use app::App;
use config::change_working_directory_to_root;

fn main() -> Result<(), Box<dyn Error>> {
    // DEBUG
    // env::set_var("RUST_BACKTRACE", "1");

    // set the current working directory to be the root Website directory
    change_working_directory_to_root();

    // set up the terminal to run
    enable_raw_mode()?; // allow for full control over the I/O processing in the terminal
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create and run the app, catching any errors it may propagate
    let result = match App::new() {
        Ok(mut app) => app.run(&mut terminal),
        Err(err) => Err(err),
    };

    // restore the terminal after the app finishes running
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // return result of running the app
    result
}
