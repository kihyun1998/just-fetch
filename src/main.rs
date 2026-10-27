use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use just_fetch::fetch::fetch_all;
use just_fetch::predict::predict_diverged;
use just_fetch::prompt::offer_fast_forward;
use just_fetch::report::render;
use just_fetch::scan::scan;

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

fn run(root: &Path) -> Result<(), Box<dyn Error>> {
    let found = scan(root)?;
    let refreshed = fetch_all(found)?;
    let predictions = predict_diverged(&refreshed)?;
    print!("{}", render(&refreshed, &predictions));
    offer_fast_forward(&refreshed)
}
