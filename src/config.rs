use ratatui::{
    style::{palette::tailwind, Color},
    widgets::{Block, BorderType},
};
use std::{env, fs::read_dir};

// Just a file containing useful config information
use crate::component::{command_list::EditCommand, selected_table::TableMetadata};

pub const WORKING_DIRECTORY: &str = "Website";
pub const DATABASE_PATH: &str = "./data/site-content.db";

/// Changes the working directory to be the ancestor directory with the
/// base name specified by the [`WORKING_DIRECTORY`] constant defined within
/// the config.rs file
pub fn change_working_directory_to_root() {
    let mut current_dir = env::current_dir().expect("Invalid cwd, or no permissions to access cwd");
    // Find the directory specified by WORKING_DIRECTORY to root out of
    while let Ok(dir_iter) = read_dir(&current_dir) {
        let root_dir_opt = dir_iter
            .filter_map(|entry_res| match entry_res {
                Ok(entry) => {
                    if entry.path().is_dir() {
                        Some(entry)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
            .find(|entry| entry.file_name().eq(WORKING_DIRECTORY));
        if let Some(root_dir) = root_dir_opt {
            current_dir = root_dir.path();
            break;
        } else {
            current_dir.pop();
        }
    }
    if current_dir.file_name().is_none() {
        panic!("Couldn't find {} directory to root from", WORKING_DIRECTORY);
    }
    env::set_current_dir(current_dir.as_path()).expect("Failed to change working directory");
}

pub struct AppColors {
    pub main_fg: Color,
    pub main_bg: Color,
    pub alt_bg: Color,
    pub highlit_bg: Color,
    pub header_fg: Color,
    pub header_bg: Color,
    pub border_color: Color,
    pub selection_one_bg: Color,
    pub selection_two_bg: Color,
    pub selection_three_bg: Color,
    pub selection_four_bg: Color,
}

impl AppColors {
    pub fn selection_colors(&self) -> Vec<Color> {
        vec![
            self.selection_one_bg,
            self.selection_two_bg,
            self.selection_three_bg,
            self.selection_four_bg,
        ]
    }

    pub fn default_block(&self) -> Block {
        Block::bordered().border_style(self.border_color)
    }

    pub fn focused_block(&self) -> Block {
        self.default_block()
            .border_type(BorderType::QuadrantOutside)
    }
}

/// A collection of colors used by components of the app to synchronize style
/// a bit easier and allow for ease of app redesign,
///
/// Highlight style of lists and tables should just be `Style::new().reversed()`
pub const DEFAULT_APP_COLORS: AppColors = AppColors {
    main_fg: tailwind::SLATE.c200,
    main_bg: tailwind::SLATE.c950,
    alt_bg: tailwind::SLATE.c900,
    highlit_bg: tailwind::GRAY.c800,
    header_fg: tailwind::SLATE.c200,
    header_bg: tailwind::BLUE.c900,
    border_color: tailwind::CYAN.c400,
    selection_one_bg: Color::Rgb(113, 169, 247), // 113, 169, 247 | 104, 125, 211
    selection_two_bg: Color::Rgb(148, 79, 160),
    selection_three_bg: Color::Rgb(199, 102, 116),
    selection_four_bg: Color::Rgb(154, 153, 69),
};

pub fn editable_tables() -> Vec<TableMetadata> {
    vec![
        TableMetadata {
            commands: vec![
                EditCommand::Modify,
                EditCommand::Reorder,
                EditCommand::Delete,
                EditCommand::Add,
            ],
            display_name: "Category",
            table_name: "category",
            autofill_funcs: None,
        },
        TableMetadata {
            commands: vec![EditCommand::Modify, EditCommand::Delete, EditCommand::Add],
            display_name: "Document",
            table_name: "document",
            autofill_funcs: None,
        },
        TableMetadata {
            commands: vec![
                EditCommand::Modify,
                EditCommand::Reorder,
                EditCommand::Swap,
                EditCommand::Delete,
                EditCommand::Add,
            ],
            display_name: "CategoryDocument",
            table_name: "categorydocument",
            autofill_funcs: None,
        },
        TableMetadata {
            commands: vec![EditCommand::Modify],
            display_name: "Pragma Info",
            table_name: "pragma_table_info('category')",
            autofill_funcs: None,
        },
    ]
}
