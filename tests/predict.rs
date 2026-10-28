mod common;

use std::path::Path;
use std::process::Command;

use common::{clone_with_upstream, commit_file, git, push_file_from_elsewhere};
use just_fetch::predict::{GitVersion, Prediction, git_version, predict};

/// A clone of `name` diverged from its remote: one local commit and one pushed from
/// elsewhere, both fetched.
fn diverged(
    root: &Path,
    name: &str,
    local: (&str, &str),
    remote: (&str, &str),
) -> std::path::PathBuf {
    let clone = clone_with_upstream(root, name);
    commit_file(&clone, local.0, local.1);
    push_file_from_elsewhere(root, name, remote.0, remote.1);
    git(&clone, &["fetch", "-q"]);
    clone
}

fn predicted(clone: &Path) -> Prediction {
    predict(clone, git_version().unwrap()).unwrap()
}

#[test]
fn the_same_line_changed_on_both_sides_is_a_predicted_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = diverged(
        tmp.path(),
        "repo",
        ("README.md", "mine\n"),
        ("README.md", "theirs\n"),
    );

    assert_eq!(
        predicted(&clone),
        Prediction::Conflicts(vec!["README.md".into()])
    );
}

#[test]
fn changes_to_different_files_merge_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = diverged(
        tmp.path(),
        "repo",
        ("mine.txt", "a\n"),
        ("theirs.txt", "b\n"),
    );

    assert_eq!(predicted(&clone), Prediction::Clean);
}

#[test]
fn predicting_changes_nothing_in_the_clone() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = diverged(
        tmp.path(),
        "repo",
        ("README.md", "mine\n"),
        ("README.md", "theirs\n"),
    );
    std::fs::write(clone.join("scratch.txt"), "uncommitted\n").unwrap();
    let before = snapshot(&clone);

    predicted(&clone);

    assert_eq!(snapshot(&clone), before);
}

#[test]
fn a_merge_driver_the_clone_names_does_not_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    commit_file(&clone, ".gitattributes", "README.md merge=planted\n");
    git(&clone, &["push", "-q"]);
    let marker = root.join("driver-ran");
    let marker_for_sh = marker.to_string_lossy().replace('\\', "/");
    git(
        &clone,
        &[
            "config",
            "merge.planted.driver",
            &format!("touch '{marker_for_sh}'; exit 1"),
        ],
    );
    commit_file(&clone, "README.md", "mine\n");
    push_file_from_elsewhere(root, "repo", "README.md", "theirs\n");
    git(&clone, &["fetch", "-q"]);
    let plain = Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["merge-tree", "--write-tree", "HEAD", "@{upstream}"])
        .output()
        .unwrap();
    assert!(plain.status.code().is_some());
    assert!(
        marker.exists(),
        "a plain merge-tree should have run the planted driver"
    );
    std::fs::remove_file(&marker).unwrap();

    predicted(&clone);

    assert!(!marker.exists(), "prediction ran the clone's merge driver");
}

#[test]
fn a_git_older_than_the_minimum_cannot_predict() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = diverged(
        tmp.path(),
        "repo",
        ("README.md", "mine\n"),
        ("README.md", "theirs\n"),
    );

    let prediction = predict(
        &clone,
        Some(GitVersion {
            major: 2,
            minor: 37,
        }),
    )
    .unwrap();
    assert_eq!(prediction, Prediction::NeedsNewerGit);
}

/// Everything a prediction must leave alone: refs, HEAD, index, working tree, objects.
fn snapshot(clone: &Path) -> Vec<String> {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(clone)
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let index = std::fs::read(clone.join(".git").join("index")).unwrap();
    vec![
        run(&["for-each-ref"]),
        run(&["rev-parse", "HEAD"]),
        run(&[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ]),
        format!("{index:?}"),
        run(&["count-objects", "-v"]),
    ]
}

#[test]
fn a_clone_whose_path_holds_the_list_separator_still_predicts() {
    let tmp = tempfile::tempdir().unwrap();
    let separator = if cfg!(windows) { ';' } else { ':' };
    let parent = tmp.path().join(format!("odd{separator}folder"));
    std::fs::create_dir_all(&parent).unwrap();
    let clone = diverged(
        &parent,
        "repo",
        ("README.md", "mine\n"),
        ("README.md", "theirs\n"),
    );

    assert_eq!(
        predicted(&clone),
        Prediction::Conflicts(vec!["README.md".into()])
    );
}

#[test]
fn a_merge_driver_named_in_the_clone_s_info_attributes_does_not_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = diverged(
        root,
        "repo",
        ("README.md", "mine\n"),
        ("README.md", "theirs\n"),
    );
    let info = clone.join(".git").join("info");
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(info.join("attributes"), "README.md merge=planted\n").unwrap();
    let marker = root.join("driver-ran");
    let marker_for_sh = marker.to_string_lossy().replace('\\', "/");
    git(
        &clone,
        &[
            "config",
            "merge.planted.driver",
            &format!("touch '{marker_for_sh}'; exit 1"),
        ],
    );
    Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["merge-tree", "--write-tree", "HEAD", "@{upstream}"])
        .output()
        .unwrap();
    assert!(
        marker.exists(),
        "a plain merge-tree should have run the planted driver"
    );
    std::fs::remove_file(&marker).unwrap();

    predicted(&clone);

    assert!(!marker.exists(), "prediction ran the clone's merge driver");
}
