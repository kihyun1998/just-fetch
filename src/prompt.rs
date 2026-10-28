use std::error::Error;
use std::io::{self, IsTerminal};

use console::Term;

use crate::fast_forward::{Outcome, fast_forward, offerable, plan};
use crate::fetch::Refreshed;
use crate::git;
use crate::pick::{Row, Screen, Step, describe};
use crate::predict::Prediction;
use crate::report::one_line;

/// Whether stdin, stdout and stderr are all terminals.
pub fn at_terminal() -> bool {
    std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::io::stderr().is_terminal()
}

/// Shows the pick screen over `refreshed` and fast-forwards the clones picked and confirmed.
/// With nothing that can be brought in, prints the groups and asks nothing.
pub fn offer_fast_forward(
    refreshed: &[Refreshed],
    predictions: &[Option<Prediction>],
) -> Result<(), Box<dyn Error>> {
    let mut rows = Vec::new();
    let mut plans = Vec::new();
    for ((r, prediction), label) in refreshed.iter().zip(predictions).zip(labels(refreshed)) {
        let planned = match offerable(r) {
            Ok(_) => Some(git::split_missing(plan(&r.found.path))?),
            Err(_) => None,
        };
        let unplanned = match &planned {
            Some(Err(message)) => Some(message.as_str()),
            _ => None,
        };
        let (group, sentence) = describe(r, prediction.as_ref(), unplanned);
        rows.push(Row {
            label,
            group,
            sentence,
        });
        plans.push(planned.and_then(Result::ok));
    }
    let mut screen = Screen::new(rows.clone());
    if !screen.has_pickable() {
        for line in screen.summary() {
            println!("{line}");
        }
        return Ok(());
    }
    let picked = show(&mut screen).map_err(|e| format!("the pick screen failed: {e}"))?;
    for i in picked {
        let Some(planned) = &plans[i] else { continue };
        let label = &rows[i].label;
        match fast_forward(&refreshed[i].found.path, planned)? {
            Outcome::Updated => println!("{label}: updated"),
            Outcome::Changed(what) => {
                println!("{label}: skipped: {what} since the list was made")
            }
            Outcome::Failed(message) => println!("{label}: failed: {}", one_line(&message)),
        }
    }
    Ok(())
}

/// Each clone's name, with its parent folder's name in front where two clones share it, and
/// its whole path where the parent's name is shared too.
fn labels(refreshed: &[Refreshed]) -> Vec<String> {
    let with_parent = |r: &Refreshed| {
        let parent = r.found.path.parent().and_then(|p| p.file_name());
        match parent {
            Some(parent) => format!("{}/{}", parent.to_string_lossy(), r.found.name),
            None => r.found.path.display().to_string(),
        }
    };
    refreshed
        .iter()
        .map(|r| {
            let name = &r.found.name;
            if refreshed.iter().filter(|o| &o.found.name == name).count() < 2 {
                return name.clone();
            }
            let label = with_parent(r);
            match refreshed.iter().filter(|o| with_parent(o) == label).count() {
                1 => label,
                _ => r.found.path.display().to_string(),
            }
        })
        .collect()
}

/// Runs `screen` on the terminal's alternate screen until a person leaves or confirms, and
/// returns the rows to fast-forward.
fn show(screen: &mut Screen) -> io::Result<Vec<usize>> {
    let term = Term::stdout();
    if !term.features().colors_supported() {
        return Err(io::Error::other("this terminal cannot draw it"));
    }
    let _alternate = Alternate::enter(&term)?;
    loop {
        let (height, width) = term.size();
        let lines = screen.lines(height as usize, width as usize);
        term.write_str(CLEAR)?;
        term.write_str(&lines.join("\n"))?;
        term.flush()?;
        let page = (height as usize).saturating_sub(3).max(1);
        match screen.key(term.read_key_raw()?, page) {
            Step::Stay => {}
            Step::Quit => return Ok(Vec::new()),
            Step::Go(picked) => return Ok(picked),
        }
    }
}

/// Moves the cursor to the top left and clears the screen.
const CLEAR: &str = "\x1b[H\x1b[2J";

/// The terminal's alternate screen with the cursor hidden, given back when dropped.
struct Alternate<'a>(&'a Term);

impl<'a> Alternate<'a> {
    fn enter(term: &'a Term) -> io::Result<Self> {
        term.write_str("\x1b[?1049h")?;
        term.hide_cursor()?;
        Ok(Self(term))
    }
}

impl Drop for Alternate<'_> {
    fn drop(&mut self) {
        let _ = self.0.show_cursor();
        let _ = self.0.write_str("\x1b[?1049l");
        let _ = self.0.flush();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::fetch::Fetch;
    use crate::scan::FoundClone;

    use super::*;

    fn at(path: &str) -> Refreshed {
        let path = PathBuf::from(path);
        Refreshed {
            found: FoundClone {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                path,
                state: Err("unused".into()),
            },
            fetch: Fetch::Skipped,
        }
    }

    #[test]
    fn a_shared_name_gets_its_parent_and_a_shared_parent_its_whole_path() {
        let refreshed = [
            at("/home/me/work/api"),
            at("/home/me/personal/api"),
            at("/a/x/web"),
            at("/b/x/web"),
            at("/home/me/work/solo"),
        ];
        let labels = labels(&refreshed);
        assert_eq!(labels[..2], ["work/api", "personal/api"]);
        assert_eq!(labels[2], PathBuf::from("/a/x/web").display().to_string());
        assert_eq!(labels[3], PathBuf::from("/b/x/web").display().to_string());
        assert_eq!(labels[4], "solo");
    }
}
