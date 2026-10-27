use std::error::Error;
use std::io::IsTerminal;

use crate::fast_forward::{Outcome, Planned, fast_forward, offerable, plan};
use crate::fetch::Refreshed;
use crate::git;
use crate::report::one_line;

/// Asks which offerable clones to fast-forward, then does it. Asks only when stdin, stdout
/// and stderr are all terminals, so piped or scripted runs change nothing.
pub fn offer_fast_forward(refreshed: &[Refreshed]) -> Result<(), Box<dyn Error>> {
    let at_terminal = std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::io::stderr().is_terminal();
    if !at_terminal {
        return Ok(());
    }
    let mut candidates: Vec<(&Refreshed, Planned)> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for r in refreshed {
        let Ok(behind) = offerable(r) else { continue };
        if let Ok(planned) = git::split_missing(plan(&r.found.path))? {
            labels.push(format!("{}  behind {behind}", r.found.name));
            candidates.push((r, planned));
        }
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Pick clones to fast-forward (space picks, enter confirms, esc leaves)")
        .items(&labels)
        .interact_opt();
    let picked = match picked {
        Ok(Some(picked)) => picked,
        Ok(None) => return Ok(()),
        Err(e) => return Err(format!("the prompt failed: {e}").into()),
    };
    for i in picked {
        let (r, planned) = &candidates[i];
        let name = &r.found.name;
        match fast_forward(&r.found.path, planned)? {
            Outcome::Updated => println!("{name}: updated"),
            Outcome::Changed(what) => println!("{name}: skipped: {what} since the list was made"),
            Outcome::Failed(message) => println!("{name}: failed: {}", one_line(&message)),
        }
    }
    Ok(())
}
