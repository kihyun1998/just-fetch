use std::error::Error;
use std::path::Path;

use crate::config;
use crate::prompt::at_terminal;
use crate::scan::{ScanError, scan};
use crate::text::Text;

/// Prints each tracked folder with the clones found in it, or why it cannot be read, then
/// where the list is kept.
pub fn list(file: &Path) -> Result<(), Box<dyn Error>> {
    let tracked = config::load(file)?;
    if tracked.folders.is_empty() {
        println!("{}", Text::NothingTracked);
    }
    for folder in &tracked.folders {
        match scan(folder) {
            Ok(found) => println!("{}", Text::ListClones(path(folder), found.len())),
            Err(ScanError::Unreadable(_, e)) => {
                println!("{}", Text::ListMissing(path(folder), e.to_string()))
            }
            Err(missing @ ScanError::GitMissing(_)) => return Err(missing.into()),
        }
    }
    println!("{}", Text::ConfigAt(path(file)));
    Ok(())
}

fn path(path: &Path) -> String {
    path.display().to_string()
}

/// Asks which tracked folders to stop tracking and, once confirmed, saves the rest to
/// `file`. Leaving without a pick, or answering no, changes nothing.
pub fn remove(file: &Path) -> Result<(), Box<dyn Error>> {
    let mut tracked = config::load(file)?;
    if tracked.folders.is_empty() {
        println!("{}", Text::NothingToRemove);
        return Ok(());
    }
    if !at_terminal() {
        return Err(Text::RemoveNeedsTerminal.to_string().into());
    }
    let labels: Vec<String> = tracked
        .folders
        .iter()
        .map(|folder| match folder.is_dir() {
            true => folder.display().to_string(),
            false => format!("{}  {}", folder.display(), Text::MissingMark),
        })
        .collect();
    let Some(picked) = dialoguer::MultiSelect::new()
        .with_prompt(Text::StopTrackingPrompt.to_string())
        .items(&labels)
        .interact_opt()?
    else {
        return Ok(());
    };
    if picked.is_empty() {
        return Ok(());
    }
    let sure = dialoguer::Confirm::new()
        .with_prompt(Text::StopTracking(picked.len()).to_string())
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
    config::save(file, &tracked)?;
    println!("{}", Text::SavedTo(path(file)));
    Ok(())
}
