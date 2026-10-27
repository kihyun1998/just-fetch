use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::git::{self, GitMissing};
use crate::status::{self, LocalState};

/// A clone found under the scan root, with its local state or why git could not read it.
#[derive(Debug)]
pub struct FoundClone {
    pub name: String,
    pub path: PathBuf,
    pub state: Result<LocalState, String>,
}

#[derive(Debug)]
pub enum ScanError {
    /// The scan root does not exist, is not a folder, or cannot be listed.
    Unreadable(PathBuf, io::Error),
    GitMissing(GitMissing),
}

impl From<GitMissing> for ScanError {
    fn from(missing: GitMissing) -> Self {
        ScanError::GitMissing(missing)
    }
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Unreadable(path, error) => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            ScanError::GitMissing(missing) => missing.fmt(f),
        }
    }
}

impl std::error::Error for ScanError {}

/// Lists the clone at `root`, or the clones directly under it, in name order.
pub fn scan(root: &Path) -> Result<Vec<FoundClone>, ScanError> {
    let clones = if is_clone(root) {
        vec![root.to_path_buf()]
    } else {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
            .map_err(|e| ScanError::Unreadable(root.to_path_buf(), e))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.is_dir() && is_clone(path))
            .collect();
        dirs.sort();
        dirs
    };
    clones
        .into_iter()
        .map(|path| {
            let state = git::split_missing(status::read(&path))?;
            Ok(FoundClone {
                name: display_name(&path),
                path,
                state,
            })
        })
        .collect()
}

/// A `.git` directory, or the `.git` file a worktree or submodule has.
fn is_clone(path: &Path) -> bool {
    path.join(".git").exists()
}

fn display_name(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    resolved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
