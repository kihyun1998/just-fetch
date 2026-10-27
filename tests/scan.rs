mod common;

use common::{clone_with_upstream, empty_clone, git};
use just_fetch::scan::{ScanError, scan};
use just_fetch::status::Head;

fn names(found: &[just_fetch::scan::FoundClone]) -> Vec<String> {
    found.iter().map(|f| f.name.clone()).collect()
}

#[test]
fn a_clone_given_directly_is_the_only_clone() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = empty_clone(tmp.path(), "solo");
    empty_clone(&clone, "nested");

    let found = scan(&clone).unwrap();

    assert_eq!(names(&found), ["solo"]);
}

#[test]
fn a_folder_lists_the_clones_directly_under_it_in_name_order() {
    let tmp = tempfile::tempdir().unwrap();
    empty_clone(tmp.path(), "beta");
    empty_clone(tmp.path(), "alpha");
    std::fs::create_dir_all(tmp.path().join("plain")).unwrap();
    empty_clone(&tmp.path().join("plain"), "too-deep");
    std::fs::write(tmp.path().join("file.txt"), "").unwrap();

    let found = scan(tmp.path()).unwrap();

    assert_eq!(names(&found), ["alpha", "beta"]);
}

#[test]
fn a_worktree_counts_as_a_clone() {
    let tmp = tempfile::tempdir().unwrap();
    let main = clone_with_upstream(tmp.path(), "main-clone");
    git(&main, &["worktree", "add", "-q", "../wt", "-b", "side"]);

    let found = scan(tmp.path()).unwrap();

    assert_eq!(names(&found), ["main-clone", "wt"]);
}

#[test]
fn reads_branch_upstream_and_dirty_from_real_git() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = clone_with_upstream(tmp.path(), "repo");
    std::fs::write(clone.join("README.md"), "changed\n").unwrap();
    std::fs::write(clone.join("new.txt"), "").unwrap();

    let found = scan(&clone).unwrap();
    let state = found[0].state.as_ref().unwrap();

    assert_eq!(state.head, Head::Branch("main".into()));
    assert_eq!(state.upstream.as_deref(), Some("origin/main"));
    assert!(state.dirty);
    assert_eq!(state.untracked, 1);
}

#[test]
fn a_clone_git_cannot_read_gets_an_error_and_the_others_still_list() {
    let tmp = tempfile::tempdir().unwrap();
    empty_clone(tmp.path(), "good");
    let broken = tmp.path().join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join(".git"), "gitdir: does/not/exist\n").unwrap();

    let found = scan(tmp.path()).unwrap();

    assert_eq!(names(&found), ["broken", "good"]);
    assert!(found[0].state.is_err());
    assert!(found[1].state.is_ok());
}

#[test]
fn a_missing_scan_root_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();

    let err = scan(&tmp.path().join("nope")).unwrap_err();

    assert!(matches!(err, ScanError::Unreadable(_, _)));
}

#[test]
fn every_untracked_file_counts_even_inside_a_new_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = empty_clone(tmp.path(), "repo");
    std::fs::create_dir_all(clone.join("notes")).unwrap();
    for name in ["a.md", "b.md", "c.md"] {
        std::fs::write(clone.join("notes").join(name), "").unwrap();
    }

    let found = scan(&clone).unwrap();

    assert_eq!(found[0].state.as_ref().unwrap().untracked, 3);
}

#[test]
fn a_file_given_as_the_scan_root_is_unreadable_not_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file.txt");
    std::fs::write(&file, "").unwrap();

    let err = scan(&file).unwrap_err();

    assert!(matches!(err, ScanError::Unreadable(_, _)), "{err:?}");
}

#[test]
fn a_clone_configured_to_run_an_fsmonitor_command_does_not_run_it() {
    let tmp = tempfile::tempdir().unwrap();
    let clone = empty_clone(tmp.path(), "repo");
    let marker = tmp.path().join("fsmonitor-ran");
    let marker_for_sh = marker.to_string_lossy().replace('\\', "/");
    git(
        &clone,
        &[
            "config",
            "core.fsmonitor",
            &format!("touch '{marker_for_sh}'; false"),
        ],
    );

    let found = scan(&clone).unwrap();

    assert!(found[0].state.is_ok());
    assert!(!marker.exists(), "scan ran the clone's fsmonitor command");
}
