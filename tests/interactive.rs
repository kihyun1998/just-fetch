//! The selection prompt, driven through a pseudo-terminal the way a person would use it:
//! a Unix pty, or ConPTY on Windows.

mod common;

use std::io::{ErrorKind, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{clone_with_upstream, push_file_from_elsewhere};
use expectrl::process::Healthcheck;
use expectrl::session::OsSession;
use expectrl::{Expect, Regex, Session};

const TIMEOUT: Duration = Duration::from_secs(20);

/// The prompt's last row. ConPTY may redraw a run of spaces as a cursor move, so the gap
/// between the name and its standing is matched as either.
const BETA_ROW: &str = r"beta(?:\s|\x1b\[[0-9;]*[A-Za-z])+behind 1";

/// A running just-fetch whose prompt is up, and everything it showed up to the prompt.
struct Prompt {
    session: OsSession,
    shown: String,
}

impl Prompt {
    fn open(root: &Path) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_just-fetch"));
        command.arg(root).env("TERM", "xterm-256color");
        let mut session = Session::spawn(command).unwrap();
        session.set_expect_timeout(Some(TIMEOUT));
        let mut shown = String::new();
        for needle in ["fast-forward", BETA_ROW] {
            let found = session.expect(Regex(needle)).unwrap();
            shown.push_str(&String::from_utf8_lossy(found.before()));
            shown.push_str(&String::from_utf8_lossy(found.get(0).unwrap_or_default()));
        }
        Self { session, shown }
    }

    fn press(&mut self, keys: &str) {
        self.session.send(keys).unwrap();
        self.session.flush().unwrap();
    }

    /// Waits for just-fetch to exit and returns what it showed after the prompt.
    fn finish(self) -> String {
        drain_until_exit(self.session)
    }
}

/// Reads `session` until its process exits. Waiting for end-of-file is not enough: ConPTY
/// can keep its output open after the child is gone.
fn drain_until_exit(mut session: OsSession) -> String {
    let deadline = Instant::now() + TIMEOUT;
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        assert!(
            Instant::now() < deadline,
            "just-fetch did not exit:\n{}",
            String::from_utf8_lossy(&out)
        );
        match session.try_read(&mut buf) {
            Ok(0) => {}
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                continue;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break,
        }
        if !session.get_process().is_alive().unwrap_or(false) {
            while let Ok(n) = session.try_read(&mut buf) {
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n]);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn two_behind(root: &Path) {
    for name in ["alpha", "beta"] {
        clone_with_upstream(root, name);
        push_file_from_elsewhere(root, name, "theirs.txt", "theirs\n");
    }
}

fn pulled(root: &Path, name: &str) -> bool {
    root.join(name).join("theirs.txt").exists()
}

#[test]
fn picking_one_clone_fast_forwards_only_that_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut prompt = Prompt::open(root);
    prompt.press(" ");
    prompt.press("\r");
    let after = prompt.finish();

    assert!(after.contains("alpha: updated"), "{after}");
    assert!(pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn leaving_without_picking_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut prompt = Prompt::open(root);
    prompt.press("\x1b");
    prompt.finish();

    assert!(!pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn confirming_with_nothing_picked_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut prompt = Prompt::open(root);
    prompt.press("\r");
    prompt.finish();

    assert!(!pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn a_clone_that_cannot_be_pulled_is_not_offered() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);
    let dirty = clone_with_upstream(root, "aaa-dirty");
    push_file_from_elsewhere(root, "aaa-dirty", "theirs.txt", "theirs\n");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();

    let mut prompt = Prompt::open(root);
    let offered = prompt.shown[prompt.shown.find("fast-forward").expect("no prompt")..].to_string();
    prompt.press("\x1b");
    prompt.finish();

    assert!(offered.contains("alpha"), "{offered}");
    assert!(
        !offered.contains("aaa-dirty"),
        "a dirty clone was offered:\n{offered}"
    );
}

#[test]
fn a_clone_edited_while_the_prompt_is_open_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut prompt = Prompt::open(root);
    std::fs::write(root.join("alpha").join("README.md"), "edited meanwhile\n").unwrap();
    prompt.press(" ");
    prompt.press("\r");
    let after = prompt.finish();

    assert!(after.contains("alpha: skipped: now dirty"), "{after}");
    assert!(!pulled(root, "alpha"));
}

#[test]
fn a_refused_fast_forward_is_reported_as_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut prompt = Prompt::open(root);
    std::fs::write(root.join("alpha").join("theirs.txt"), "my own notes\n").unwrap();
    prompt.press(" ");
    prompt.press("\r");
    let after = prompt.finish();

    assert!(after.contains("alpha: failed: "), "{after}");
    assert_eq!(
        std::fs::read_to_string(root.join("alpha").join("theirs.txt")).unwrap(),
        "my own notes\n"
    );
}

#[test]
fn with_output_piped_away_nothing_is_offered_or_pulled() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let session = Session::spawn(piped(root)).unwrap();
    let shown = drain_until_exit(session);

    assert!(shown.contains("alpha"), "{shown}");
    assert!(
        !shown.contains("fast-forward"),
        "prompted with output piped:\n{shown}"
    );
    assert!(!pulled(root, "alpha"));
}

/// just-fetch on `root` inside the terminal, with its output piped through another program.
#[cfg(unix)]
fn piped(root: &Path) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!(
            "'{}' '{}' | cat",
            env!("CARGO_BIN_EXE_just-fetch"),
            root.display()
        ))
        .env("TERM", "xterm-256color");
    command
}

#[cfg(windows)]
fn piped(root: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("cmd");
    command.raw_arg(format!(
        "/C \"\"{}\" \"{}\" | more\"",
        env!("CARGO_BIN_EXE_just-fetch"),
        root.display()
    ));
    command
}
