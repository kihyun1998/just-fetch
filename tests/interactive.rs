//! The selection prompt, driven through a pseudo-terminal the way a person would use it.
#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::Command;

use common::{clone_with_upstream, push_file_from_elsewhere};
use rexpect::session::{PtySession, spawn_command};

const TIMEOUT_MS: u64 = 20_000;

/// Starts just-fetch on `root` and waits for the prompt; returns everything shown up to it.
fn open(root: &Path) -> (PtySession, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_just-fetch"));
    command.arg(root).env("TERM", "xterm-256color");
    let mut session = spawn_command(command, Some(TIMEOUT_MS)).unwrap();
    let before = session.exp_string("fast-forward").unwrap();
    let after = session.exp_string("beta  behind 1").unwrap();
    (session, format!("{before}fast-forward{after}"))
}

fn press(session: &mut PtySession, keys: &str) {
    session.send(keys).unwrap();
    session.flush().unwrap();
}

fn two_behind(root: &Path) {
    for name in ["alpha", "beta"] {
        clone_with_upstream(root, name);
        push_file_from_elsewhere(root, name, "theirs.txt", "theirs\n");
    }
}

#[test]
fn picking_one_clone_fast_forwards_only_that_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let (mut session, _) = open(root);
    press(&mut session, " ");
    press(&mut session, "\r");
    session.exp_string("alpha: updated").unwrap();
    session.exp_eof().unwrap();

    assert!(root.join("alpha").join("theirs.txt").exists());
    assert!(!root.join("beta").join("theirs.txt").exists());
}

#[test]
fn leaving_without_picking_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let (mut session, _) = open(root);
    press(&mut session, "\x1b");
    session.exp_eof().unwrap();

    assert!(!root.join("alpha").join("theirs.txt").exists());
    assert!(!root.join("beta").join("theirs.txt").exists());
}

#[test]
fn confirming_with_nothing_picked_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let (mut session, _) = open(root);
    press(&mut session, "\r");
    session.exp_eof().unwrap();

    assert!(!root.join("alpha").join("theirs.txt").exists());
    assert!(!root.join("beta").join("theirs.txt").exists());
}

#[test]
fn a_clone_that_cannot_be_pulled_is_not_offered() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);
    let dirty = clone_with_upstream(root, "aaa-dirty");
    push_file_from_elsewhere(root, "aaa-dirty", "theirs.txt", "theirs\n");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();

    let (mut session, shown) = open(root);
    press(&mut session, "\x1b");
    session.exp_eof().unwrap();

    let prompt = &shown[shown.find("fast-forward").expect("no prompt")..];
    assert!(prompt.contains("alpha  behind 1"), "{prompt}");
    assert!(
        !prompt.contains("aaa-dirty"),
        "a dirty clone was offered:\n{prompt}"
    );
}

#[test]
fn with_output_piped_away_nothing_is_offered_or_pulled() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!(
            "'{}' '{}' | cat",
            env!("CARGO_BIN_EXE_just-fetch"),
            root.display()
        ))
        .env("TERM", "xterm-256color");
    let mut session = spawn_command(command, Some(TIMEOUT_MS)).unwrap();

    let shown = session.exp_eof().unwrap();

    assert!(shown.contains("alpha"), "{shown}");
    assert!(
        !shown.contains("fast-forward"),
        "prompted with output piped:\n{shown}"
    );
    assert!(!root.join("alpha").join("theirs.txt").exists());
}

#[test]
fn a_clone_edited_while_the_prompt_is_open_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let (mut session, _) = open(root);
    std::fs::write(root.join("alpha").join("README.md"), "edited meanwhile\n").unwrap();
    press(&mut session, " ");
    press(&mut session, "\r");
    session.exp_string("alpha: skipped: now dirty").unwrap();
    session.exp_eof().unwrap();

    assert!(!root.join("alpha").join("theirs.txt").exists());
}

#[test]
fn a_refused_fast_forward_is_reported_as_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let (mut session, _) = open(root);
    std::fs::write(root.join("alpha").join("theirs.txt"), "my own notes\n").unwrap();
    press(&mut session, " ");
    press(&mut session, "\r");
    session.exp_string("alpha: failed: ").unwrap();
    session.exp_eof().unwrap();

    assert_eq!(
        std::fs::read_to_string(root.join("alpha").join("theirs.txt")).unwrap(),
        "my own notes\n"
    );
}
