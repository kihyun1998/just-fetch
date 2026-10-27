use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use just_fetch::fetch::{Fetch, Refreshed, fetch_all};
use just_fetch::git::GitMissing;
use just_fetch::predict::{Prediction, git_version, predict};
use just_fetch::pull::{Planned, Pulled, fast_forward, plan, pullable};
use just_fetch::scan::{ScanError, scan};
use just_fetch::status::{CloneStatus, Head, LocalState};

/// Fetches the clones under a folder and shows how each stands against its upstream.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// A clone, or a folder holding clones. Defaults to the current folder.
    path: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let root = args.path.unwrap_or_else(|| PathBuf::from("."));
    match run(&root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("just-fetch: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(root: &Path) -> Result<(), String> {
    const GIT_MISSING: &str = "git was not found on PATH";
    let found = scan(root).map_err(|e| match e {
        ScanError::Unreadable(path, error) => format!("cannot read {}: {error}", path.display()),
        ScanError::GitMissing => GIT_MISSING.to_string(),
    })?;
    let refreshed = fetch_all(found).map_err(|GitMissing| GIT_MISSING.to_string())?;
    let predictions = predict_diverged(&refreshed).map_err(|GitMissing| GIT_MISSING.to_string())?;
    print!("{}", render(&refreshed, &predictions));
    offer_fast_forward(&refreshed)
}

/// Asks which pullable clones to fast-forward, then does it. Asks only when stdin, stdout
/// and stderr are all terminals, so piped or scripted runs change nothing.
fn offer_fast_forward(refreshed: &[Refreshed]) -> Result<(), String> {
    let at_terminal = std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::io::stderr().is_terminal();
    if !at_terminal {
        return Ok(());
    }
    let git_missing = |GitMissing| "git was not found on PATH".to_string();
    let mut candidates: Vec<(&Refreshed, Planned)> = Vec::new();
    for r in refreshed.iter().filter(|r| pullable(r).is_ok()) {
        if let Ok(planned) = plan(&r.clone.path).map_err(git_missing)? {
            candidates.push((r, planned));
        }
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let labels: Vec<String> = candidates
        .iter()
        .map(
            |(r, _)| match r.clone.state.as_ref().map(|s| s.clone_status()) {
                Ok(CloneStatus::Behind(n)) => format!("{}  behind {n}", r.clone.name),
                _ => r.clone.name.clone(),
            },
        )
        .collect();
    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Pick clones to fast-forward (space picks, enter confirms, esc leaves)")
        .items(&labels)
        .interact_opt();
    let picked = match picked {
        Ok(Some(picked)) => picked,
        Ok(None) => return Ok(()),
        Err(e) => return Err(format!("the prompt failed: {e}")),
    };
    for i in picked {
        let (r, planned) = &candidates[i];
        let name = &r.clone.name;
        match fast_forward(&r.clone.path, planned).map_err(git_missing)? {
            Pulled::Updated => println!("{name}: updated"),
            Pulled::Changed(what) => println!("{name}: skipped: {what} since the list was made"),
            Pulled::Failed(message) => println!("{name}: failed: {}", one_line(&message)),
        }
    }
    Ok(())
}

/// A prediction for each diverged clone, `None` for the rest.
fn predict_diverged(refreshed: &[Refreshed]) -> Result<Vec<Option<Prediction>>, GitMissing> {
    let is_diverged = |r: &Refreshed| {
        matches!(&r.clone.state, Ok(state) if matches!(state.clone_status(), CloneStatus::Diverged { .. }))
            && !matches!(r.fetch, Fetch::Failed(_))
    };
    let version = if refreshed.iter().any(is_diverged) {
        git_version()?
    } else {
        None
    };
    refreshed
        .iter()
        .map(|r| {
            if is_diverged(r) {
                predict(&r.clone.path, version).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

fn render(refreshed: &[Refreshed], predictions: &[Option<Prediction>]) -> String {
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
fn one_line(message: &str) -> String {
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
