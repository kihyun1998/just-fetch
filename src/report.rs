use crate::fetch::{Fetch, Refreshed};
use crate::predict::Prediction;
use crate::status::{CloneStatus, Head, LocalState};
use crate::text::Text;

/// The table: one row per clone, names padded to the longest.
pub fn render(refreshed: &[Refreshed], predictions: &[Option<Prediction>]) -> String {
    let width = refreshed
        .iter()
        .map(|r| r.found.name.chars().count())
        .max()
        .unwrap_or(0);
    refreshed
        .iter()
        .zip(predictions)
        .map(|(r, prediction)| {
            let detail = match &r.found.state {
                Ok(state) => describe(state, &r.fetch, prediction.as_ref()),
                Err(message) => Text::RowError(one_line(message)).to_string(),
            };
            format!("{:<width$}  {detail}\n", r.found.name)
        })
        .collect()
}

fn describe(state: &LocalState, fetch: &Fetch, prediction: Option<&Prediction>) -> String {
    let head = match &state.head {
        Head::Branch(name) => name.clone(),
        Head::Unborn(name) => Text::NoCommitsHead(name.clone()).to_string(),
        Head::Detached => Text::Detached.to_string(),
    };
    let mut parts = vec![head];
    if let Some(upstream) = &state.upstream {
        parts.push(format!("-> {upstream}"));
    }
    match fetch {
        Fetch::Failed(message) => parts.push(Text::FetchFailed(one_line(message)).to_string()),
        Fetch::Refused(key) => parts.push(Text::FetchRefused(key.clone()).to_string()),
        Fetch::Skipped | Fetch::Fetched => parts.extend(standing(&state.clone_status())),
    }
    parts.extend(prediction.map(foretell));
    if state.dirty {
        parts.push(Text::Dirty.to_string());
    }
    if state.untracked > 0 {
        parts.push(Text::Untracked(state.untracked).to_string());
    }
    parts.join("  ")
}

/// `None` for a branch with no commits, whose head already says so.
fn standing(status: &CloneStatus) -> Option<String> {
    let text = match status {
        CloneStatus::NoUpstream => Text::NoUpstream,
        CloneStatus::NoCommits => return None,
        CloneStatus::GoneUpstream => Text::UpstreamGone,
        CloneStatus::UpToDate => Text::UpToDate,
        CloneStatus::Behind(n) => Text::Behind(*n),
        CloneStatus::Ahead(n) => Text::Ahead(*n),
        CloneStatus::Diverged { ahead, behind } => Text::Diverged(*ahead, *behind),
    };
    Some(text.to_string())
}

/// git's message with its lines joined, so it stays on its clone's row.
pub fn one_line(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn foretell(prediction: &Prediction) -> String {
    let text = match prediction {
        Prediction::Clean => Text::MergesCleanly,
        Prediction::Conflicts(files) => Text::Conflict(files.clone()),
        Prediction::NeedsNewerGit => Text::ConflictCheckNeedsGit,
        Prediction::Unavailable(reason) => Text::ConflictCheckUnavailable(one_line(reason)),
    };
    text.to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::scan::FoundClone;
    use crate::status;

    use super::*;

    fn row(porcelain: &str, fetch: Fetch, prediction: Option<Prediction>) -> String {
        let refreshed = Refreshed {
            found: FoundClone {
                name: "repo".into(),
                path: PathBuf::from("repo"),
                state: Ok(status::parse(porcelain)),
            },
            fetch,
        };
        render(&[refreshed], &[prediction])
    }

    const TRACKING: &str = "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
                            # branch.head main\n\
                            # branch.upstream origin/main\n";

    #[test]
    fn a_detached_head_is_named_as_detached() {
        let out = row(
            "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n# branch.head (detached)\n",
            Fetch::Skipped,
            None,
        );
        assert_eq!(out, "repo  (detached)  no upstream\n");
    }

    #[test]
    fn a_branch_without_commits_says_so_once() {
        let out = row(
            "# branch.oid (initial)\n# branch.head main\n# branch.upstream origin/main\n",
            Fetch::Fetched,
            None,
        );
        assert_eq!(out, "repo  main (no commits)  -> origin/main\n");
    }

    #[test]
    fn untracked_files_are_counted_after_dirty() {
        let porcelain = format!(
            "{TRACKING}# branch.ab +0 -0\n\
             1 .M N... 100644 100644 100644 e69de29 e69de29 README.md\n\
             ? a.txt\n? b.txt\n"
        );
        let out = row(&porcelain, Fetch::Fetched, None);
        assert_eq!(
            out,
            "repo  main  -> origin/main  up to date  dirty  2 untracked\n"
        );
    }

    #[test]
    fn an_unavailable_prediction_says_why_on_one_line() {
        let porcelain = format!("{TRACKING}# branch.ab +1 -1\n");
        let prediction = Prediction::Unavailable("fatal: bad\nobject".into());
        let out = row(&porcelain, Fetch::Fetched, Some(prediction));
        assert_eq!(
            out,
            "repo  main  -> origin/main  diverged +1 -1  \
             conflict check unavailable: fatal: bad; object\n"
        );
    }

    #[test]
    fn a_clone_git_could_not_read_shows_its_error_on_one_line() {
        let refreshed = Refreshed {
            found: FoundClone {
                name: "broken".into(),
                path: PathBuf::from("broken"),
                state: Err("fatal: not a git repository\nhint: check it".into()),
            },
            fetch: Fetch::Skipped,
        };
        let out = render(&[refreshed], &[None]);
        assert_eq!(
            out,
            "broken  error: fatal: not a git repository; hint: check it\n"
        );
    }

    #[test]
    fn names_are_padded_to_the_longest() {
        let short = Refreshed {
            found: FoundClone {
                name: "a".into(),
                path: PathBuf::from("a"),
                state: Err("x".into()),
            },
            fetch: Fetch::Skipped,
        };
        let long = Refreshed {
            found: FoundClone {
                name: "longer".into(),
                path: PathBuf::from("longer"),
                state: Err("y".into()),
            },
            fetch: Fetch::Skipped,
        };
        assert_eq!(
            render(&[short, long], &[None, None]),
            "a       error: x\nlonger  error: y\n"
        );
    }
}
