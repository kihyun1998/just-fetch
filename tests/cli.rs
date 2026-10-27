mod common;

use std::process::{Command, Output};

use common::{
    clone_with_upstream, commit, commit_file, empty_clone, git, push_file_from_elsewhere,
    push_from_elsewhere, remote_of,
};

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

#[test]
fn a_diverged_row_says_whether_it_would_merge_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clashing = clone_with_upstream(root, "clashing");
    commit_file(&clashing, "README.md", "mine\n");
    push_file_from_elsewhere(root, "clashing", "README.md", "theirs\n");
    let separate = clone_with_upstream(root, "separate");
    commit_file(&separate, "mine.txt", "a\n");
    push_file_from_elsewhere(root, "separate", "theirs.txt", "b\n");
    clone_with_upstream(root, "behind");
    push_file_from_elsewhere(root, "behind", "README.md", "theirs\n");

    let out = just_fetch().arg(root).output().unwrap();
    let text = stdout(&out);

    assert!(
        row(&text, "clashing").contains("conflict: README.md"),
        "{text}"
    );
    assert!(row(&text, "separate").contains("merges cleanly"), "{text}");
    assert!(!row(&text, "behind").contains("conflict"), "{text}");
    assert!(!row(&text, "behind").contains("merges cleanly"), "{text}");
}

#[test]
fn a_long_conflict_list_is_cut_short_with_a_count() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "many");
    let names = ["a", "b", "c", "d", "e", "f", "g"];
    for name in names {
        std::fs::write(clone.join(name), "base\n").unwrap();
    }
    git(&clone, &["add", "."]);
    git(&clone, &["commit", "-q", "-m", "base files"]);
    git(&clone, &["push", "-q"]);
    for name in names {
        std::fs::write(clone.join(name), "mine\n").unwrap();
    }
    git(&clone, &["commit", "-q", "-am", "mine"]);
    for name in names {
        push_file_from_elsewhere(root, "many", name, "theirs\n");
    }

    let out = just_fetch().arg(root).output().unwrap();
    let many = row(&stdout(&out), "many").to_string();

    assert!(many.contains("conflict: a, b, c, d, e (+2 more)"), "{many}");
}

#[test]
fn a_diverged_clone_whose_fetch_failed_gets_no_prediction() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "stale");
    commit_file(&clone, "README.md", "mine\n");
    push_file_from_elsewhere(root, "stale", "README.md", "theirs\n");
    git(&clone, &["fetch", "-q"]);
    git(
        &clone,
        &["remote", "set-url", "origin", "does/not/exist.git"],
    );

    let out = just_fetch().arg(root).output().unwrap();
    let stale = row(&stdout(&out), "stale").to_string();

    assert!(stale.contains("fetch failed: "), "{stale}");
    assert!(!stale.contains("conflict"), "{stale}");
}

#[cfg(unix)]
#[test]
fn a_conflicted_file_with_a_newline_in_its_name_stays_on_its_row() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "odd");
    commit_file(&clone, "two\nlines", "base\n");
    git(&clone, &["push", "-q"]);
    commit_file(&clone, "two\nlines", "mine\n");
    push_file_from_elsewhere(root, "odd", "two\nlines", "theirs\n");

    let out = just_fetch().arg(root).output().unwrap();
    let text = stdout(&out);

    assert_eq!(text.lines().count(), 1, "one clone, one row:\n{text}");
    assert!(
        row(&text, "odd").contains("conflict: two\\nlines"),
        "{text}"
    );
}

#[test]
fn without_a_terminal_nothing_is_pulled() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    clone_with_upstream(root, "behind");
    push_file_from_elsewhere(root, "behind", "theirs.txt", "theirs\n");

    let out = just_fetch().arg(root).output().unwrap();

    assert!(out.status.success());
    assert!(row(&stdout(&out), "behind").contains("behind 1"));
    assert!(!root.join("behind").join("theirs.txt").exists());
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
