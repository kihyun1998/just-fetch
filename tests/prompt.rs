//! The selection prompt, driven through a pseudo-terminal the way a person would use it:
//! a Unix pty, or ConPTY on Windows.

mod common;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{clone_with_upstream, push_file_from_elsewhere};
use just_fetch::config::{CONFIG_DIR_VAR, Config, load, save};
use just_fetch::lang::SYSTEM_LANGUAGE_VAR;
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
        if command.get_env(SYSTEM_LANGUAGE_VAR).is_none() {
            command.env(SYSTEM_LANGUAGE_VAR, "en");
        }
        if command.get_env(CONFIG_DIR_VAR).is_none() {
            command.env(
                CONFIG_DIR_VAR,
                std::env::temp_dir().join("just-fetch-tests-no-config"),
            );
        }
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
        self.wait_for_nth(needle, 1);
    }

    /// Waits until the screen, read as [`plain`], contains `needle` `n` times.
    fn wait_for_nth(&mut self, needle: &str, n: usize) {
        let deadline = Instant::now() + TIMEOUT;
        while plain(&self.text()).matches(needle).count() < n {
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
    fn finish(self) -> String {
        self.finish_with_status().0
    }

    /// [`finish`](Self::finish), asserting the program exited successfully.
    fn finish_ok(self) -> String {
        let (shown, ok) = self.finish_with_status();
        assert!(ok, "exited with a failure:\n{shown}");
        shown
    }

    /// The output, and whether the program exited successfully.
    fn finish_with_status(mut self) -> (String, bool) {
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            self.answer_queries();
            assert!(Instant::now() < deadline, "did not exit:\n{}", self.text());
            std::thread::sleep(Duration::from_millis(50));
        };
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
        (last, status.success())
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

/// just-fetch on `root`, with its pick screen up.
fn open(root: &Path) -> Terminal {
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_just-fetch"));
    command.arg(root);
    let mut terminal = Terminal::spawn(command);
    terminal.wait_for("Canbringin(2)");
    terminal.wait_for("0picked");
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

/// Enter on the list, then enter again on the confirmation once it is up.
fn confirm(terminal: &mut Terminal) {
    terminal.press("\r");
    terminal.wait_for("uptodate:");
    terminal.press("\r");
}

#[test]
fn picking_one_clone_fast_forwards_only_that_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press(" ");
    confirm(&mut terminal);
    let shown = plain(&terminal.finish_ok());

    assert!(shown.contains("Bringthese1cloneuptodate:"), "{shown}");
    assert!(shown.contains("alpha:updated"), "{shown}");
    assert!(pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn at_a_terminal_the_screen_stands_in_for_the_table() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("q");
    let shown = plain(&terminal.finish_ok());

    assert!(
        shown.contains("alpha1newcommitupstream.Readytobringin"),
        "{shown}"
    );
    assert!(
        !shown.contains("->origin/main"),
        "the table was printed:\n{shown}"
    );
}

#[test]
fn leaving_with_esc_or_q_changes_nothing() {
    for key in ["\x1b", "q"] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        two_behind(root);

        let mut terminal = open(root);
        terminal.press(" ");
        terminal.wait_for("1picked");
        terminal.press(key);
        terminal.finish_ok();

        assert!(!pulled(root, "alpha"), "after {key:?}");
    }
}

#[test]
fn enter_with_nothing_picked_stays_on_the_list() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("\r");
    terminal.press("q");
    let shown = plain(&terminal.finish_ok());

    assert!(!shown.contains("Bringthese"), "{shown}");
    assert!(!pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn the_three_groups_show_and_the_cursor_skips_rows_that_cannot_be_picked() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);
    let dirty = clone_with_upstream(root, "aaa-dirty");
    push_file_from_elsewhere(root, "aaa-dirty", "theirs.txt", "theirs\n");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();
    clone_with_upstream(root, "zzz-current");

    let mut terminal = open(root);
    terminal.wait_for("Needsyoufirst(can'tbepickedhere)(1)");
    terminal.wait_for("Alreadyuptodate(1)");
    for _ in 0..5 {
        terminal.press("\x1b[B");
    }
    terminal.press(" ");
    confirm(&mut terminal);
    let shown = plain(&terminal.finish_ok());

    assert!(
        shown.contains("aaa-dirty1newcommitupstream,butyouhaveunsavededits"),
        "{shown}"
    );
    assert!(shown.contains("beta:updated"), "{shown}");
    assert!(!pulled(root, "alpha"));
    assert!(!dirty.join("theirs.txt").exists());
}

#[test]
fn a_picks_every_clone_that_can_be_brought_in() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("a");
    terminal.wait_for("aunpickall");
    confirm(&mut terminal);
    terminal.finish_ok();

    assert!(pulled(root, "alpha"));
    assert!(pulled(root, "beta"));
}

#[test]
fn a_twice_picks_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("a");
    terminal.wait_for("2picked");
    terminal.press("a");
    terminal.wait_for_nth("0picked", 2);
    terminal.press("\r");
    terminal.press("q");
    terminal.finish_ok();

    assert!(!pulled(root, "alpha"));
    assert!(!pulled(root, "beta"));
}

#[test]
fn another_key_at_the_confirmation_goes_back_and_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press(" ");
    terminal.press("\r");
    terminal.wait_for("Bringthese1cloneuptodate:");
    terminal.press("x");
    terminal.wait_for_nth("Picktheclones", 2);
    terminal.press("q");
    terminal.finish_ok();

    assert!(!pulled(root, "alpha"));
}

#[test]
fn ctrl_c_leaves_and_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press(" ");
    terminal.wait_for("1picked");
    terminal.press("\x03");
    terminal.finish_ok();

    assert!(!pulled(root, "alpha"));
}

#[test]
fn a_clone_edited_while_the_screen_is_open_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    std::fs::write(root.join("alpha").join("README.md"), "edited meanwhile\n").unwrap();
    terminal.press(" ");
    confirm(&mut terminal);
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
    confirm(&mut terminal);
    let shown = plain(&terminal.finish());

    assert!(shown.contains("alpha:failed:"), "{shown}");
    assert_eq!(
        std::fs::read_to_string(root.join("alpha").join("theirs.txt")).unwrap(),
        "my own notes\n"
    );
}

#[test]
fn with_nothing_to_bring_in_the_groups_are_printed_and_nothing_is_asked() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    clone_with_upstream(root, "current");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_just-fetch"));
    command.arg(root);
    let shown = plain(&Terminal::spawn(command).finish_ok());

    assert!(shown.contains("Canbringin(0)"), "{shown}");
    assert!(
        shown.contains("currentSameastheremote.Nothingtodo"),
        "{shown}"
    );
    assert!(!shown.contains("0picked"), "{shown}");
}

#[test]
fn clones_sharing_a_name_are_told_apart_by_their_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let (work, personal) = (tmp.path().join("work"), tmp.path().join("personal"));
    for folder in [&work, &personal] {
        std::fs::create_dir(folder).unwrap();
        clone_with_upstream(folder, "api");
        push_file_from_elsewhere(folder, "api", "theirs.txt", "theirs\n");
    }
    clone_with_upstream(&work, "solo");
    let tracked = Config {
        language: None,
        folders: vec![work.clone(), personal.clone()],
    };
    save(&config.join("config.toml"), &tracked).unwrap();

    let mut terminal = with_config(&config, &[]);
    terminal.wait_for("Canbringin(2)");
    terminal.press("q");
    let shown = plain(&terminal.finish_ok());

    assert!(shown.contains("work/api1newcommit"), "{shown}");
    assert!(shown.contains("personal/api1newcommit"), "{shown}");
    assert!(shown.contains("soloSameastheremote"), "{shown}");
    assert!(!shown.contains("work/solo"), "{shown}");
}

#[test]
fn a_list_taller_than_the_terminal_scrolls_with_f_and_b() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    clone_with_upstream(root, "aaa");
    push_file_from_elsewhere(root, "aaa", "theirs.txt", "theirs\n");
    for i in 0..45 {
        common::empty_clone(root, &format!("n{i:02}"));
    }

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_just-fetch"));
    command.arg(root);
    let mut terminal = Terminal::spawn(command);
    terminal.wait_for("below(f)");
    let before = plain(&terminal.text());
    terminal.press("f");
    terminal.wait_for("n44Noremotebranch");
    terminal.wait_for("above(b)");
    terminal.press("b");
    terminal.wait_for_nth("Picktheclones", 2);
    terminal.press("q");
    terminal.finish_ok();

    assert!(!before.contains("n44"), "{before}");
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

/// just-fetch with `args`, keeping its config under `config_dir`.
fn with_config(config_dir: &Path, args: &[&str]) -> Terminal {
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_just-fetch"));
    command.args(args);
    command.env(CONFIG_DIR_VAR, config_dir);
    Terminal::spawn(command)
}

/// Keeps the language setup offers first, the one already in use.
fn answer_language(terminal: &mut Terminal) {
    terminal.wait_for("Language");
    terminal.press("\r");
}

/// The tracked folders saved under `config_dir`.
fn tracked_in(config_dir: &Path) -> Vec<std::path::PathBuf> {
    load(&config_dir.join("config.toml")).unwrap().folders
}

#[test]
fn setup_saves_each_folder_typed_until_an_empty_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let (code, notes) = (tmp.path().join("code"), tmp.path().join("notes"));
    for folder in [&code, &notes] {
        std::fs::create_dir(folder).unwrap();
    }
    clone_with_upstream(&code, "alpha");

    let mut terminal = with_config(&config, &["setup"]);
    answer_language(&mut terminal);
    for (n, folder) in [&code, &notes].into_iter().enumerate() {
        terminal.wait_for_nth("Foldertotrack", n + 1);
        terminal.press(&format!("{}\r", folder.display()));
        terminal.wait_for(&format!("tracking{}", plain(&folder.display().to_string())));
    }
    terminal.press("\r");
    let shown = plain(&terminal.finish());

    assert!(shown.contains("(1clone)"), "{shown}");
    assert!(shown.contains("savedto"), "{shown}");
    assert_eq!(tracked_in(&config), [code, notes]);
}

#[test]
fn with_nothing_tracked_a_run_at_a_terminal_starts_setup_and_then_covers_it() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let code = tmp.path().join("code");
    std::fs::create_dir(&code).unwrap();
    two_behind(&code);

    let mut terminal = with_config(&config, &[]);
    answer_language(&mut terminal);
    terminal.wait_for("Foldertotrack");
    terminal.press(&format!("{}\r", code.display()));
    terminal.wait_for("tracking");
    terminal.press("\r");
    terminal.wait_for("Canbringin(2)");
    terminal.press("\x1b");
    terminal.finish();

    assert_eq!(tracked_in(&config), std::slice::from_ref(&code));
    assert!(!pulled(&code, "alpha"));
}

#[test]
fn a_folder_added_before_setup_is_interrupted_stays_tracked() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let code = tmp.path().join("code");
    std::fs::create_dir(&code).unwrap();

    let mut terminal = with_config(&config, &["setup"]);
    answer_language(&mut terminal);
    terminal.wait_for("Foldertotrack");
    terminal.press(&format!("{}\r", code.display()));
    terminal.wait_for_nth("Foldertotrack", 2);
    terminal.press("\x03");
    terminal.finish();

    assert_eq!(tracked_in(&config), std::slice::from_ref(&code));
}

#[test]
fn a_folder_pasted_in_quotes_is_tracked() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let code = tmp.path().join("code");
    std::fs::create_dir(&code).unwrap();

    let mut terminal = with_config(&config, &["setup"]);
    answer_language(&mut terminal);
    terminal.wait_for("Foldertotrack");
    terminal.press(&format!("\"{}\"\r", code.display()));
    terminal.wait_for_nth("Foldertotrack", 2);
    terminal.press("\r");
    terminal.finish();

    assert_eq!(tracked_in(&config), std::slice::from_ref(&code));
}

#[test]
fn a_run_whose_setup_adds_nothing_says_nothing_is_tracked() {
    let tmp = tempfile::tempdir().unwrap();

    let mut terminal = with_config(&tmp.path().join("config"), &[]);
    answer_language(&mut terminal);
    terminal.wait_for("Foldertotrack");
    terminal.press("\r");
    let shown = plain(&terminal.finish());

    assert!(shown.contains("just-fetchsetup"), "{shown}");
}

/// `just-fetch remove` with `folders` tracked under `config_dir`, its list up.
fn removing(config_dir: &Path, folders: &[&Path]) -> Terminal {
    let tracked = Config {
        language: None,
        folders: folders.iter().map(|f| f.to_path_buf()).collect(),
    };
    save(&config_dir.join("config.toml"), &tracked).unwrap();
    let mut terminal = with_config(config_dir, &["remove"]);
    terminal.wait_for("stoptracking");
    terminal
}

/// Two folders to track, the second of them gone.
fn code_and_gone(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let code = root.join("code");
    std::fs::create_dir(&code).unwrap();
    (code, root.join("gone"))
}

#[test]
fn removing_the_folder_picked_keeps_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let (code, gone) = code_and_gone(tmp.path());

    let mut terminal = removing(&config, &[&code, &gone]);
    terminal.press("\x1b[B");
    terminal.press(" ");
    terminal.press("\r");
    terminal.wait_for("Stoptracking1folder?[y/n]");
    terminal.press("y");
    terminal.finish_ok();

    assert_eq!(tracked_in(&config), std::slice::from_ref(&code));
}

#[test]
fn leaving_remove_without_picking_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let (code, gone) = code_and_gone(tmp.path());

    let mut terminal = removing(&config, &[&code, &gone]);
    terminal.press("\x1b");
    terminal.finish_ok();

    assert_eq!(tracked_in(&config), [code, gone]);
}

#[test]
fn confirming_remove_with_nothing_picked_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let (code, gone) = code_and_gone(tmp.path());

    let mut terminal = removing(&config, &[&code, &gone]);
    terminal.press("\r");
    terminal.finish_ok();

    assert_eq!(tracked_in(&config), [code, gone]);
}

#[test]
fn answering_no_to_remove_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    let (code, gone) = code_and_gone(tmp.path());

    let mut terminal = removing(&config, &[&code, &gone]);
    terminal.press(" ");
    terminal.press("\r");
    terminal.wait_for("[y/n]");
    terminal.press("n");
    terminal.finish_ok();

    assert_eq!(tracked_in(&config), [code, gone]);
}

#[test]
fn l_switches_the_screen_and_what_follows_it_to_the_other_language() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    two_behind(root);

    let mut terminal = open(root);
    terminal.press("l");
    terminal.wait_for("원격에새커밋이있는저장소를골라");
    terminal.wait_for("받아올수있음(2)");
    terminal.press(" ");
    terminal.press("\r");
    terminal.wait_for("이1개를원격최신으로받아옵니다:");
    terminal.press("\r");
    let shown = plain(&terminal.finish_ok());

    assert!(shown.contains("alpha:받아옴"), "{shown}");
    assert!(pulled(root, "alpha"));
}

#[test]
fn setup_saves_the_language_picked() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");

    let mut terminal = with_config(&config, &["setup"]);
    terminal.wait_for("Language");
    terminal.press("\x1b[B");
    terminal.press("\r");
    terminal.wait_for("추적할폴더");
    terminal.press("\r");
    let shown = plain(&terminal.finish_ok());

    assert!(shown.contains("저장함:"), "{shown}");
    assert_eq!(
        load(&config.join("config.toml")).unwrap().language,
        Some(just_fetch::lang::Lang::Ko)
    );
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
