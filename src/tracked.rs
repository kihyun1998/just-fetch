use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The folders a run with no path covers, as saved by `just-fetch setup`.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracked {
    #[serde(default)]
    pub folders: Vec<PathBuf>,
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
pub fn load(file: &Path) -> Result<Tracked, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Tracked::default()),
        Err(e) => return Err(format!("cannot read {}: {e}", file.display())),
    };
    toml::from_str(&text).map_err(|e| format!("cannot read {}: {e}", file.display()))
}

/// Writes the config file, making its folder when needed.
pub fn save(file: &Path, tracked: &Tracked) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("cannot write {}: {e}", file.display());
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
    }
    let text = toml::to_string(tracked).map_err(|e| fail(&e))?;
    std::fs::write(file, text).map_err(|e| fail(&e))
}

impl Tracked {
    /// Adds `folder` as an absolute path, unless it is already tracked under any spelling.
    /// `false` when it was.
    pub fn add(&mut self, folder: &Path) -> Result<bool, String> {
        let refuse = |why: String| format!("cannot track {}: {why}", folder.display());
        let same = folder.canonicalize().map_err(|e| refuse(e.to_string()))?;
        if !same.is_dir() {
            return Err(refuse("not a folder".to_string()));
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
            Tracked::default()
        );
    }

    #[test]
    fn folders_survive_a_save_and_load_including_backslashes() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sub/config.toml");
        let tracked = Tracked {
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
        let mut tracked = Tracked::default();

        assert!(tracked.add(&folder).unwrap());
        assert!(!tracked.add(&folder.join("sub").join("..")).unwrap());

        assert_eq!(tracked.folders, [folder]);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut tracked = Tracked::default();
        assert!(tracked.add(&tmp.path().join("missing")).is_err());
        assert!(tracked.folders.is_empty());
    }

    #[test]
    fn a_file_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("notes.txt");
        std::fs::write(&file, "").unwrap();
        let mut tracked = Tracked::default();
        assert!(tracked.add(&file).unwrap_err().contains("not a folder"));
        assert!(tracked.folders.is_empty());
    }

    #[test]
    fn a_broken_file_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("config.toml");
        std::fs::write(&file, "folders = [unclosed").unwrap();
        assert!(load(&file).unwrap_err().contains("config.toml"));
    }
}
