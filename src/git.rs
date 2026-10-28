use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// git is not on PATH, so nothing can be asked of any clone.
#[derive(Debug)]
pub struct GitMissing;

impl fmt::Display for GitMissing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("git was not found on PATH")
    }
}

impl std::error::Error for GitMissing {}

/// A git call that did not give its answer: git is missing altogether, or it failed for
/// this clone alone.
#[derive(Debug)]
pub enum GitError {
    Missing,
    /// git's stderr, trimmed, or why git could not be started for this clone.
    Failed(String),
}

impl From<io::Error> for GitError {
    fn from(e: io::Error) -> Self {
        GitError::Failed(e.to_string())
    }
}

/// Splits `result` into what stops the whole run (git missing) and the rest: the answer, or
/// git's failure message.
pub fn split_missing<T>(result: Result<T, GitError>) -> Result<Result<T, String>, GitMissing> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(GitError::Failed(message)) => Ok(Err(message)),
        Err(GitError::Missing) => Err(GitMissing),
    }
}

/// A finished git call: its exit code (`None` when killed by a signal) and its output.
#[derive(Debug)]
pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    /// Trimmed.
    pub stderr: String,
}

impl Ran {
    /// stdout for exit 0, stderr for anything else.
    fn into_stdout(self) -> Result<String, GitError> {
        match self.code {
            Some(0) => Ok(self.stdout),
            _ => Err(GitError::Failed(self.stderr)),
        }
    }
}

/// Runs git in `clone` without prompting, without fsmonitor and without the clone's hooks,
/// and returns its stdout.
pub fn run(clone: &Path, args: &[&str]) -> Result<String, GitError> {
    run_for_exit_code(clone, args)?.into_stdout()
}

/// [`run`], handing back the exit code for the caller to read.
pub fn run_for_exit_code(clone: &Path, args: &[&str]) -> Result<Ran, GitError> {
    let no_hooks = no_hooks_dir().ok_or_else(|| {
        GitError::Failed(
            "cannot find just-fetch's own path, so the clone's hooks cannot be turned off"
                .to_string(),
        )
    })?;
    let hooks_off = format!("core.hooksPath={}", no_hooks.display());
    start(clone, &["-c", &hooks_off], args)
}

/// [`run`], except that the clone's own hooks run.
pub fn run_with_clone_hooks(clone: &Path, args: &[&str]) -> Result<String, GitError> {
    start(clone, &[], args)?.into_stdout()
}

/// The commits `clone`'s HEAD and its upstream point at.
pub fn head_and_upstream(clone: &Path) -> Result<(String, String), GitError> {
    let out = run(clone, &["rev-parse", "HEAD", "@{upstream}"])?;
    match out.split_whitespace().collect::<Vec<_>>().as_slice() {
        [head, upstream] => Ok((head.to_string(), upstream.to_string())),
        _ => Err(GitError::Failed(format!(
            "unexpected rev-parse output: {out:?}"
        ))),
    }
}

/// Whether git would reach `url` through its local transport (a path or `file://`), by
/// git's own rule in `url_is_local_not_ssh` and `transport_get`.
pub fn is_local_url(url: &str) -> bool {
    if url.starts_with("file://") {
        return true;
    }
    let Some(colon) = url.find(':') else {
        return true;
    };
    let slash_first = url.find('/').is_some_and(|slash| slash < colon);
    let drive = cfg!(windows) && colon == 1 && url.starts_with(|c: char| c.is_ascii_alphabetic());
    slash_first || drive
}

fn start(clone: &Path, hooks: &[&str], args: &[&str]) -> Result<Ran, GitError> {
    let out = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(hooks)
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(clone)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output();
    match out {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(GitError::Missing),
        Err(e) => Err(e.into()),
        Ok(out) => Ok(Ran {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        }),
    }
}

/// A folder path that cannot exist: it sits under this executable, which is a file.
/// `None` when the executable's own path is unknown.
fn no_hooks_dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| std::env::current_exe().ok().map(|exe| exe.join("no-hooks")))
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_file_urls_are_local() {
        assert!(is_local_url("/srv/git/repo.git"));
        assert!(is_local_url("../sibling.git"));
        assert!(is_local_url("repo.git"));
        assert!(is_local_url("file:///srv/git/repo.git"));
        assert!(is_local_url("./odd:name.git"));
    }

    #[test]
    fn network_and_scp_like_urls_are_not_local() {
        assert!(!is_local_url(
            "https://github.com/kihyun1998/just-fetch.git"
        ));
        assert!(!is_local_url(
            "ssh://git@github.com/kihyun1998/just-fetch.git"
        ));
        assert!(!is_local_url("git://example.org/repo.git"));
        assert!(!is_local_url("git@github.com:kihyun1998/just-fetch.git"));
        assert!(!is_local_url("ext::sh -c x"));
    }

    #[test]
    fn a_drive_letter_is_local_only_on_windows() {
        assert_eq!(is_local_url("C:/Users/me/repo.git"), cfg!(windows));
        assert_eq!(is_local_url("D:\\repos\\x.git"), cfg!(windows));
    }
}
