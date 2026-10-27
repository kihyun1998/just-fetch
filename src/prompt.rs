use std::error::Error;
use std::io::IsTerminal;

use crate::fetch::Refreshed;
use crate::git;
use crate::pull::{Planned, Pulled, fast_forward, plan, pullable};
use crate::report::one_line;

/// Asks which pullable clones to fast-forward, then does it. Asks only when stdin, stdout
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
        let Ok(behind) = pullable(r) else { continue };
        if let Ok(planned) = git::split_missing(plan(&r.clone.path))? {
            labels.push(format!("{}  behind {behind}", r.clone.name));
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
        let name = &r.clone.name;
        match fast_forward(&r.clone.path, planned)? {
            Pulled::Updated => println!("{name}: updated"),
            Pulled::Changed(what) => println!("{name}: skipped: {what} since the list was made"),
            Pulled::Failed(message) => println!("{name}: failed: {}", one_line(&message)),
        }
    }
    Ok(())
}
