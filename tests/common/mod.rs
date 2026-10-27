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
