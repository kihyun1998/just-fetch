// Each test crate uses only some of these helpers.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// An empty file standing in for the global git config, so a developer's own settings
/// (hooks, `status.*`, signing) never reach the fixtures.
fn empty_global_config() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("just-fetch-tests-empty-gitconfig");
    if !path.exists() {
        std::fs::write(&path, "").unwrap();
    }
    path
}

pub fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", empty_global_config())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A clone of a fresh bare remote, with one commit pushed, so `main` tracks `origin/main`.
pub fn clone_with_upstream(parent: &Path, name: &str) -> std::path::PathBuf {
    let remote = parent.join(format!(".remotes/{name}.git"));
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "--bare", "-q"]);
    let clone = parent.join(name);
    git(parent, &["clone", "-q", remote.to_str().unwrap(), name]);
    std::fs::write(clone.join("README.md"), "hello\n").unwrap();
    git(&clone, &["add", "README.md"]);
    git(&clone, &["commit", "-q", "-m", "first"]);
    git(&clone, &["push", "-q", "-u", "origin", "main"]);
    clone
}

/// A clone with no remote and no commits.
pub fn empty_clone(parent: &Path, name: &str) -> std::path::PathBuf {
    let clone = parent.join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q"]);
    clone
}

/// The bare remote `clone_with_upstream` made for `name`.
pub fn remote_of(parent: &Path, name: &str) -> std::path::PathBuf {
    parent.join(format!(".remotes/{name}.git"))
}

pub fn commit(clone: &Path, message: &str) {
    git(clone, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// Pushes one new commit to `branch` of `name`'s remote from a second clone, as another
/// machine would.
pub fn push_from_elsewhere(parent: &Path, name: &str, branch: &str) {
    let elsewhere = parent.join(format!(".elsewhere/{name}"));
    if !elsewhere.exists() {
        std::fs::create_dir_all(parent.join(".elsewhere")).unwrap();
        git(
            &parent.join(".elsewhere"),
            &[
                "clone",
                "-q",
                remote_of(parent, name).to_str().unwrap(),
                name,
            ],
        );
    }
    git(&elsewhere, &["fetch", "-q"]);
    git(&elsewhere, &["switch", "-q", branch]);
    git(&elsewhere, &["pull", "-q", "--ff-only"]);
    commit(&elsewhere, "from elsewhere");
    git(&elsewhere, &["push", "-q", "origin", branch]);
}

/// Writes `content` to `file` and commits it.
pub fn commit_file(clone: &Path, file: &str, content: &str) {
    std::fs::write(clone.join(file), content).unwrap();
    git(clone, &["add", file]);
    git(clone, &["commit", "-q", "-m", &format!("change {file}")]);
}

/// Like `push_from_elsewhere`, with the commit changing `file` to `content`.
pub fn push_file_from_elsewhere(parent: &Path, name: &str, file: &str, content: &str) {
    let elsewhere = parent.join(format!(".elsewhere/{name}"));
    if !elsewhere.exists() {
        std::fs::create_dir_all(parent.join(".elsewhere")).unwrap();
        git(
            &parent.join(".elsewhere"),
            &[
                "clone",
                "-q",
                remote_of(parent, name).to_str().unwrap(),
                name,
            ],
        );
    }
    git(&elsewhere, &["pull", "-q", "--ff-only"]);
    commit_file(&elsewhere, file, content);
    git(&elsewhere, &["push", "-q", "origin", "HEAD"]);
}

/// Marks a hook script executable. Git for Windows runs hooks whatever their mode.
#[cfg(unix)]
pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
pub fn make_executable(_path: &Path) {}

/// Runs git like [`git`] and says whether it succeeded, for commands expected to fail.
pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .env("GIT_CONFIG_GLOBAL", empty_global_config())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("git runs")
        .status
        .success()
}

/// `path` as a shell script would write it, with forward slashes.
pub fn for_sh(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A stand-in for ssh that waits `seconds`, then runs the remote command locally.
/// Use it with `ssh.variant=simple`, which passes the host and the command as arguments.
pub fn fake_ssh(dir: &Path, seconds: u32) -> std::path::PathBuf {
    let script = dir.join(format!("fake-ssh-{seconds}.sh"));
    std::fs::write(
        &script,
        format!("#!/bin/sh\nsleep {seconds}\nfor arg; do last=$arg; done\nexec sh -c \"$last\"\n"),
    )
    .unwrap();
    make_executable(&script);
    script
}

/// Points `clone`'s origin at its own bare remote over `fake_ssh`, as `localhost:<path>`.
pub fn fetch_over_fake_ssh(clone: &Path, remote: &Path, ssh: &Path) {
    git(clone, &["config", "core.sshCommand", &for_sh(ssh)]);
    git(clone, &["config", "ssh.variant", "simple"]);
    git(
        clone,
        &[
            "remote",
            "set-url",
            "origin",
            &format!("localhost:{}", for_sh(remote)),
        ],
    );
}
