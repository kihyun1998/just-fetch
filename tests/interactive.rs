//! The selection prompt, driven through a pseudo-terminal the way a person would use it:
//! a Unix pty, or ConPTY on Windows.

mod common;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{clone_with_upstream, push_file_from_elsewhere};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

const TIMEOUT: Duration = Duration::from_secs(30);

/// How long the output must stay unchanged after the process exits before it counts as read.
const SETTLE: Duration = Duration::from_secs(1);

/// A program running in a pseudo-terminal, with everything it has shown so far.
struct Terminal {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    shown: Arc<Mutex<Vec<u8>>>,
    /// Cursor-position queries already answered.
    answered: usize,
    _master: Box<dyn MasterPty + Send>,
}

/// The cursor-position query ConPTY sends when it starts, and waits on until it is answered.
const CURSOR_QUERY: &str = "\x1b[6n";

impl Terminal {
    fn spawn(mut command: CommandBuilder) -> Self {
        command.env("TERM", "xterm-256color");
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 160,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let shown = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&shown);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&buf[..n]);
            }
        });
        Self {
            child,
            writer,
            shown,
            answered: 0,
            _master: pair.master,
        }
    }

    /// Answers each new cursor-position query the way a terminal would, from the top left.
    fn answer_queries(&mut self) {
        let asked = self.text().matches(CURSOR_QUERY).count();
        while self.answered < asked {
            self.press("\x1b[1;1R");
            self.answered += 1;
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.shown.lock().unwrap()).into_owned()
    }

    /// Waits until the screen, read as [`plain`], contains `needle`.
    fn wait_for(&mut self, needle: &str) {
        let deadline = Instant::now() + TIMEOUT;
        while !plain(&self.text()).contains(needle) {
            self.answer_queries();
            assert!(
                Instant::now() < deadline,
                "never showed {needle:?}:\n{}",
                self.text()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn press(&mut self, keys: &str) {
        self.writer.write_all(keys.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }

    /// Waits for the program to exit and its output to settle, and returns all of it.
    fn finish(mut self) -> String {
        let deadline = Instant::now() + TIMEOUT;
        while self.child.try_wait().unwrap().is_none() {
            self.answer_queries();
            assert!(Instant::now() < deadline, "did not exit:\n{}", self.text());
            std::thread::sleep(Duration::from_millis(50));
        }
        let mut last = self.text();
        let mut quiet_since = Instant::now();
        while quiet_since.elapsed() < SETTLE {
            std::thread::sleep(Duration::from_millis(50));
            let now = self.text();
            if now != last {
                last = now;
                quiet_since = Instant::now();
            }
        }
        last
    }
}

/// `text` without terminal escape sequences or whitespace. A terminal may draw a run of
/// spaces as a cursor move, so text is compared with neither.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' || (c == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

/// just-fetch on `root`, with its prompt up.
fn open(root: &Path) -> Terminal {
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_just-fetch"));
    command.arg(root);
    let mut terminal = Terminal::spawn(command);
    terminal.wait_for("fast-forward");
    terminal.wait_for("betabehind1");
    terminal
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

    let mut terminal = open(root);
    terminal.press(" ");
    terminal.press("\r");
    let shown = plain(&terminal.finish());

    assert!(shown.contains("alpha:updated"), "{shown}");
    assert!(pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn leaving_without_picking_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("\x1b");
    terminal.finish();

    assert!(!pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn confirming_with_nothing_picked_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("\r");
    terminal.finish();

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

    let mut terminal = open(root);
    let shown = plain(&terminal.text());
    let offered = shown[shown.find("fast-forward").expect("no prompt")..].to_string();
    terminal.press("\x1b");
    terminal.finish();

    assert!(offered.contains("alphabehind1"), "{offered}");
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

    let mut terminal = open(root);
    std::fs::write(root.join("alpha").join("README.md"), "edited meanwhile\n").unwrap();
    terminal.press(" ");
    terminal.press("\r");
    let shown = plain(&terminal.finish());

    assert!(shown.contains("alpha:skipped:nowdirty"), "{shown}");
    assert!(!pulled(root, "alpha"));
}

#[test]
fn a_refused_fast_forward_is_reported_as_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    std::fs::write(root.join("alpha").join("theirs.txt"), "my own notes\n").unwrap();
    terminal.press(" ");
    terminal.press("\r");
    let shown = plain(&terminal.finish());

    assert!(shown.contains("alpha:failed:"), "{shown}");
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

    let shown = plain(&Terminal::spawn(piped(root)).finish());

    assert!(shown.contains("alpha"), "{shown}");
    assert!(
        !shown.contains("fast-forward"),
        "prompted with output piped:\n{shown}"
    );
    assert!(!pulled(root, "alpha"));
}

/// just-fetch on `root` inside the terminal, with its output piped through another program.
#[cfg(unix)]
fn piped(root: &Path) -> CommandBuilder {
    let mut command = CommandBuilder::new("sh");
    command.arg("-c");
    command.arg(format!(
        "'{}' '{}' | cat",
        env!("CARGO_BIN_EXE_just-fetch"),
        root.display()
    ));
    command
}

#[cfg(windows)]
fn piped(root: &Path) -> CommandBuilder {
    let mut command = CommandBuilder::new("powershell");
    command.args(["-NoProfile", "-Command"]);
    command.arg(format!(
        "& '{}' '{}' | Out-Host",
        env!("CARGO_BIN_EXE_just-fetch"),
        root.display()
    ));
    command
}
