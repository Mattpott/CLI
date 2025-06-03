use glob::{MatchOptions, glob_with};

use crate::config::PHP_PATH;

pub type AutoFillFn = fn(&str) -> Option<String>;

/// Provides with an option for the filepath directing to an HTML file
/// associated with a PHP file stored in the pre-defined `PHP_PATH` folder.
pub fn html_filepath(content: &str) -> Option<String> {
    if content.is_empty() {
        return None;
    }
    let options = MatchOptions {
        case_sensitive: false,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    let search_path = format!("{}{}*", PHP_PATH, content);
    let paths = match glob_with(&search_path, options) {
        Ok(p) => p,
        Err(_) => return None,
    };
    let mut suggestion: Option<String> = None;
    // grab the first globbed path
    if let Some(path) = paths.flatten().next() {
        let suggested_path = if path.is_dir() {
            path
        } else {
            path.with_extension("html")
        };
        if let Some(suggested_string) = suggested_path.to_str() {
            let lead_dirname = if let Some(stripped) = PHP_PATH.strip_prefix("./") {
                stripped
            } else {
                PHP_PATH
            };
            // remove the leading, already present content
            suggestion = Some(suggested_string[(lead_dirname.len() + content.len())..].to_string());
        }
    }
    suggestion
}
