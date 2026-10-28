use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

use crate::lang::Lang;
use crate::text::Text;

/// What `just-fetch setup` saves: the folders a run with no path covers, and the language.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "known_language"
    )]
    pub language: Option<Lang>,
    #[serde(default)]
    pub folders: Vec<PathBuf>,
}

/// A saved language just-fetch speaks, or `None` for any other value.
fn known_language<'de, D: Deserializer<'de>>(value: D) -> Result<Option<Lang>, D::Error> {
    Ok(match Option::<String>::deserialize(value)?.as_deref() {
        Some("en") => Some(Lang::En),
        Some("ko") => Some(Lang::Ko),
        _ => None,
    })
}

/// The variable that names another folder for the config file.
pub const CONFIG_DIR_VAR: &str = "JUST_FETCH_CONFIG_DIR";

/// Where the config file lives: `JUST_FETCH_CONFIG_DIR`, else the OS config folder.
pub fn config_file() -> Option<PathBuf> {
    let dir = match std::env::var_os(CONFIG_DIR_VAR) {
        Some(dir) => PathBuf::from(dir),
        None => dirs::config_dir()?.join("just-fetch"),
    };
    Some(dir.join("config.toml"))
}

/// Reads the config file; a file that does not exist yet is an empty list.
pub fn load(file: &Path) -> Result<Config, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => {
            return Err(Text::CannotRead(file.display().to_string(), e.to_string()).to_string());
        }
    };
    toml::from_str(&text)
        .map_err(|e| Text::CannotRead(file.display().to_string(), e.to_string()).to_string())
}

/// Writes the config file, making its folder when needed.
pub fn save(file: &Path, tracked: &Config) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| {
        Text::CannotWrite(file.display().to_string(), e.to_string()).to_string()
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
    }
    let text = toml::to_string(tracked).map_err(|e| fail(&e))?;
    std::fs::write(file, text).map_err(|e| fail(&e))
}

impl Config {
    /// Adds `folder` as an absolute path, unless it is already tracked under any spelling.
    /// `false` when it was.
    pub fn add(&mut self, folder: &Path) -> Result<bool, String> {
        let refuse = |why: String| Text::CannotTrack(folder.display().to_string(), why).to_string();
        let same = folder.canonicalize().map_err(|e| refuse(e.to_string()))?;
        if !same.is_dir() {
            return Err(refuse(Text::NotAFolder.to_string()));
        }
        if self
            .folders
            .iter()
            .any(|tracked| tracked.canonicalize().is_ok_and(|it| it == same))
        {
            return Ok(false);
        }
        self.folders
            .push(std::path::absolute(folder).map_err(|e| refuse(e.to_string()))?);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_an_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&tmp.path().join("nope/config.toml")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn folders_survive_a_save_and_load_including_backslashes() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sub/config.toml");
        let tracked = Config {
            language: None,
            folders: vec![
                PathBuf::from("/srv/code"),
                PathBuf::from(r"C:\Users\me\code"),
            ],
        };

        save(&file, &tracked).unwrap();

        assert_eq!(load(&file).unwrap(), tracked);
    }

    #[test]
    fn a_folder_is_stored_as_given_and_once_however_it_is_spelled() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("code");
        std::fs::create_dir_all(folder.join("sub")).unwrap();
        let mut tracked = Config::default();

        assert!(tracked.add(&folder).unwrap());
        assert!(!tracked.add(&folder.join("sub").join("..")).unwrap());

        assert_eq!(tracked.folders, [folder]);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut tracked = Config::default();
        assert!(tracked.add(&tmp.path().join("missing")).is_err());
        assert!(tracked.folders.is_empty());
    }

    #[test]
    fn a_file_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("notes.txt");
        std::fs::write(&file, "").unwrap();
        let mut tracked = Config::default();
        assert!(tracked.add(&file).unwrap_err().contains("not a folder"));
        assert!(tracked.folders.is_empty());
    }

    #[test]
    fn an_unknown_language_is_as_if_none_were_saved() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("config.toml");
        std::fs::write(&file, "language = \"fr\"\nfolders = [\"/srv/code\"]\n").unwrap();
        let config = load(&file).unwrap();
        assert_eq!(config.language, None);
        assert_eq!(config.folders, [PathBuf::from("/srv/code")]);

        std::fs::write(&file, "language = \"ko\"\n").unwrap();
        assert_eq!(load(&file).unwrap().language, Some(Lang::Ko));
    }

    #[test]
    fn a_broken_file_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("config.toml");
        std::fs::write(&file, "folders = [unclosed").unwrap();
        assert!(load(&file).unwrap_err().contains("config.toml"));
    }
}
