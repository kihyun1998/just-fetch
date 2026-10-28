use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use just_fetch::config::{self, config_file};
use just_fetch::fetch::fetch_all;
use just_fetch::lang;
use just_fetch::manage::{list, remove};
use just_fetch::predict::predict_diverged;
use just_fetch::prompt::{at_terminal, offer_fast_forward};
use just_fetch::report::render;
use just_fetch::scan::{Folder, scan, scan_folders};
use just_fetch::setup::setup;
use just_fetch::text::Text;
use just_fetch::upgrade::upgrade;

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
    /// Bring just-fetch itself to its newest release, through what installed it.
    Upgrade,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("just-fetch: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<ExitCode, Box<dyn Error>> {
    let saved = config_file()
        .and_then(|file| config::load(&file).ok())
        .and_then(|config| config.language);
    lang::set(lang::resolve(saved));
    let file = || config_file().ok_or_else(|| Text::NoConfigFolder.to_string());
    match args.command {
        Some(Command::Setup) => return setup(&file()?).map(|_| ExitCode::SUCCESS),
        Some(Command::List) => return list(&file()?).map(|_| ExitCode::SUCCESS),
        Some(Command::Remove) => return remove(&file()?).map(|_| ExitCode::SUCCESS),
        Some(Command::Upgrade) => return upgrade(),
        None => {}
    }
    let folders = match args.path {
        Some(path) => vec![Folder {
            clones: Ok(scan(&path)?),
            root: path,
        }],
        None => {
            let file = file()?;
            let mut tracked = config::load(&file)?;
            if tracked.folders.is_empty() {
                if !at_terminal() {
                    return Err(Text::NothingTracked.to_string().into());
                }
                tracked = setup(&file)?;
                if tracked.folders.is_empty() {
                    return Err(Text::NothingTracked.to_string().into());
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
    let interactive = at_terminal();
    let mut at = 0;
    for (root, size) in sections {
        match size {
            Ok(size) if !interactive => {
                if headings {
                    println!("{}", root.display());
                }
                print!(
                    "{}",
                    render(&refreshed[at..at + size], &predictions[at..at + size])
                );
                at += size;
            }
            Ok(_) => {}
            Err(message) if !interactive && headings => {
                println!("{}", root.display());
                println!("{message}");
            }
            Err(message) => println!("{message}"),
        }
    }
    if none_read {
        return Err(Text::NoneRead.to_string().into());
    }
    if interactive {
        offer_fast_forward(&refreshed, &predictions)?;
    }
    Ok(ExitCode::SUCCESS)
}
