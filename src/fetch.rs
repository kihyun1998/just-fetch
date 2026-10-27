use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::git::{self, GitMissing};
use crate::scan::{self, FoundClone};

/// What happened when just-fetch fetched a clone's upstream remote.
#[derive(Debug)]
pub enum Fetch {
    /// Not fetched: no upstream, or git could not read the clone.
    Skipped,
    Fetched,
    /// git's message, trimmed.
    Failed(String),
}

/// A clone after its fetch, with its local state read again.
#[derive(Debug)]
pub struct Refreshed {
    pub clone: FoundClone,
    pub fetch: Fetch,
}

/// How many clones fetch at once.
const WORKERS: usize = 8;

/// Fetches every clone that has an upstream from that upstream's remote, several at a time,
/// and reads each fetched clone's state again. Results keep the order they came in.
pub fn fetch_all(found: Vec<FoundClone>) -> Result<Vec<Refreshed>, GitMissing> {
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<FoundClone>>> =
        found.into_iter().map(|f| Mutex::new(Some(f))).collect();
    let results: Vec<Mutex<Option<Result<Refreshed, GitMissing>>>> =
        slots.iter().map(|_| Mutex::new(None)).collect();

    thread::scope(|scope| {
        for _ in 0..WORKERS.min(slots.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(slot) = slots.get(i) else { break };
                    let clone = slot.lock().unwrap().take().unwrap();
                    *results[i].lock().unwrap() = Some(refresh(clone));
                }
            });
        }
    });

    results
        .into_iter()
        .map(|r| r.into_inner().unwrap().unwrap())
        .collect()
}

fn refresh(mut clone: FoundClone) -> Result<Refreshed, GitMissing> {
    let has_upstream = matches!(&clone.state, Ok(state) if state.upstream.is_some());
    if !has_upstream {
        return Ok(Refreshed {
            clone,
            fetch: Fetch::Skipped,
        });
    }
    let fetch = match git::run(&clone.path, &["fetch", "--prune", "--quiet"])? {
        Ok(_) => Fetch::Fetched,
        Err(message) => Fetch::Failed(message),
    };
    clone.state = scan::local_state(&clone.path)?;
    Ok(Refreshed { clone, fetch })
}
