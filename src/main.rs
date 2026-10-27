use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use just_fetch::scan::{FoundClone, ScanError, scan};
use just_fetch::status::{Head, LocalState};

/// Lists the clones under a folder and their local state.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// A clone, or a folder holding clones. Defaults to the current folder.
    path: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let root = args.path.unwrap_or_else(|| PathBuf::from("."));
    match scan(&root) {
        Ok(found) => {
            print!("{}", render(&found));
            ExitCode::SUCCESS
        }
        Err(ScanError::Unreadable(path, error)) => {
            eprintln!("just-fetch: cannot read {}: {error}", path.display());
            ExitCode::FAILURE
        }
        Err(ScanError::GitMissing) => {
            eprintln!("just-fetch: git was not found on PATH");
            ExitCode::FAILURE
        }
    }
}

fn render(found: &[FoundClone]) -> String {
    let width = found
        .iter()
        .map(|f| f.name.chars().count())
        .max()
        .unwrap_or(0);
    found
        .iter()
        .map(|f| {
            let detail = match &f.state {
                Ok(state) => describe(state),
                Err(message) => format!("error: {message}"),
            };
            format!("{:<width$}  {detail}\n", f.name)
        })
        .collect()
}

fn describe(state: &LocalState) -> String {
    let head = match &state.head {
        Head::Branch(name) => name.clone(),
        Head::Unborn(name) => format!("{name} (no commits)"),
        Head::Detached => "(detached)".to_string(),
    };
    let mut parts = vec![head];
    parts.push(match &state.upstream {
        Some(upstream) => format!("-> {upstream}"),
        None => "no upstream".to_string(),
    });
    if state.dirty {
        parts.push("dirty".to_string());
    }
    if state.untracked > 0 {
        parts.push(format!("{} untracked", state.untracked));
    }
    parts.join("  ")
}
