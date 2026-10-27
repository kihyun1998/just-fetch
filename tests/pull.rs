mod common;

use std::path::Path;
use std::process::Command;

use common::{clone_with_upstream, commit_file, git, push_file_from_elsewhere};
use just_fetch::pull::{Pulled, fast_forward, plan};

fn rev(clone: &Path, what: &str) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(clone)
        .args(["rev-parse", what])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn behind(root: &Path, name: &str) -> std::path::PathBuf {
    let clone = clone_with_upstream(root, name);
    push_file_from_elsewhere(root, name, "theirs.txt", "theirs\n");
    git(&clone, &["fetch", "-q"]);
    clone
}

#[test]
fn a_behind_clone_is_fast_forwarded_to_its_upstream() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = behind(tmp.path(), "repo");
    assert_ne!(rev(&clone, "HEAD"), rev(&clone, "@{upstream}"));

    let pulled = fast_forward(&clone, &plan(&clone).unwrap().unwrap()).unwrap();

    assert!(matches!(pulled, Pulled::Updated), "{pulled:?}");
    assert_eq!(rev(&clone, "HEAD"), rev(&clone, "@{upstream}"));
    assert_eq!(text(&clone.join("theirs.txt")), "theirs\n");
}

#[test]
fn a_diverged_clone_is_refused_and_left_as_it_was() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = behind(root, "repo");
    commit_file(&clone, "mine.txt", "mine\n");
    let head = rev(&clone, "HEAD");

    let pulled = fast_forward(&clone, &plan(&clone).unwrap().unwrap()).unwrap();

    match pulled {
        Pulled::Failed(message) => assert!(!message.is_empty()),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(rev(&clone, "HEAD"), head);
}

#[test]
fn an_untracked_file_in_the_way_is_refused_and_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = behind(tmp.path(), "repo");
    std::fs::write(clone.join("theirs.txt"), "my own notes\n").unwrap();
    let head = rev(&clone, "HEAD");

    let pulled = fast_forward(&clone, &plan(&clone).unwrap().unwrap()).unwrap();

    assert!(matches!(pulled, Pulled::Failed(_)), "{pulled:?}");
    assert_eq!(rev(&clone, "HEAD"), head);
    assert_eq!(text(&clone.join("theirs.txt")), "my own notes\n");
}

#[test]
fn the_clone_s_own_post_merge_hook_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = behind(root, "repo");
    let hooks = root.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let marker = root.join("post-merge-ran");
    let marker_for_sh = marker.to_string_lossy().replace('\\', "/");
    let hook = hooks.join("post-merge");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch '{marker_for_sh}'\n")).unwrap();
    make_executable(&hook);
    git(
        &clone,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );

    let pulled = fast_forward(&clone, &plan(&clone).unwrap().unwrap()).unwrap();

    assert!(matches!(pulled, Pulled::Updated), "{pulled:?}");
    assert!(marker.exists(), "the clone's post-merge hook did not run");
}

#[test]
fn only_the_commits_seen_when_planning_are_applied() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = behind(root, "repo");
    let planned = plan(&clone).unwrap().unwrap();
    let seen = rev(&clone, "@{upstream}");
    push_file_from_elsewhere(root, "repo", "later.txt", "later\n");
    git(&clone, &["fetch", "-q"]);

    let pulled = fast_forward(&clone, &planned).unwrap();

    assert!(matches!(pulled, Pulled::Updated), "{pulled:?}");
    assert_eq!(rev(&clone, "HEAD"), seen);
    assert!(!clone.join("later.txt").exists());
}

#[test]
fn a_clone_that_turned_dirty_after_planning_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = behind(tmp.path(), "repo");
    let planned = plan(&clone).unwrap().unwrap();
    std::fs::write(
        clone.join("README.md"),
        "edited while the prompt was open\n",
    )
    .unwrap();
    let head = rev(&clone, "HEAD");

    let pulled = fast_forward(&clone, &planned).unwrap();

    assert!(matches!(pulled, Pulled::Changed(_)), "{pulled:?}");
    assert_eq!(rev(&clone, "HEAD"), head);
}

#[test]
fn a_clone_whose_head_moved_after_planning_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = behind(tmp.path(), "repo");
    let planned = plan(&clone).unwrap().unwrap();
    commit_file(&clone, "mine.txt", "mine\n");
    git(&clone, &["reset", "-q", "--hard", "HEAD~1"]);
    git(&clone, &["switch", "-q", "-c", "elsewhere"]);
    commit_file(&clone, "other.txt", "x\n");
    let head = rev(&clone, "HEAD");

    let pulled = fast_forward(&clone, &planned).unwrap();

    assert!(matches!(pulled, Pulled::Changed(_)), "{pulled:?}");
    assert_eq!(rev(&clone, "HEAD"), head);
}

/// A file's text with line endings as committed, since checkout may turn `\n` into `\r\n`
/// where `core.autocrlf` is on (Git for Windows sets it system-wide).
fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
