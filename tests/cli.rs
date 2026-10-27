mod common;

use std::process::{Command, Output};

use common::{clone_with_upstream, commit, empty_clone, git, push_from_elsewhere, remote_of};

fn just_fetch() -> Command {
    Command::new(env!("CARGO_BIN_EXE_just-fetch"))
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn row<'a>(text: &'a str, name: &str) -> &'a str {
    text.lines()
        .find(|line| line.split_whitespace().next() == Some(name))
        .unwrap_or_else(|| panic!("no row for {name} in:\n{text}"))
}

#[test]
fn with_no_path_the_current_folder_is_the_scan_root() {
    let tmp = tempfile::tempdir().unwrap();
    empty_clone(tmp.path(), "here");

    let out = just_fetch().current_dir(tmp.path()).output().unwrap();

    assert!(out.status.success());
    row(&stdout(&out), "here");
}

#[test]
fn each_row_shows_branch_upstream_and_dirty() {
    let tmp = tempfile::tempdir().unwrap();
    let tracked = clone_with_upstream(tmp.path(), "tracked");
    std::fs::write(tracked.join("README.md"), "changed\n").unwrap();
    empty_clone(tmp.path(), "lonely");

    let out = just_fetch().arg(tmp.path()).output().unwrap();
    let text = stdout(&out);

    let tracked_row = row(&text, "tracked");
    assert!(tracked_row.contains("main"), "{tracked_row}");
    assert!(tracked_row.contains("origin/main"), "{tracked_row}");
    assert!(tracked_row.contains("dirty"), "{tracked_row}");

    let lonely_row = row(&text, "lonely");
    assert!(lonely_row.contains("no upstream"), "{lonely_row}");
    assert!(!lonely_row.contains("dirty"), "{lonely_row}");
}

#[test]
fn without_git_on_path_it_says_so_and_fails() {
    let tmp = tempfile::tempdir().unwrap();
    empty_clone(tmp.path(), "repo");

    let out = just_fetch()
        .arg(tmp.path())
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("git was not found on PATH"));
}

#[test]
fn a_missing_path_is_named_in_the_error() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nope");

    let out = just_fetch().arg(&missing).output().unwrap();

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("nope"));
}

#[test]
fn run_inside_a_clone_with_no_path_names_the_clone_not_dot() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = empty_clone(tmp.path(), "inside");

    let out = just_fetch().current_dir(&clone).output().unwrap();

    row(&stdout(&out), "inside");
}

#[test]
fn a_file_given_as_the_path_is_not_called_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file.txt");
    std::fs::write(&file, "").unwrap();

    let out = just_fetch().arg(&file).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success());
    assert!(err.contains("file.txt"), "{err}");
    assert!(!err.contains("no such folder"), "{err}");
}

#[test]
fn each_row_says_how_the_clone_stands_against_its_upstream() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    clone_with_upstream(root, "same");
    clone_with_upstream(root, "behind");
    push_from_elsewhere(root, "behind", "main");
    let ahead = clone_with_upstream(root, "ahead");
    commit(&ahead, "local only");
    let diverged = clone_with_upstream(root, "diverged");
    commit(&diverged, "local only");
    push_from_elsewhere(root, "diverged", "main");
    let gone = clone_with_upstream(root, "gone");
    git(&gone, &["switch", "-q", "-c", "side"]);
    git(&gone, &["push", "-q", "-u", "origin", "side"]);
    git(&remote_of(root, "gone"), &["branch", "-q", "-D", "side"]);
    let broken = clone_with_upstream(root, "broken");
    git(
        &broken,
        &["remote", "set-url", "origin", "does/not/exist.git"],
    );

    let out = just_fetch().arg(root).output().unwrap();
    let text = stdout(&out);

    assert!(out.status.success());
    assert!(row(&text, "same").contains("up to date"));
    assert!(row(&text, "behind").contains("behind 1"));
    assert!(row(&text, "ahead").contains("ahead 1"));
    assert!(row(&text, "diverged").contains("diverged +1 -1"));
    assert!(row(&text, "gone").contains("upstream gone"));
    assert!(row(&text, "broken").contains("fetch failed: "));
    assert!(!row(&text, "broken").contains("up to date"));
}

#[test]
fn a_many_line_git_message_stays_on_its_clone_row() {
    let tmp = tempfile::tempdir().unwrap();
    let broken = clone_with_upstream(tmp.path(), "broken");
    git(
        &broken,
        &["remote", "set-url", "origin", "does/not/exist.git"],
    );

    let out = just_fetch().arg(tmp.path()).output().unwrap();
    let text = stdout(&out);

    assert_eq!(text.lines().count(), 1, "one clone, one row:\n{text}");
    assert!(row(&text, "broken").contains("fetch failed: "));
}
