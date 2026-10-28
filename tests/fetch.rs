mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{
    clone_with_upstream, commit, empty_clone, fake_ssh, fetch_over_fake_ssh, for_sh, git, git_ok,
    make_executable, push_from_elsewhere, remote_of,
};
use just_fetch::fetch::{Fetch, Refreshed, fetch_all};
use just_fetch::scan::scan;
use just_fetch::status::CloneStatus;

fn refresh(root: &Path) -> Vec<Refreshed> {
    fetch_all(scan(root).unwrap()).unwrap()
}

fn status_of<'a>(refreshed: &'a [Refreshed], name: &str) -> (CloneStatus, &'a Fetch) {
    let r = refreshed
        .iter()
        .find(|r| r.found.name == name)
        .unwrap_or_else(|| panic!("no clone named {name}"));
    (r.found.state.as_ref().unwrap().clone_status(), &r.fetch)
}

#[test]
fn fetch_reveals_behind_ahead_diverged_and_up_to_date() {
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

    let refreshed = refresh(root);

    assert_eq!(status_of(&refreshed, "same").0, CloneStatus::UpToDate);
    assert_eq!(status_of(&refreshed, "behind").0, CloneStatus::Behind(1));
    assert_eq!(status_of(&refreshed, "ahead").0, CloneStatus::Ahead(1));
    assert_eq!(
        status_of(&refreshed, "diverged").0,
        CloneStatus::Diverged {
            ahead: 1,
            behind: 1
        }
    );
    assert!(matches!(status_of(&refreshed, "behind").1, Fetch::Fetched));
}

#[test]
fn an_upstream_branch_deleted_on_the_remote_is_gone() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    git(&clone, &["switch", "-q", "-c", "side"]);
    git(&clone, &["push", "-q", "-u", "origin", "side"]);
    git(&remote_of(root, "repo"), &["branch", "-q", "-D", "side"]);

    let refreshed = refresh(root);

    assert_eq!(status_of(&refreshed, "repo").0, CloneStatus::GoneUpstream);
}

#[test]
fn a_failed_fetch_carries_git_message_and_the_others_still_fetch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let broken = clone_with_upstream(root, "broken");
    git(
        &broken,
        &["remote", "set-url", "origin", "does/not/exist.git"],
    );
    clone_with_upstream(root, "fine");
    push_from_elsewhere(root, "fine", "main");

    let refreshed = refresh(root);

    match status_of(&refreshed, "broken").1 {
        Fetch::Failed(message) => assert!(message.contains("does/not/exist.git"), "{message}"),
        other => panic!("expected a failed fetch, got {other:?}"),
    }
    assert_eq!(status_of(&refreshed, "fine").0, CloneStatus::Behind(1));
}

#[test]
fn a_clone_with_no_upstream_is_not_fetched() {
    let tmp = tempfile::tempdir().unwrap();
    empty_clone(tmp.path(), "lonely");

    let refreshed = refresh(tmp.path());

    let (status, fetch) = status_of(&refreshed, "lonely");
    assert_eq!(status, CloneStatus::NoUpstream);
    assert!(matches!(fetch, Fetch::Skipped), "{fetch:?}");
}

#[test]
fn only_the_upstream_remote_is_fetched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    clone_with_upstream(root, "other");
    git(
        &clone,
        &[
            "remote",
            "add",
            "other",
            remote_of(root, "other").to_str().unwrap(),
        ],
    );

    refresh(root);

    let other_refs = std::process::Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["for-each-ref", "refs/remotes/other"])
        .output()
        .unwrap();
    assert!(other_refs.status.success());
    assert!(
        other_refs.stdout.is_empty(),
        "fetched a remote that is not the upstream: {}",
        String::from_utf8_lossy(&other_refs.stdout)
    );
}

#[test]
fn an_upstream_on_another_remote_is_fetched_from_that_remote() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    clone_with_upstream(root, "fork");
    git(
        &clone,
        &[
            "remote",
            "add",
            "fork",
            remote_of(root, "fork").to_str().unwrap(),
        ],
    );
    git(&clone, &["fetch", "-q", "fork"]);
    git(
        &clone,
        &["switch", "-q", "-c", "from-fork", "--track", "fork/main"],
    );
    push_from_elsewhere(root, "fork", "main");
    push_from_elsewhere(root, "repo", "main");

    let refreshed = refresh(root);

    let r = refreshed.iter().find(|r| r.found.name == "repo").unwrap();
    let state = r.found.state.as_ref().unwrap();
    assert_eq!(state.upstream.as_deref(), Some("fork/main"));
    assert_eq!(state.clone_status(), CloneStatus::Behind(1));
    let origin_main = std::process::Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["rev-list", "--count", "main..origin/main"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&origin_main.stdout).trim(),
        "0",
        "origin was fetched though the upstream is on fork"
    );
}

#[test]
fn a_hook_the_clone_configures_does_not_run_during_fetch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    let marker = root.join("hook-ran");
    let hooks = root.join("planted-hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    let marker_for_sh = marker.to_string_lossy().replace('\\', "/");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch '{marker_for_sh}'\n")).unwrap();
    make_executable(&hook);
    // The clone's own config names the hooks, so no global setting can hide them.
    git(
        &clone,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    git(&clone, &["fetch", "-q"]);
    assert!(
        marker.exists(),
        "a plain fetch should have run the planted hook"
    );
    std::fs::remove_file(&marker).unwrap();
    push_from_elsewhere(root, "repo", "main");

    let refreshed = refresh(root);

    assert_eq!(status_of(&refreshed, "repo").0, CloneStatus::Behind(1));
    assert!(!marker.exists(), "fetch ran the clone's hook");
}

#[test]
fn clones_are_fetched_at_the_same_time() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let ssh = fake_ssh(root, 2);
    for name in ["a", "b", "c", "d"] {
        let clone = clone_with_upstream(root, name);
        fetch_over_fake_ssh(&clone, &remote_of(root, name), &ssh);
    }

    let started = Instant::now();
    let refreshed = refresh(root);
    let elapsed = started.elapsed();

    for name in ["a", "b", "c", "d"] {
        assert!(matches!(status_of(&refreshed, name).1, Fetch::Fetched));
    }
    assert!(
        elapsed < Duration::from_secs(6),
        "four 2-second fetches took {elapsed:?}; one at a time would take at least 8s"
    );
}

/// A command that leaves `marker` behind when it runs, for planting in a clone's config.
fn planted(marker: &Path) -> String {
    format!("touch '{}'", for_sh(marker))
}

#[test]
fn a_local_remote_s_planted_upload_pack_does_not_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    let marker = root.join("upload-pack-ran");
    let pack = format!("{}; git-upload-pack", planted(&marker));
    git(&clone, &["config", "remote.origin.uploadpack", &pack]);
    assert!(git_ok(&clone, &["fetch", "-q"]));
    assert!(
        marker.exists(),
        "a plain fetch should have run the planted upload-pack"
    );
    std::fs::remove_file(&marker).unwrap();
    push_from_elsewhere(root, "repo", "main");

    let refreshed = refresh(root);

    assert_eq!(status_of(&refreshed, "repo").0, CloneStatus::Behind(1));
    assert!(!marker.exists(), "fetch ran the clone's upload-pack");
}

#[test]
fn a_non_local_remote_keeps_its_configured_upload_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    fetch_over_fake_ssh(&clone, &remote_of(root, "repo"), &fake_ssh(root, 0));
    let marker = root.join("upload-pack-ran");
    let pack = format!("{}; git-upload-pack", planted(&marker));
    git(&clone, &["config", "remote.origin.uploadpack", &pack]);

    let refreshed = refresh(root);

    assert!(matches!(status_of(&refreshed, "repo").1, Fetch::Fetched));
    assert!(
        marker.exists(),
        "an ssh remote's upload-pack should be left alone"
    );
}

#[test]
fn a_submodule_is_not_fetched_into() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    clone_with_upstream(root, "sub");
    git(
        &clone,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            remote_of(root, "sub").to_str().unwrap(),
            "sub",
        ],
    );
    commit(&clone, "add sub");
    git(&clone, &["push", "-q"]);
    let inner = clone.join("sub");
    let marker = root.join("submodule-upload-pack-ran");
    let pack = format!("{}; git-upload-pack", planted(&marker));
    git(&inner, &["config", "remote.origin.uploadpack", &pack]);
    git(&inner, &["config", "protocol.file.allow", "always"]);
    git(&clone, &["config", "fetch.recurseSubmodules", "yes"]);
    assert!(git_ok(&clone, &["fetch", "-q"]));
    assert!(
        marker.exists(),
        "a plain fetch should have recursed into the submodule"
    );
    std::fs::remove_file(&marker).unwrap();

    refresh(root);

    assert!(!marker.exists(), "fetch recursed into the submodule");
}

#[test]
fn an_ext_remote_the_clone_allows_is_not_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    let marker = root.join("ext-ran");
    let remote = for_sh(&remote_of(root, "repo"));
    let url = format!(
        "ext::sh -c {};git-upload-pack% {remote}",
        planted(&marker).replace(' ', "% ")
    );
    git(&clone, &["remote", "set-url", "origin", &url]);
    git(&clone, &["config", "protocol.ext.allow", "always"]);
    assert!(git_ok(&clone, &["fetch", "-q"]));
    assert!(
        marker.exists(),
        "a plain fetch should have run the ext remote"
    );
    std::fs::remove_file(&marker).unwrap();

    let refreshed = refresh(root);

    assert!(matches!(status_of(&refreshed, "repo").1, Fetch::Failed(_)));
    assert!(!marker.exists(), "fetch ran the clone's ext remote");
}

#[test]
fn a_clone_that_sets_its_own_git_proxy_is_not_fetched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "git://example.invalid/repo.git",
        ],
    );
    let marker = root.join("proxy-ran");
    let proxy = root.join("proxy.sh");
    std::fs::write(
        &proxy,
        format!(
            "#!/bin/sh
{}
exit 1
",
            planted(&marker)
        ),
    )
    .unwrap();
    make_executable(&proxy);
    git(&clone, &["config", "core.gitProxy", &for_sh(&proxy)]);
    git_ok(&clone, &["fetch", "-q"]);
    assert!(marker.exists(), "a plain fetch should have run the proxy");
    std::fs::remove_file(&marker).unwrap();

    let refreshed = refresh(root);

    match status_of(&refreshed, "repo").1 {
        Fetch::Failed(message) => assert!(message.contains("core.gitProxy"), "{message}"),
        other => panic!("expected a refused fetch, got {other:?}"),
    }
    assert!(!marker.exists(), "fetch ran the clone's proxy");
}

#[test]
fn only_the_upstream_remote_is_fetched_even_when_the_clone_asks_for_all() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    clone_with_upstream(root, "other");
    git(
        &clone,
        &[
            "remote",
            "add",
            "other",
            remote_of(root, "other").to_str().unwrap(),
        ],
    );
    git(&clone, &["config", "fetch.all", "true"]);

    refresh(root);

    let other_refs = std::process::Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["for-each-ref", "refs/remotes/other"])
        .output()
        .unwrap();
    assert!(
        other_refs.stdout.is_empty(),
        "fetched a remote that is not the upstream: {}",
        String::from_utf8_lossy(&other_refs.stdout)
    );
}

#[test]
fn a_clone_that_sets_its_own_remote_helper_is_not_fetched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    git(&clone, &["config", "remote.origin.vcs", "unknown-helper"]);

    let refreshed = refresh(root);

    match status_of(&refreshed, "repo").1 {
        Fetch::Failed(message) => assert!(message.contains("remote.origin.vcs"), "{message}"),
        other => panic!("expected a refused fetch, got {other:?}"),
    }
}

#[test]
fn a_clone_that_sets_its_own_alternate_refs_command_is_not_fetched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let clone = clone_with_upstream(root, "repo");
    git(&clone, &["config", "core.alternateRefsCommand", "unused"]);

    let refreshed = refresh(root);

    match status_of(&refreshed, "repo").1 {
        Fetch::Failed(message) => {
            assert!(message.contains("core.alternateRefsCommand"), "{message}")
        }
        other => panic!("expected a refused fetch, got {other:?}"),
    }
}
