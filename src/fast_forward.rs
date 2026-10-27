use std::path::Path;

use crate::fetch::{NotFresh, Refreshed};
use crate::git::{self, GitError, GitMissing};
use crate::status::{self, CloneStatus};

/// Why a clone cannot be fast-forwarded.
#[derive(Debug, PartialEq, Eq)]
pub enum Blocked {
    Unreadable,
    FetchFailed,
    Dirty,
    UpToDate,
    Ahead,
    Diverged,
    NoUpstream,
    GoneUpstream,
    NoCommits,
}

/// Whether `refreshed` can be offered for a fast-forward: behind its upstream, with nothing
/// uncommitted in tracked files. `Ok` holds how many commits it is behind.
pub fn offerable(refreshed: &Refreshed) -> Result<u32, Blocked> {
    let status = refreshed
        .fresh_status()
        .map_err(|not_fresh| match not_fresh {
            NotFresh::Unreadable => Blocked::Unreadable,
            NotFresh::FetchFailed => Blocked::FetchFailed,
        })?;
    let dirty = refreshed
        .found
        .state
        .as_ref()
        .is_ok_and(|state| state.dirty);
    match status {
        CloneStatus::Behind(_) if dirty => Err(Blocked::Dirty),
        CloneStatus::Behind(behind) => Ok(behind),
        CloneStatus::UpToDate => Err(Blocked::UpToDate),
        CloneStatus::Ahead(_) => Err(Blocked::Ahead),
        CloneStatus::Diverged { .. } => Err(Blocked::Diverged),
        CloneStatus::NoUpstream => Err(Blocked::NoUpstream),
        CloneStatus::GoneUpstream => Err(Blocked::GoneUpstream),
        CloneStatus::NoCommits => Err(Blocked::NoCommits),
    }
}

/// The result of fast-forwarding one clone.
#[derive(Debug)]
pub enum Outcome {
    Updated,
    /// The clone is no longer as planned, so nothing was done; what changed.
    Changed(String),
    /// git refused, and the clone is as it was; git's message, trimmed.
    Failed(String),
}

/// The commits a fast-forward is planned between, read before a person is asked.
#[derive(Debug)]
pub struct Planned {
    pub head: String,
    pub upstream: String,
}

/// Reads the commits a fast-forward of `clone` would move between.
pub fn plan(clone: &Path) -> Result<Planned, GitError> {
    let (head, upstream) = git::head_and_upstream(clone)?;
    Ok(Planned { head, upstream })
}

/// Moves `clone`'s branch to the upstream commit in `planned`, only if that is a
/// fast-forward and the clone is still clean and at the planned HEAD. The clone's own hooks
/// run.
pub fn fast_forward(clone: &Path, planned: &Planned) -> Result<Outcome, GitMissing> {
    Ok(git::split_missing(try_fast_forward(clone, planned))?.unwrap_or_else(Outcome::Failed))
}

fn try_fast_forward(clone: &Path, planned: &Planned) -> Result<Outcome, GitError> {
    if status::read(clone)?.dirty {
        return Ok(Outcome::Changed("now dirty".to_string()));
    }
    if git::run(clone, &["rev-parse", "HEAD"])?.trim() != planned.head {
        return Ok(Outcome::Changed("HEAD moved".to_string()));
    }
    git::run_with_clone_hooks(clone, &["merge", "--ff-only", "--quiet", &planned.upstream])?;
    Ok(Outcome::Updated)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::fetch::Fetch;
    use crate::scan::FoundClone;
    use crate::status;

    fn clone(porcelain: &str, fetch: Fetch) -> Refreshed {
        Refreshed {
            found: FoundClone {
                name: "repo".into(),
                path: PathBuf::from("repo"),
                state: Ok(status::parse(porcelain)),
            },
            fetch,
        }
    }

    fn tracking(ab: &str, extra: &str) -> String {
        format!(
            "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
             # branch.head main\n\
             # branch.upstream origin/main\n{ab}{extra}"
        )
    }

    const DIRTY: &str = "1 .M N... 100644 100644 100644 e69de29 e69de29 README.md\n";

    #[test]
    fn a_clean_behind_clone_is_offerable() {
        let r = clone(&tracking("# branch.ab +0 -2\n", ""), Fetch::Fetched);
        assert_eq!(offerable(&r), Ok(2));
    }

    #[test]
    fn untracked_files_alone_do_not_block() {
        let r = clone(
            &tracking("# branch.ab +0 -2\n", "? notes.txt\n"),
            Fetch::Fetched,
        );
        assert_eq!(offerable(&r), Ok(2));
    }

    #[test]
    fn a_dirty_behind_clone_is_blocked_as_dirty() {
        let r = clone(&tracking("# branch.ab +0 -2\n", DIRTY), Fetch::Fetched);
        assert_eq!(offerable(&r), Err(Blocked::Dirty));
    }

    #[test]
    fn each_other_standing_blocks_with_its_own_reason() {
        let cases = [
            ("# branch.ab +0 -0\n", Blocked::UpToDate),
            ("# branch.ab +1 -0\n", Blocked::Ahead),
            ("# branch.ab +1 -1\n", Blocked::Diverged),
            ("", Blocked::GoneUpstream),
        ];
        for (ab, reason) in cases {
            let r = clone(&tracking(ab, ""), Fetch::Fetched);
            assert_eq!(offerable(&r), Err(reason), "{ab:?}");
        }
        let lonely = clone("# branch.oid 374dd5d\n# branch.head main\n", Fetch::Skipped);
        assert_eq!(offerable(&lonely), Err(Blocked::NoUpstream));
        let unborn = clone(
            "# branch.oid (initial)\n# branch.head main\n# branch.upstream origin/main\n",
            Fetch::Fetched,
        );
        assert_eq!(offerable(&unborn), Err(Blocked::NoCommits));
    }

    #[test]
    fn a_failed_fetch_blocks_even_a_behind_clone() {
        let r = clone(
            &tracking("# branch.ab +0 -2\n", ""),
            Fetch::Failed("no".into()),
        );
        assert_eq!(offerable(&r), Err(Blocked::FetchFailed));
    }

    #[test]
    fn a_clone_git_could_not_read_is_blocked() {
        let mut r = clone(&tracking("# branch.ab +0 -2\n", ""), Fetch::Skipped);
        r.found.state = Err("broken".into());
        assert_eq!(offerable(&r), Err(Blocked::Unreadable));
    }
}
