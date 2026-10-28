use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use just_fetch::fetch::fetch_all;
use just_fetch::manage::{list, remove};
use just_fetch::predict::predict_diverged;
use just_fetch::prompt::{at_terminal, offer_fast_forward};
use just_fetch::report::render;
use just_fetch::scan::{Folder, scan, scan_folders};
use just_fetch::setup::setup;
use just_fetch::tracked::{self, config_file};

const NOTHING_TRACKED: &str = "no folders are tracked yet: run `just-fetch setup` to add some";

/// Fetches your clones and shows how each stands against its upstream.
#[derive(Parser)]
#[command(version, args_conflicts_with_subcommands = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    /// A clone, or a folder holding clones, to cover this once. Defaults to the tracked folders.
    path: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Add folders to track, so a run with no path covers them.
    Setup,
    /// Show the tracked folders and where the list is kept.
    List,
    /// Pick tracked folders to stop tracking.
    Remove,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("just-fetch: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let file = || config_file().ok_or("cannot find a folder for the config file");
    match args.command {
        Some(Command::Setup) => return setup(&file()?).map(|_| ()),
        Some(Command::List) => return list(&file()?),
        Some(Command::Remove) => return remove(&file()?),
        None => {}
    }
    let folders = match args.path {
        Some(path) => vec![Folder {
            clones: Ok(scan(&path)?),
            root: path,
        }],
        None => {
            let file = file()?;
            let mut tracked = tracked::load(&file)?;
            if tracked.folders.is_empty() {
                if !at_terminal() {
                    return Err(NOTHING_TRACKED.into());
                }
                tracked = setup(&file)?;
                if tracked.folders.is_empty() {
                    return Err(NOTHING_TRACKED.into());
                }
            }
            scan_folders(&tracked.folders)?
        }
    };
    let headings = folders.len() > 1;
    let mut all = Vec::new();
    let sections: Vec<_> = folders
        .into_iter()
        .map(|folder| {
            let size = folder.clones.map(|clones| {
                let size = clones.len();
                all.extend(clones);
                size
            });
            (folder.root, size)
        })
        .collect();
    let none_read = sections.iter().all(|(_, size)| size.is_err());
    let refreshed = fetch_all(all)?;
    let predictions = predict_diverged(&refreshed)?;
    let mut at = 0;
    for (root, size) in sections {
        if headings {
            println!("{}", root.display());
        }
        match size {
            Ok(size) => {
                print!(
                    "{}",
                    render(&refreshed[at..at + size], &predictions[at..at + size])
                );
                at += size;
            }
            Err(message) => println!("{message}"),
        }
    }
    if none_read {
        return Err("none of the tracked folders could be read".into());
    }
    offer_fast_forward(&refreshed)
}
