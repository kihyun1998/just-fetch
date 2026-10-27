use std::path::Path;

use crate::fetch::{Fetch, Refreshed};
use crate::git::{self, GitMissing};
use crate::scan;
use crate::status::CloneStatus;

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
/// uncommitted in tracked files.
pub fn pullable(refreshed: &Refreshed) -> Result<(), Blocked> {
    let Ok(state) = &refreshed.clone.state else {
        return Err(Blocked::Unreadable);
    };
    if matches!(refreshed.fetch, Fetch::Failed(_)) {
        return Err(Blocked::FetchFailed);
    }
    match state.clone_status() {
        CloneStatus::Behind(_) if state.dirty => Err(Blocked::Dirty),
        CloneStatus::Behind(_) => Ok(()),
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
pub enum Pulled {
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

pub fn plan(clone: &Path) -> Result<Result<Planned, String>, GitMissing> {
    Ok(
        git::run(clone, &["rev-parse", "HEAD", "@{upstream}"])?.and_then(|out| {
            match out.split_whitespace().collect::<Vec<_>>().as_slice() {
                [head, upstream] => Ok(Planned {
                    head: head.to_string(),
                    upstream: upstream.to_string(),
                }),
                _ => Err(format!("unexpected rev-parse output: {out:?}")),
            }
        }),
    )
}

/// Moves `clone`'s branch to the upstream commit in `planned`, only if that is a
/// fast-forward and the clone is still clean and at the planned HEAD. The clone's own hooks
/// run.
pub fn fast_forward(clone: &Path, planned: &Planned) -> Result<Pulled, GitMissing> {
    let state = match scan::local_state(clone)? {
        Ok(state) => state,
        Err(message) => return Ok(Pulled::Failed(message)),
    };
    if state.dirty {
        return Ok(Pulled::Changed("now dirty".to_string()));
    }
    match git::run(clone, &["rev-parse", "HEAD"])? {
        Ok(head) if head.trim() == planned.head => {}
        Ok(_) => return Ok(Pulled::Changed("HEAD moved".to_string())),
        Err(message) => return Ok(Pulled::Failed(message)),
    }
    Ok(
        match git::run_with_clone_hooks(
            clone,
            &["merge", "--ff-only", "--quiet", &planned.upstream],
        )? {
            Ok(_) => Pulled::Updated,
            Err(message) => Pulled::Failed(message),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::scan::FoundClone;
    use crate::status;

    fn clone(porcelain: &str, fetch: Fetch) -> Refreshed {
        Refreshed {
            clone: FoundClone {
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
    fn a_clean_behind_clone_is_pullable() {
        let r = clone(&tracking("# branch.ab +0 -2\n", ""), Fetch::Fetched);
        assert_eq!(pullable(&r), Ok(()));
    }

    #[test]
    fn untracked_files_alone_do_not_block() {
        let r = clone(
            &tracking("# branch.ab +0 -2\n", "? notes.txt\n"),
            Fetch::Fetched,
        );
        assert_eq!(pullable(&r), Ok(()));
    }

    #[test]
    fn a_dirty_behind_clone_is_blocked_as_dirty() {
        let r = clone(&tracking("# branch.ab +0 -2\n", DIRTY), Fetch::Fetched);
        assert_eq!(pullable(&r), Err(Blocked::Dirty));
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
            assert_eq!(pullable(&r), Err(reason), "{ab:?}");
        }
        let lonely = clone("# branch.oid 374dd5d\n# branch.head main\n", Fetch::Skipped);
        assert_eq!(pullable(&lonely), Err(Blocked::NoUpstream));
        let unborn = clone(
            "# branch.oid (initial)\n# branch.head main\n# branch.upstream origin/main\n",
            Fetch::Fetched,
        );
        assert_eq!(pullable(&unborn), Err(Blocked::NoCommits));
    }

    #[test]
    fn a_failed_fetch_blocks_even_a_behind_clone() {
        let r = clone(
            &tracking("# branch.ab +0 -2\n", ""),
            Fetch::Failed("no".into()),
        );
        assert_eq!(pullable(&r), Err(Blocked::FetchFailed));
    }

    #[test]
    fn a_clone_git_could_not_read_is_blocked() {
        let mut r = clone(&tracking("# branch.ab +0 -2\n", ""), Fetch::Skipped);
        r.clone.state = Err("broken".into());
        assert_eq!(pullable(&r), Err(Blocked::Unreadable));
    }
}
