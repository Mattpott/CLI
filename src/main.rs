// #![allow(dead_code, unused)]
// define modules within this crate
mod action;
mod app;
mod component;
mod config;
mod connection;
mod value;
mod wrap;

// import official rust-lang crates
use getopts::Options;
use glob::glob;
use ratatui::crossterm::execute;
use std::{
    env,
    error::Error,
    fs::{create_dir_all, File},
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
};
// import external crates
use ratatui::{
    crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    prelude::*,
    Terminal,
};

use app::App;
use config::change_working_directory_to_root;

fn main() -> Result<(), Box<dyn Error>> {
    // DEBUG
    // env::set_var("RUST_BACKTRACE", "1");

    // set the current working directory to be the root Website directory
    change_working_directory_to_root();

    // handle commandline arguments and do not run the app if files get updated
    if commandline() {
        return Ok(());
    }

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

fn commandline() -> bool {
    // set up the commandline options allowed
    let args: Vec<String> = env::args().skip(1).collect();
    let mut opts: Options = Options::new();
    opts.optopt("s", "single", "single file", "FILE");
    opts.optflag(
        "u",
        "update",
        "update the content of all files, or a single file if s flag passed as well",
    );
    let opt_matches = match opts.parse(&args) {
        Ok(m) => m,
        Err(e) => panic!("{}", e.to_string()),
    };
    if opt_matches.opt_present("u") {
        if let Some(intended_file) = opt_matches.opt_str("s") {
            // update just the passed file
            match run_php(PathBuf::from(intended_file)) {
                Ok(msg) => println!("{msg}"),
                Err(e) => eprintln!("{:?}", e),
            }
        } else {
            run_php_all(); // update all as s wasn't passed
        }
        return true;
    }
    false
    // opt_matches.free.is_empty(); //^ `matches.free` contains all the arguments that are not options.
}

fn run_php(input_path: PathBuf) -> Result<String, Box<dyn Error>> {
    // create a path to an HTML file with the same name
    // remove the root php directory which is included in the glob
    let output_path = PathBuf::from_iter(input_path.components().skip(1)).with_extension("html");
    // spawn a php process to run the code, or bubble up the returned error
    let php_child: Child = Command::new("php")
        .arg(&input_path)
        .stdout(Stdio::piped())
        .spawn()?;

    // ensure that all directories of the destination path are created
    if let Some(path_parent) = output_path.parent() {
        if !path_parent.exists() {
            // create the associated directories for the file
            create_dir_all(path_parent)?; // TODO: may need to match this for the error
        }
    }
    let mut output_file: File = File::create(&output_path)?;

    let mut child_output = php_child
        .stdout
        .ok_or("Failed to read the child process' output")?;
    let result = io::copy(&mut child_output, &mut output_file);
    let output_display = output_path.display();
    match result {
        Ok(num_bytes) => Ok(format!(
            "Succeeded write of {num_bytes} bytes to {output_display}"
        )),
        Err(_) => Err(format!("Failed within write to {output_display}").into()),
    }
}

fn run_php_all() {
    let mut count: u16 = 0;
    // iterates over all files in all directories within the php directory
    for php_file in glob("php/**/*.php").expect("Failed to read glob pattern") {
        match php_file {
            Ok(path) => {
                // ignore all php files in any directory with the name utils
                // done this way as there is no way to exclude within the glob seemingly
                if !path
                    .ancestors()
                    .any(|ancestor| ancestor.file_name().is_some_and(|name| name == "utils"))
                {
                    count += 1;
                    match run_php(path) {
                        Ok(msg) => println!("{msg}"),
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
            }
            Err(e) => eprintln!("Glob Error: {:?}", e),
        }
    }
    if count == 0 {
        println!("No files were updated");
    } else if count == 1 {
        println!("1 file was updated");
    } else {
        println!("{} files were updated", count);
    }
}
