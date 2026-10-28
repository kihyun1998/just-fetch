use std::error::Error;
use std::path::Path;

use crate::prompt::at_terminal;
use crate::scan::{ScanError, scan};
use crate::tracked;

/// `n` followed by `noun`, adding an `s` unless `n` is 1.
pub fn count(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        _ => format!("{n} {noun}s"),
    }
}

/// Prints each tracked folder with the clones found in it, or why it cannot be read, then
/// where the list is kept.
pub fn list(file: &Path) -> Result<(), Box<dyn Error>> {
    let tracked = tracked::load(file)?;
    if tracked.folders.is_empty() {
        println!("nothing is tracked yet: run `just-fetch setup` to add folders");
    }
    for folder in &tracked.folders {
        match scan(folder) {
            Ok(found) => println!("{}  {}", folder.display(), count(found.len(), "clone")),
            Err(ScanError::Unreadable(_, e)) => println!("{}  missing: {e}", folder.display()),
            Err(missing @ ScanError::GitMissing(_)) => return Err(missing.into()),
        }
    }
    println!("config: {}", file.display());
    Ok(())
}

/// Asks which tracked folders to stop tracking and, once confirmed, saves the rest to
/// `file`. Leaving without a pick, or answering no, changes nothing.
pub fn remove(file: &Path) -> Result<(), Box<dyn Error>> {
    let mut tracked = tracked::load(file)?;
    if tracked.folders.is_empty() {
        println!("nothing is tracked, so there is nothing to remove");
        return Ok(());
    }
    if !at_terminal() {
        return Err("remove asks which folders to drop, so it needs a terminal".into());
    }
    let labels: Vec<String> = tracked
        .folders
        .iter()
        .map(|folder| match folder.is_dir() {
            true => folder.display().to_string(),
            false => format!("{}  (missing)", folder.display()),
        })
        .collect();
    let Some(picked) = dialoguer::MultiSelect::new()
        .with_prompt("Folders to stop tracking (space picks, enter confirms, esc leaves)")
        .items(&labels)
        .interact_opt()?
    else {
        return Ok(());
    };
    if picked.is_empty() {
        return Ok(());
    }
    let sure = dialoguer::Confirm::new()
        .with_prompt(format!("Stop tracking {}?", count(picked.len(), "folder")))
        .interact_opt()?;
    if sure != Some(true) {
        return Ok(());
    }
    let mut index = 0;
    tracked.folders.retain(|_| {
        let keep = !picked.contains(&index);
        index += 1;
        keep
    });
    tracked::save(file, &tracked)?;
    println!("saved to {}", file.display());
    Ok(())
}
