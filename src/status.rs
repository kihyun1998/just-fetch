use std::path::Path;

use crate::git::{self, GitError};

/// Where a clone's HEAD points.
#[derive(Debug, PartialEq, Eq)]
pub enum Head {
    Branch(String),
    /// A branch with no commits yet.
    Unborn(String),
    Detached,
}

/// What a clone's own working tree and branch look like, without asking the remote.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalState {
    pub head: Head,
    pub upstream: Option<String>,
    pub dirty: bool,
    pub untracked: usize,
    /// Commits the clone has that its upstream lacks, and the reverse, as git last saw the
    /// upstream. Absent when there is no upstream, or no commit to compare.
    pub ahead_behind: Option<(u32, u32)>,
}

/// How a clone stands against its upstream.
#[derive(Debug, PartialEq, Eq)]
pub enum CloneStatus {
    NoUpstream,
    /// A branch with no commits yet, so there is nothing to compare.
    NoCommits,
    GoneUpstream,
    UpToDate,
    Behind(u32),
    Ahead(u32),
    Diverged {
        ahead: u32,
        behind: u32,
    },
}

impl LocalState {
    pub fn clone_status(&self) -> CloneStatus {
        if self.upstream.is_none() {
            return CloneStatus::NoUpstream;
        }
        if matches!(self.head, Head::Unborn(_)) {
            return CloneStatus::NoCommits;
        }
        match self.ahead_behind {
            None => CloneStatus::GoneUpstream,
            Some((0, 0)) => CloneStatus::UpToDate,
            Some((0, behind)) => CloneStatus::Behind(behind),
            Some((ahead, 0)) => CloneStatus::Ahead(ahead),
            Some((ahead, behind)) => CloneStatus::Diverged { ahead, behind },
        }
    }
}

/// Reads `clone`'s local state with one `git status`.
pub fn read(clone: &Path) -> Result<LocalState, GitError> {
    git::run(
        clone,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ],
    )
    .map(|out| parse(&out))
}

/// Reads the output of `git status --porcelain=v2 --branch`.
pub fn parse(porcelain: &str) -> LocalState {
    let mut unborn = false;
    let mut branch = None;
    let mut upstream = None;
    let mut dirty = false;
    let mut untracked = 0;
    let mut ahead_behind = None;
    for line in porcelain.lines() {
        if line == "# branch.oid (initial)" {
            unborn = true;
        } else if let Some(name) = line.strip_prefix("# branch.head ") {
            branch = (name != "(detached)").then(|| name.to_string());
        } else if let Some(name) = line.strip_prefix("# branch.upstream ") {
            upstream = Some(name.to_string());
        } else if let Some(counts) = line.strip_prefix("# branch.ab ") {
            ahead_behind = parse_ahead_behind(counts);
        } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
            dirty = true;
        } else if line.starts_with("? ") {
            untracked += 1;
        }
    }
    let head = match branch {
        Some(name) if unborn => Head::Unborn(name),
        Some(name) => Head::Branch(name),
        None => Head::Detached,
    };
    LocalState {
        head,
        upstream,
        dirty,
        untracked,
        ahead_behind,
    }
}

/// `+<ahead> -<behind>`.
fn parse_ahead_behind(counts: &str) -> Option<(u32, u32)> {
    let (ahead, behind) = counts.split_once(' ')?;
    Some((
        ahead.strip_prefix('+')?.parse().ok()?,
        behind.strip_prefix('-')?.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_with_upstream_and_clean_tree() {
        let out = "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
                   # branch.head main\n\
                   # branch.upstream origin/main\n\
                   # branch.ab +0 -0\n";
        assert_eq!(
            parse(out),
            LocalState {
                head: Head::Branch("main".into()),
                upstream: Some("origin/main".into()),
                dirty: false,
                untracked: 0,
                ahead_behind: Some((0, 0)),
            }
        );
    }

    #[test]
    fn detached_head_has_no_branch() {
        let out = "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
                   # branch.head (detached)\n";
        assert_eq!(parse(out).head, Head::Detached);
    }

    #[test]
    fn branch_without_commits_is_unborn() {
        let out = "# branch.oid (initial)\n# branch.head main\n";
        assert_eq!(parse(out).head, Head::Unborn("main".into()));
    }

    #[test]
    fn no_upstream_line_means_no_upstream() {
        let out = "# branch.oid (initial)\n# branch.head main\n";
        assert_eq!(parse(out).upstream, None);
    }

    #[test]
    fn changed_tracked_file_makes_dirty() {
        let out = "# branch.oid (initial)\n# branch.head main\n\
                   1 .M N... 100644 100644 100644 e69de29 e69de29 README.md\n";
        assert!(parse(out).dirty);
    }

    #[test]
    fn renamed_and_unmerged_entries_make_dirty() {
        let renamed = "# branch.head main\n\
                       2 R. N... 100644 100644 100644 e69de29 e69de29 R100 new.md\told.md\n";
        let unmerged = "# branch.head main\n\
                        u UU N... 100644 100644 100644 100644 e69de29 e69de29 e69de29 a.md\n";
        assert!(parse(renamed).dirty);
        assert!(parse(unmerged).dirty);
    }

    #[test]
    fn untracked_files_alone_are_counted_but_not_dirty() {
        let out = "# branch.head main\n? notes.txt\n? scratch/\n";
        let state = parse(out);
        assert!(!state.dirty);
        assert_eq!(state.untracked, 2);
    }

    fn status_of(ab_line: &str) -> CloneStatus {
        let out = format!(
            "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
             # branch.head main\n\
             # branch.upstream origin/main\n{ab_line}"
        );
        parse(&out).clone_status()
    }

    #[test]
    fn ahead_and_behind_counts_decide_the_status() {
        assert_eq!(status_of("# branch.ab +0 -0\n"), CloneStatus::UpToDate);
        assert_eq!(status_of("# branch.ab +0 -3\n"), CloneStatus::Behind(3));
        assert_eq!(status_of("# branch.ab +2 -0\n"), CloneStatus::Ahead(2));
        assert_eq!(
            status_of("# branch.ab +2 -3\n"),
            CloneStatus::Diverged {
                ahead: 2,
                behind: 3
            }
        );
    }

    #[test]
    fn an_upstream_with_no_counts_is_gone() {
        assert_eq!(status_of(""), CloneStatus::GoneUpstream);
    }

    #[test]
    fn no_upstream_line_is_no_upstream_status() {
        let out = "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n# branch.head main\n";
        assert_eq!(parse(out).clone_status(), CloneStatus::NoUpstream);
    }

    #[test]
    fn an_unborn_branch_with_an_upstream_has_no_commits_not_a_gone_upstream() {
        let out = "# branch.oid (initial)\n# branch.head main\n# branch.upstream origin/main\n";
        assert_eq!(parse(out).clone_status(), CloneStatus::NoCommits);
    }
}
