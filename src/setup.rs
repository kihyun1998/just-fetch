use std::error::Error;
use std::path::{Path, PathBuf};

use crate::config::{self, Config};
use crate::lang::{self, Lang};
use crate::prompt::at_terminal;
use crate::scan::scan;
use crate::text::Text;

/// The languages setup offers, each named in itself.
const LANGUAGES: [(Lang, &str); 2] = [(Lang::En, "English"), (Lang::Ko, "한국어")];

/// Asks for the language, then for folders to track one at a time until an empty answer,
/// saving each answer to `file` alongside what is already there as soon as it is given.
pub fn setup(file: &Path) -> Result<Config, Box<dyn Error>> {
    if !at_terminal() {
        return Err(Text::SetupNeedsTerminal.to_string().into());
    }
    let mut tracked = config::load(file)?;
    let names: Vec<&str> = LANGUAGES.iter().map(|(_, name)| *name).collect();
    let now = LANGUAGES
        .iter()
        .position(|(lang, _)| *lang == lang::current())
        .unwrap_or(0);
    let chosen = dialoguer::Select::new()
        .with_prompt(Text::LanguagePrompt.to_string())
        .items(&names)
        .default(now)
        .interact()?;
    let (language, _) = LANGUAGES[chosen];
    lang::set(language);
    tracked.language = Some(language);
    config::save(file, &tracked)?;
    loop {
        let answer: String = dialoguer::Input::new()
            .with_prompt(Text::FolderToTrack.to_string())
            .allow_empty(true)
            .interact_text()?;
        let answer = unquote(answer.trim());
        if answer.is_empty() {
            break;
        }
        let folder = expand_home(answer);
        match tracked.add(&folder) {
            Ok(true) => {
                config::save(file, &tracked)?;
                let stored = tracked.folders.last().expect("just added");
                let clones = scan(stored).map(|found| found.len()).unwrap_or(0);
                println!("{}", Text::Tracking(stored.display().to_string(), clones));
            }
            Ok(false) => println!("{}", Text::AlreadyTracked(folder.display().to_string())),
            Err(message) => println!("{message}"),
        }
    }
    println!("{}", Text::SavedTo(file.display().to_string()));
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
