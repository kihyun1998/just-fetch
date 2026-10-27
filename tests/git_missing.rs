//! git going missing after the scan.

mod common;

use common::{clone_with_upstream, commit_file, git, push_file_from_elsewhere};
use just_fetch::fetch::fetch_all;
use just_fetch::predict::predict;
use just_fetch::scan::scan;

#[test]
fn git_vanishing_after_the_scan_stops_fetch_and_prediction() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    commit_file(&clone, "README.md", "mine\n");
    push_file_from_elsewhere(root, "repo", "README.md", "theirs\n");
    git(&clone, &["fetch", "-q"]);
    let found = scan(root).unwrap();

    // SAFETY: the only test in this crate, so no other thread reads the environment.
    unsafe { std::env::set_var("PATH", "") };

    let fetched = fetch_all(found).expect_err("fetch went on without git");
    assert_eq!(fetched.to_string(), "git was not found on PATH");
    let predicted = predict(&clone, Some((2, 50))).expect_err("prediction went on without git");
    assert_eq!(predicted.to_string(), "git was not found on PATH");
}
