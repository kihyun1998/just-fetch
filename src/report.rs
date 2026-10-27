use crate::fetch::{Fetch, Refreshed};
use crate::predict::Prediction;
use crate::status::{CloneStatus, Head, LocalState};

/// The table: one row per clone, names padded to the longest.
pub fn render(refreshed: &[Refreshed], predictions: &[Option<Prediction>]) -> String {
    let width = refreshed
        .iter()
        .map(|r| r.clone.name.chars().count())
        .max()
        .unwrap_or(0);
    refreshed
        .iter()
        .zip(predictions)
        .map(|(r, prediction)| {
            let detail = match &r.clone.state {
                Ok(state) => describe(state, &r.fetch, prediction.as_ref()),
                Err(message) => format!("error: {}", one_line(message)),
            };
            format!("{:<width$}  {detail}\n", r.clone.name)
        })
        .collect()
}

fn describe(state: &LocalState, fetch: &Fetch, prediction: Option<&Prediction>) -> String {
    let head = match &state.head {
        Head::Branch(name) => name.clone(),
        Head::Unborn(name) => format!("{name} (no commits)"),
        Head::Detached => "(detached)".to_string(),
    };
    let mut parts = vec![head];
    if let Some(upstream) = &state.upstream {
        parts.push(format!("-> {upstream}"));
    }
    match fetch {
        Fetch::Failed(message) => parts.push(format!("fetch failed: {}", one_line(message))),
        Fetch::Skipped | Fetch::Fetched => parts.extend(standing(&state.clone_status())),
    }
    parts.extend(prediction.map(foretell));
    if state.dirty {
        parts.push("dirty".to_string());
    }
    if state.untracked > 0 {
        parts.push(format!("{} untracked", state.untracked));
    }
    parts.join("  ")
}

/// `None` for a branch with no commits, whose head already says so.
fn standing(status: &CloneStatus) -> Option<String> {
    Some(match status {
        CloneStatus::NoUpstream => "no upstream".to_string(),
        CloneStatus::NoCommits => return None,
        CloneStatus::GoneUpstream => "upstream gone".to_string(),
        CloneStatus::UpToDate => "up to date".to_string(),
        CloneStatus::Behind(n) => format!("behind {n}"),
        CloneStatus::Ahead(n) => format!("ahead {n}"),
        CloneStatus::Diverged { ahead, behind } => format!("diverged +{ahead} -{behind}"),
    })
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

/// How many conflicted files a row names before counting the rest.
const CONFLICTS_SHOWN: usize = 5;

fn foretell(prediction: &Prediction) -> String {
    match prediction {
        Prediction::Clean => "merges cleanly".to_string(),
        Prediction::Conflicts(files) => {
            let shown = files[..files.len().min(CONFLICTS_SHOWN)]
                .iter()
                .map(|name| name.replace('\n', "\\n"))
                .collect::<Vec<_>>()
                .join(", ");
            match files.len().saturating_sub(CONFLICTS_SHOWN) {
                0 => format!("conflict: {shown}"),
                more => format!("conflict: {shown} (+{more} more)"),
            }
        }
        Prediction::Unavailable(reason) => {
            format!("conflict check unavailable: {}", one_line(reason))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::scan::FoundClone;
    use crate::status;

    use super::*;

    fn row(porcelain: &str, fetch: Fetch, prediction: Option<Prediction>) -> String {
        let refreshed = Refreshed {
            clone: FoundClone {
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
        let prediction = Prediction::Unavailable("needs git 2.38\nor later".into());
        let out = row(&porcelain, Fetch::Fetched, Some(prediction));
        assert_eq!(
            out,
            "repo  main  -> origin/main  diverged +1 -1  \
             conflict check unavailable: needs git 2.38; or later\n"
        );
    }

    #[test]
    fn a_clone_git_could_not_read_shows_its_error_on_one_line() {
        let refreshed = Refreshed {
            clone: FoundClone {
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
            clone: FoundClone {
                name: "a".into(),
                path: PathBuf::from("a"),
                state: Err("x".into()),
            },
            fetch: Fetch::Skipped,
        };
        let long = Refreshed {
            clone: FoundClone {
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
