use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// git is not on PATH, so nothing can be asked of any clone.
#[derive(Debug)]
pub struct GitMissing;

/// A finished git call: its exit code (`None` when killed by a signal) and its output.
#[derive(Debug)]
pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    /// Trimmed.
    pub stderr: String,
}

/// Runs git in `clone` without prompting, without fsmonitor and without the clone's hooks.
///
/// `Ok(Err(message))` is git failing for this clone alone; its stderr, trimmed.
pub fn run(clone: &Path, args: &[&str]) -> Result<Result<String, String>, GitMissing> {
    Ok(run_with(clone, args, &[])?.and_then(Ran::into_stdout))
}

/// [`run`] with extra environment, handing back the exit code for the caller to read.
///
/// `Ok(Err(message))` is git not starting for this clone.
pub fn run_with(
    clone: &Path,
    args: &[&str],
    env: &[(&str, &OsStr)],
) -> Result<Result<Ran, String>, GitMissing> {
    let Some(no_hooks) = no_hooks_dir() else {
        return Ok(Err(
            "cannot find just-fetch's own path, so the clone's hooks cannot be turned off"
                .to_string(),
        ));
    };
    let hooks_off = format!("core.hooksPath={}", no_hooks.display());
    start(clone, &["-c", &hooks_off], args, env)
}

/// [`run`], except that the clone's own hooks run.
pub fn run_with_clone_hooks(
    clone: &Path,
    args: &[&str],
) -> Result<Result<String, String>, GitMissing> {
    Ok(start(clone, &[], args, &[])?.and_then(Ran::into_stdout))
}

impl Ran {
    /// stdout for exit 0, stderr for anything else.
    fn into_stdout(self) -> Result<String, String> {
        match self.code {
            Some(0) => Ok(self.stdout),
            _ => Err(self.stderr),
        }
    }
}

fn start(
    clone: &Path,
    hooks: &[&str],
    args: &[&str],
    env: &[(&str, &OsStr)],
) -> Result<Result<Ran, String>, GitMissing> {
    let out = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(hooks)
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(clone)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .output();
    match out {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(GitMissing),
        Err(e) => Ok(Err(e.to_string())),
        Ok(out) => Ok(Ok(Ran {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })),
    }
}

/// A folder path that cannot exist: it sits under this executable, which is a file.
/// `None` when the executable's own path is unknown.
fn no_hooks_dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| std::env::current_exe().ok().map(|exe| exe.join("no-hooks")))
        .as_ref()
}
