use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use just_fetch::fetch::{Fetch, Refreshed, fetch_all};
use just_fetch::git::GitMissing;
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
        Ok(table) => {
            print!("{table}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("just-fetch: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(root: &Path) -> Result<String, String> {
    const GIT_MISSING: &str = "git was not found on PATH";
    let found = scan(root).map_err(|e| match e {
        ScanError::Unreadable(path, error) => format!("cannot read {}: {error}", path.display()),
        ScanError::GitMissing => GIT_MISSING.to_string(),
    })?;
    let refreshed = fetch_all(found).map_err(|GitMissing| GIT_MISSING.to_string())?;
    Ok(render(&refreshed))
}

fn render(refreshed: &[Refreshed]) -> String {
    let width = refreshed
        .iter()
        .map(|r| r.clone.name.chars().count())
        .max()
        .unwrap_or(0);
    refreshed
        .iter()
        .map(|r| {
            let detail = match &r.clone.state {
                Ok(state) => describe(state, &r.fetch),
                Err(message) => format!("error: {}", one_line(message)),
            };
            format!("{:<width$}  {detail}\n", r.clone.name)
        })
        .collect()
}

fn describe(state: &LocalState, fetch: &Fetch) -> String {
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
