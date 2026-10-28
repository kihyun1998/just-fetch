use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::git::{self, GitError, GitMissing};
use crate::scan::FoundClone;
use crate::status::{self, CloneStatus, Head};

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
    pub found: FoundClone,
    pub fetch: Fetch,
}

/// Why a clone's standing against its upstream is not known.
#[derive(Debug, PartialEq, Eq)]
pub enum NotFresh {
    /// git could not read the clone.
    Unreadable,
    FetchFailed,
}

impl Refreshed {
    /// How the clone stands against its upstream after this run's fetch.
    pub fn fresh_status(&self) -> Result<CloneStatus, NotFresh> {
        let state = self
            .found
            .state
            .as_ref()
            .map_err(|_| NotFresh::Unreadable)?;
        match self.fetch {
            Fetch::Failed(_) => Err(NotFresh::FetchFailed),
            Fetch::Skipped | Fetch::Fetched => Ok(state.clone_status()),
        }
    }
}

/// Fetches only the remote `branch`'s upstream lives on, without submodule recursion or the
/// `ext::` transport, with git's own upload-pack for a local remote. A clone whose own
/// config sets `core.gitProxy` or its upstream remote's `vcs` is not fetched.
fn fetch_upstream(clone: &Path, branch: &str) -> Result<String, GitError> {
    let remote = git::run(
        clone,
        &["config", "--get", &format!("branch.{branch}.remote")],
    )?
    .trim()
    .to_string();
    let url = match remote.as_str() {
        "." => ".".to_string(),
        _ => git::run(clone, &["remote", "get-url", &remote])?
            .trim()
            .to_string(),
    };
    for key in ["core.gitProxy".to_string(), format!("remote.{remote}.vcs")] {
        if set_by_the_clone(clone, &key)? {
            return Err(GitError::Failed(format!(
                "not fetched: this clone's own config sets {key}, which would run a command"
            )));
        }
    }
    let mut args = vec![
        "-c",
        "protocol.ext.allow=never",
        "fetch",
        "--prune",
        "--quiet",
        "--no-recurse-submodules",
    ];
    if git::is_local_url(&url) {
        args.push("--upload-pack=git-upload-pack");
    }
    args.push(&remote);
    git::run(clone, &args)
}

/// Whether `key` is set in the clone's own config, at local or worktree scope, including
/// files those pull in.
fn set_by_the_clone(clone: &Path, key: &str) -> Result<bool, GitError> {
    match git::run(clone, &["config", "--show-scope", "--get-all", key]) {
        Ok(scopes) => Ok(scopes
            .lines()
            .any(|line| line.starts_with("local") || line.starts_with("worktree"))),
        Err(GitError::Failed(_)) => Ok(false),
        Err(GitError::Missing) => Err(GitError::Missing),
    }
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

fn refresh(mut found: FoundClone) -> Result<Refreshed, GitMissing> {
    let branch = match &found.state {
        Ok(state) if state.upstream.is_some() => match &state.head {
            Head::Branch(name) | Head::Unborn(name) => Some(name.clone()),
            Head::Detached => None,
        },
        _ => None,
    };
    let Some(branch) = branch else {
        return Ok(Refreshed {
            found,
            fetch: Fetch::Skipped,
        });
    };
    let fetch = match git::split_missing(fetch_upstream(&found.path, &branch))? {
        Ok(_) => Fetch::Fetched,
        Err(message) => Fetch::Failed(message),
    };
    found.state = git::split_missing(status::read(&found.path))?;
    Ok(Refreshed { found, fetch })
}
