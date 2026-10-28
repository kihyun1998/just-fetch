use std::path::{Path, PathBuf};

use crate::fetch::Refreshed;
use crate::git::{self, GitError, GitMissing};
use crate::status::CloneStatus;
use crate::text::Text;

/// What a trial merge of a diverged clone with its upstream says would happen.
#[derive(Debug, PartialEq, Eq)]
pub enum Prediction {
    Clean,
    /// The files that would be left in conflict, in git's order.
    Conflicts(Vec<String>),
    /// No prediction could be made: the installed git is older than [`MINIMUM_GIT`].
    NeedsNewerGit,
    /// No prediction could be made, and git's reason.
    Unavailable(String),
}

/// A git release, compared by major then minor number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
}

/// The oldest git whose `merge-tree` can predict.
pub const MINIMUM_GIT: GitVersion = GitVersion {
    major: 2,
    minor: 38,
};

/// Reads `git version` output: `git version 2.50.1 (Apple Git-155)`, `git version 2.56.0.windows.2`.
pub fn parse_version(output: &str) -> Option<GitVersion> {
    let number = output
        .trim()
        .strip_prefix("git version ")?
        .split(' ')
        .next()?;
    let mut parts = number.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some(GitVersion { major, minor })
}

/// Reads `merge-tree --write-tree --name-only --no-messages -z` output: the tree, then each
/// conflicted file once.
pub fn parse_conflicts(output: &str) -> Vec<String> {
    output
        .split('\0')
        .skip(1)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// The installed git's version, `None` when its output cannot be read.
pub fn git_version() -> Result<Option<GitVersion>, GitMissing> {
    Ok(git::split_missing(git::run(Path::new("."), &["version"]))?
        .ok()
        .and_then(|out| parse_version(&out)))
}

/// Trial-merges `clone`'s upstream into its HEAD. The merge runs in a scratch repository
/// that reads the clone's objects, so the clone's refs, index, working tree, object store,
/// config and attributes play no part and are left as they were.
pub fn predict(clone: &Path, version: Option<GitVersion>) -> Result<Prediction, GitMissing> {
    if version.is_none_or(|v| v < MINIMUM_GIT) {
        return Ok(Prediction::NeedsNewerGit);
    }
    Ok(git::split_missing(try_predict(clone))?.unwrap_or_else(Prediction::Unavailable))
}

fn try_predict(clone: &Path) -> Result<Prediction, GitError> {
    let (ours, theirs) = git::head_and_upstream(clone)?;
    let scratch = Scratch::create(clone)?;
    let ran = git::run_for_exit_code(
        scratch.path(),
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            "-z",
            &ours,
            &theirs,
        ],
    )?;
    Ok(match ran.code {
        Some(0) => Prediction::Clean,
        Some(1) => Prediction::Conflicts(parse_conflicts(&ran.stdout)),
        _ => Prediction::Unavailable(ran.stderr),
    })
}

/// An empty bare repository, removed when dropped, whose object store borrows the clone's.
struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    fn create(clone: &Path) -> Result<Self, GitError> {
        let probe = git::run(
            clone,
            &[
                "rev-parse",
                "--show-object-format",
                "--path-format=absolute",
                "--git-path",
                "objects",
            ],
        )?;
        let (format, objects) = match probe.lines().collect::<Vec<_>>().as_slice() {
            [format, objects] => (format.to_string(), PathBuf::from(objects)),
            _ => {
                return Err(GitError::Failed(
                    Text::UnexpectedGitOutput(format!("{probe:?}")).to_string(),
                ));
            }
        };
        let dir = tempfile::Builder::new()
            .prefix("just-fetch-predict-")
            .tempdir()?;
        git::run(
            dir.path(),
            &[
                "init",
                "--bare",
                "--quiet",
                &format!("--object-format={format}"),
            ],
        )?;
        let alternates = dir.path().join("objects").join("info").join("alternates");
        std::fs::write(&alternates, format!("{}\n", objects.display()))?;
        Ok(Self { dir })
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// A prediction for each diverged clone, `None` for the rest.
pub fn predict_diverged(refreshed: &[Refreshed]) -> Result<Vec<Option<Prediction>>, GitMissing> {
    let is_diverged = |r: &Refreshed| matches!(r.fresh_status(), Ok(CloneStatus::Diverged { .. }));
    let version = if refreshed.iter().any(is_diverged) {
        git_version()?
    } else {
        None
    };
    refreshed
        .iter()
        .map(|r| {
            if is_diverged(r) {
                predict(&r.found.path, version).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_apple_and_windows_versions() {
        assert_eq!(
            parse_version("git version 2.38.0\n"),
            Some(GitVersion {
                major: 2,
                minor: 38
            })
        );
        assert_eq!(
            parse_version("git version 2.50.1 (Apple Git-155)\n"),
            Some(GitVersion {
                major: 2,
                minor: 50
            })
        );
        assert_eq!(
            parse_version("git version 2.56.0.windows.2\n"),
            Some(GitVersion {
                major: 2,
                minor: 56
            })
        );
        assert_eq!(
            parse_version("git version 3.0\n"),
            Some(GitVersion { major: 3, minor: 0 })
        );
    }

    #[test]
    fn something_that_is_not_a_version_reads_as_none() {
        assert_eq!(parse_version("hello\n"), None);
        assert_eq!(parse_version("git version x.y\n"), None);
    }

    #[test]
    fn versions_compare_against_the_minimum() {
        assert!(parse_version("git version 2.37.7\n").unwrap() < MINIMUM_GIT);
        assert!(parse_version("git version 2.38.0\n").unwrap() >= MINIMUM_GIT);
        assert!(parse_version("git version 3.0.0\n").unwrap() >= MINIMUM_GIT);
    }

    #[test]
    fn conflicted_names_follow_the_tree() {
        // Recorded from git 2.50.1 on a merge where f.txt and g.txt both conflict.
        let out = "99f7929862204e31aa1432440eb47ce5ce4962c7\0f.txt\0g.txt\0";
        assert_eq!(parse_conflicts(out), ["f.txt", "g.txt"]);
    }

    #[test]
    fn a_clean_merge_has_only_the_tree() {
        assert!(parse_conflicts("500d30ebdca78f45428d029d18d7beebcae2b1ae\0").is_empty());
    }

    #[test]
    fn names_with_spaces_and_newlines_survive() {
        let out = "99f7929862204e31aa1432440eb47ce5ce4962c7\0my notes.md\0a\nb\0";
        assert_eq!(parse_conflicts(out), ["my notes.md", "a\nb"]);
    }
}
