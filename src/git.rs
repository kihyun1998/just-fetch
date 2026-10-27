use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// git is not on PATH, so nothing can be asked of any clone.
#[derive(Debug)]
pub struct GitMissing;

/// Runs git in `clone`, the way every just-fetch call does: never prompting, and never
/// running a command the clone's own config names for status or ref updates.
///
/// `Ok(Err(message))` is git failing for this clone alone; its stderr, trimmed.
pub fn run(clone: &Path, args: &[&str]) -> Result<Result<String, String>, GitMissing> {
    let Some(no_hooks) = no_hooks_dir() else {
        return Ok(Err(
            "cannot find just-fetch's own path, so the clone's hooks cannot be turned off"
                .to_string(),
        ));
    };
    let out = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .arg("-c")
        .arg(format!("core.hooksPath={}", no_hooks.display()))
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(clone)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output();
    match out {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(GitMissing),
        Err(e) => Ok(Err(e.to_string())),
        Ok(out) if !out.status.success() => {
            Ok(Err(String::from_utf8_lossy(&out.stderr).trim().to_string()))
        }
        Ok(out) => Ok(Ok(String::from_utf8_lossy(&out.stdout).into_owned())),
    }
}

/// A folder path that cannot exist: it sits under this executable, which is a file.
/// `None` when the executable's own path is unknown.
fn no_hooks_dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| std::env::current_exe().ok().map(|exe| exe.join("no-hooks")))
        .as_ref()
}
