use std::error::Error;
use std::path::{Path, PathBuf};

use crate::prompt::at_terminal;
use crate::scan::scan;
use crate::tracked::{self, Tracked};

/// Asks for folders to track, one at a time, until an empty answer, saving each to `file`
/// alongside any already tracked as soon as it is added.
pub fn setup(file: &Path) -> Result<Tracked, Box<dyn Error>> {
    if !at_terminal() {
        return Err("setup asks questions, so it needs a terminal".into());
    }
    let mut tracked = tracked::load(file)?;
    loop {
        let answer: String = dialoguer::Input::new()
            .with_prompt("Folder to track (leave empty to finish)")
            .allow_empty(true)
            .interact_text()?;
        let answer = unquote(answer.trim());
        if answer.is_empty() {
            break;
        }
        let folder = expand_home(answer);
        match tracked.add(&folder) {
            Ok(true) => {
                tracked::save(file, &tracked)?;
                let stored = tracked.folders.last().expect("just added");
                let clones = scan(stored).map(|found| found.len()).unwrap_or(0);
                println!("tracking {} ({clones} clones)", stored.display());
            }
            Ok(false) => println!("already tracked: {}", folder.display()),
            Err(message) => println!("{message}"),
        }
    }
    println!("saved to {}", file.display());
    Ok(tracked)
}

/// `answer` without one pair of surrounding double or single quotes.
fn unquote(answer: &str) -> &str {
    ['"', '\'']
        .iter()
        .find_map(|q| answer.strip_prefix(*q)?.strip_suffix(*q))
        .unwrap_or(answer)
}

/// `~` or `~/…` as a path under the home folder; anything else as given.
fn expand_home(answer: &str) -> PathBuf {
    match (answer.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            home.join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(answer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tilde_means_the_home_folder() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/code"), home.join("code"));
    }

    #[test]
    fn one_pair_of_surrounding_quotes_is_dropped() {
        assert_eq!(unquote("\"C:\\My code\""), "C:\\My code");
        assert_eq!(unquote("'~/code'"), "~/code");
        assert_eq!(unquote("\"half"), "\"half");
        assert_eq!(unquote("it's"), "it's");
    }

    #[test]
    fn a_tilde_inside_a_name_is_left_alone() {
        assert_eq!(expand_home("~code"), PathBuf::from("~code"));
        assert_eq!(expand_home("code/~"), PathBuf::from("code/~"));
    }
}
