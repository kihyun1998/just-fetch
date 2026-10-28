use console::{Key, style};

use crate::fetch::{Fetch, Refreshed};
use crate::lang::Lang;
use crate::predict::Prediction;
use crate::report::one_line;
use crate::status::CloneStatus;
use crate::text::{Outlook, Text};

/// Where a clone sits on the pick screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    CanBringIn,
    NeedsYou,
    UpToDate,
}

/// One clone on the pick screen: its label, its group and what it means in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub group: Group,
    pub sentence: Text,
}

/// The group and sentence for `refreshed`. `unplanned` is why its fast-forward could not be
/// planned, for a clone that would otherwise be offered.
pub fn describe(
    refreshed: &Refreshed,
    prediction: Option<&Prediction>,
    unplanned: Option<&str>,
) -> (Group, Text) {
    use Group::*;
    let state = match &refreshed.found.state {
        Ok(state) => state,
        Err(message) => return (NeedsYou, Text::SayUnreadable(one_line(message))),
    };
    match &refreshed.fetch {
        Fetch::Failed(message) => return (NeedsYou, Text::SayFetchFailed(one_line(message))),
        Fetch::Refused(key) => return (NeedsYou, Text::SayFetchRefused(key.clone())),
        Fetch::Skipped | Fetch::Fetched => {}
    }
    match state.clone_status() {
        CloneStatus::Behind(n) if state.dirty => (NeedsYou, Text::SayDirty(n)),
        CloneStatus::Behind(n) => match unplanned {
            Some(why) => (NeedsYou, Text::SayUnplanned(n, one_line(why))),
            None => (CanBringIn, Text::SayCanBringIn(n)),
        },
        CloneStatus::UpToDate => (UpToDate, Text::SaySame),
        CloneStatus::Ahead(n) => (NeedsYou, Text::SayAhead(n)),
        CloneStatus::Diverged { ahead, behind } => {
            let outlook = match prediction {
                Some(Prediction::Clean) => Outlook::Clean,
                Some(Prediction::Conflicts(files)) => Outlook::Conflicts(files.clone()),
                Some(Prediction::Unavailable(why)) => Outlook::Unavailable(one_line(why)),
                Some(Prediction::NeedsNewerGit) => Outlook::NeedsNewerGit,
                None => Outlook::Unknown,
            };
            (NeedsYou, Text::SayDiverged(ahead, behind, outlook))
        }
        CloneStatus::NoUpstream => (NeedsYou, Text::SayNoUpstream),
        CloneStatus::GoneUpstream => (NeedsYou, Text::SayUpstreamGone),
        CloneStatus::NoCommits => (NeedsYou, Text::SayNoCommits),
    }
}

const GROUPS: [(Group, Text); 3] = [
    (Group::CanBringIn, Text::GroupCanBringIn),
    (Group::NeedsYou, Text::GroupNeedsYou),
    (Group::UpToDate, Text::GroupUpToDate),
];

/// What a key on the pick screen leads to.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Stay,
    Quit,
    /// Fast-forward these rows, by index into the rows given.
    Go(Vec<usize>),
}

/// The pick screen's state: the rows, the cursor over the rows that can be brought in, what
/// is picked, how far the list is scrolled, whether the confirmation is up, and the language.
#[derive(Debug)]
pub struct Screen {
    rows: Vec<Row>,
    pickable: Vec<usize>,
    at: usize,
    picked: Vec<bool>,
    scroll: usize,
    follow: bool,
    confirming: bool,
    lang: Lang,
}

impl Screen {
    pub fn new(rows: Vec<Row>, lang: Lang) -> Self {
        let pickable = (0..rows.len())
            .filter(|&i| rows[i].group == Group::CanBringIn)
            .collect();
        let picked = vec![false; rows.len()];
        Self {
            rows,
            pickable,
            at: 0,
            picked,
            scroll: 0,
            follow: true,
            confirming: false,
            lang,
        }
    }

    /// The language the screen is in, which `l` switches.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Whether any row can be picked at all.
    pub fn has_pickable(&self) -> bool {
        !self.pickable.is_empty()
    }

    fn chosen(&self) -> Vec<usize> {
        (0..self.rows.len()).filter(|&i| self.picked[i]).collect()
    }

    /// Acts on `key`. `page` is how many lines `f` and `b` move the list.
    pub fn key(&mut self, key: Key, page: usize) -> Step {
        if self.confirming {
            return match key {
                Key::Enter => Step::Go(self.chosen()),
                Key::CtrlC => Step::Quit,
                _ => {
                    self.confirming = false;
                    Step::Stay
                }
            };
        }
        match key {
            Key::ArrowUp | Key::Char('k') => {
                self.at = self.at.saturating_sub(1);
                self.follow = true;
            }
            Key::ArrowDown | Key::Char('j') => {
                self.at = (self.at + 1).min(self.pickable.len().saturating_sub(1));
                self.follow = true;
            }
            Key::Char(' ') => {
                if let Some(&i) = self.pickable.get(self.at) {
                    self.picked[i] = !self.picked[i];
                }
                self.follow = true;
            }
            Key::Char('a') => {
                let all = self.all_picked();
                for &i in &self.pickable {
                    self.picked[i] = !all;
                }
                self.follow = true;
            }
            Key::Char('f') => {
                self.scroll += page;
                self.follow = false;
            }
            Key::Char('b') => {
                self.scroll = self.scroll.saturating_sub(page);
                self.follow = false;
            }
            Key::Enter if self.picked.contains(&true) => self.confirming = true,
            Key::Char('l') => self.lang = self.lang.other(),
            Key::Char('q') | Key::Escape | Key::CtrlC => return Step::Quit,
            _ => {}
        }
        Step::Stay
    }

    fn all_picked(&self) -> bool {
        self.pickable.iter().all(|&i| self.picked[i])
    }

    /// The groups and their rows, with the index of the line the cursor is on.
    fn body(&self) -> (Vec<String>, Option<usize>) {
        let width = self
            .rows
            .iter()
            .map(|row| console::measure_text_width(&row.label))
            .max()
            .unwrap_or(0);
        let cursor_row = self.pickable.get(self.at).copied();
        let say = |text: &Text| text.in_lang(self.lang);
        let mut lines = vec![style(say(&Text::Title)).bold().to_string(), String::new()];
        let mut cursor_line = None;
        for (group, title) in GROUPS {
            let members: Vec<usize> = (0..self.rows.len())
                .filter(|&i| self.rows[i].group == group)
                .collect();
            lines.push(
                style(format!("{} ({})", say(&title), members.len()))
                    .underlined()
                    .to_string(),
            );
            for i in members {
                let row = &self.rows[i];
                let label = console::pad_str(&row.label, width, console::Alignment::Left, None);
                if group == Group::CanBringIn {
                    let mark = if self.picked[i] { "[x]" } else { "[ ]" };
                    let line = format!("{mark} {label}  {}", say(&row.sentence));
                    if cursor_row == Some(i) {
                        cursor_line = Some(lines.len());
                        lines.push(format!("> {line}"));
                    } else {
                        lines.push(format!("  {line}"));
                    }
                } else {
                    lines.push(
                        style(format!("      {label}  {}", say(&row.sentence)))
                            .dim()
                            .to_string(),
                    );
                }
            }
            lines.push(String::new());
        }
        (lines, cursor_line)
    }

    /// The screen as `height` lines at most, each cut to `width` columns.
    pub fn lines(&mut self, height: usize, width: usize) -> Vec<String> {
        let lines = if self.confirming {
            self.confirmation(height)
        } else {
            self.list(height)
        };
        lines
            .into_iter()
            .take(height.max(1))
            .map(|line| {
                console::truncate_str(&line, width.saturating_sub(1).max(1), "…").into_owned()
            })
            .collect()
    }

    fn list(&mut self, height: usize) -> Vec<String> {
        let (body, cursor_line) = self.body();
        let view = height.saturating_sub(3).max(1);
        if self.follow {
            if let Some(line) = cursor_line {
                if line < self.scroll {
                    self.scroll = line;
                } else if line >= self.scroll + view {
                    self.scroll = line + 1 - view;
                }
            }
        }
        self.scroll = self.scroll.min(body.len().saturating_sub(view));
        let above = self.scroll;
        let below = body.len().saturating_sub(self.scroll + view);
        let mut lines: Vec<String> = body.into_iter().skip(self.scroll).take(view).collect();
        let say = |text: Text| text.in_lang(self.lang);
        let mut marks = Vec::new();
        if above > 0 {
            marks.push(say(Text::MoreAbove(above)));
        }
        if below > 0 {
            marks.push(say(Text::MoreBelow(below)));
        }
        lines.push(style(marks.join("   ")).dim().to_string());
        let keys = Text::Keys {
            all_picked: self.all_picked(),
        };
        lines.push(style(say(keys)).dim().to_string());
        lines.push(say(Text::Picked(
            self.picked.iter().filter(|&&p| p).count(),
        )));
        lines
    }

    fn confirmation(&self, height: usize) -> Vec<String> {
        let chosen = self.chosen();
        let room = height.saturating_sub(4).max(1);
        let say = |text: &Text| text.in_lang(self.lang);
        let mut lines = vec![
            style(say(&Text::BringThese(chosen.len())))
                .bold()
                .to_string(),
        ];
        let shown = if chosen.len() > room {
            room - 1
        } else {
            chosen.len()
        };
        for &i in &chosen[..shown] {
            lines.push(format!(
                "  {}  {}",
                self.rows[i].label,
                say(&self.rows[i].sentence)
            ));
        }
        if shown < chosen.len() {
            lines.push(say(&Text::AndMore(chosen.len() - shown)));
        }
        lines.push(String::new());
        lines.push(style(say(&Text::GoOrBack)).dim().to_string());
        lines
    }

    /// The groups and their rows with nothing to pick, for a run where nothing can be brought in.
    pub fn summary(&self) -> Vec<String> {
        let (body, _) = self.body();
        body.into_iter().skip(2).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::scan::FoundClone;
    use crate::status;

    use super::*;

    fn row(label: &str, group: Group) -> Row {
        Row {
            label: label.to_string(),
            group,
            sentence: Text::SayUnreadable(label.to_string()),
        }
    }

    /// Two that can be brought in, with one that cannot between them.
    fn screen() -> Screen {
        Screen::new(
            vec![
                row("alpha", Group::CanBringIn),
                row("beta", Group::NeedsYou),
                row("gamma", Group::CanBringIn),
                row("delta", Group::UpToDate),
            ],
            Lang::En,
        )
    }

    fn text(screen: &mut Screen, height: usize) -> Vec<String> {
        screen
            .lines(height, 200)
            .iter()
            .map(|line| console::strip_ansi_codes(line).trim_end().to_string())
            .collect()
    }

    #[test]
    fn the_groups_come_in_order_with_their_counts() {
        let shown = text(&mut screen(), 40);
        let at = |needle: &str| shown.iter().position(|l| l.starts_with(needle)).unwrap();
        assert!(at("Can bring in (2)") < at("Needs you first"));
        assert!(at("Needs you first (can't be picked here) (1)") < at("Already up to date (1)"));
    }

    #[test]
    fn the_cursor_moves_only_over_rows_that_can_be_picked() {
        let mut screen = screen();
        screen.key(Key::ArrowDown, 10);
        screen.key(Key::Char(' '), 10);
        screen.key(Key::Char('j'), 10);
        screen.key(Key::Enter, 10);
        assert_eq!(screen.key(Key::Enter, 10), Step::Go(vec![2]));
    }

    #[test]
    fn the_cursor_stops_at_either_end() {
        let mut screen = screen();
        screen.key(Key::ArrowUp, 10);
        screen.key(Key::Char(' '), 10);
        for _ in 0..5 {
            screen.key(Key::Char('k'), 10);
        }
        screen.key(Key::Enter, 10);
        assert_eq!(screen.key(Key::Enter, 10), Step::Go(vec![0]));
    }

    #[test]
    fn a_picks_every_row_that_can_be_picked_and_again_none() {
        let mut screen = screen();
        screen.key(Key::Char('a'), 10);
        assert!(
            text(&mut screen, 40)
                .iter()
                .any(|l| l.contains("a unpick all"))
        );
        screen.key(Key::Enter, 10);
        assert_eq!(screen.key(Key::Enter, 10), Step::Go(vec![0, 2]));

        let mut screen = self::screen();
        screen.key(Key::Char('a'), 10);
        screen.key(Key::Char('a'), 10);
        assert!(
            text(&mut screen, 40)
                .iter()
                .any(|l| l.contains("a pick all"))
        );
        assert!(text(&mut screen, 40).contains(&"0 picked".to_string()));
    }

    #[test]
    fn a_after_picking_some_picks_the_rest() {
        let mut screen = screen();
        screen.key(Key::Char(' '), 10);
        screen.key(Key::Char('a'), 10);
        screen.key(Key::Enter, 10);
        assert_eq!(screen.key(Key::Enter, 10), Step::Go(vec![0, 2]));
    }

    #[test]
    fn enter_with_nothing_picked_stays_on_the_list() {
        let mut screen = screen();
        assert_eq!(screen.key(Key::Enter, 10), Step::Stay);
        assert!(text(&mut screen, 40)[0].starts_with("Pick the clones"));
    }

    #[test]
    fn the_confirmation_lists_what_was_picked_and_another_key_goes_back() {
        let mut screen = screen();
        screen.key(Key::Char('a'), 10);
        screen.key(Key::Enter, 10);
        let shown = text(&mut screen, 40);
        assert_eq!(shown[0], "Bring these 2 clones up to date:");
        assert_eq!(shown[1], "  alpha  git couldn't read it: alpha");
        assert_eq!(shown[2], "  gamma  git couldn't read it: gamma");

        assert_eq!(screen.key(Key::Char('x'), 10), Step::Stay);
        assert!(text(&mut screen, 40)[0].starts_with("Pick the clones"));
    }

    #[test]
    fn l_switches_the_language_and_back() {
        let mut screen = screen();
        screen.key(Key::Char('l'), 10);
        assert_eq!(screen.lang(), Lang::Ko);
        assert!(text(&mut screen, 40)[0].starts_with("원격에 새 커밋이 있는"));
        screen.key(Key::Char('l'), 10);
        assert_eq!(screen.lang(), Lang::En);
        assert!(text(&mut screen, 40)[0].starts_with("Pick the clones"));
    }

    #[test]
    fn q_esc_and_ctrl_c_leave() {
        for key in [Key::Char('q'), Key::Escape, Key::CtrlC] {
            assert_eq!(screen().key(key, 10), Step::Quit);
        }
        let mut confirming = screen();
        confirming.key(Key::Char(' '), 10);
        confirming.key(Key::Enter, 10);
        assert_eq!(confirming.key(Key::CtrlC, 10), Step::Quit);
    }

    /// One that can be brought in at the top, and `n` that cannot.
    fn tall(n: usize) -> Screen {
        let mut rows = vec![row("first", Group::CanBringIn)];
        rows.extend((0..n).map(|i| row(&format!("other{i}"), Group::UpToDate)));
        rows.push(row("last", Group::CanBringIn));
        Screen::new(rows, Lang::En)
    }

    #[test]
    fn a_list_taller_than_the_screen_fits_it_and_says_how_much_is_below() {
        let mut screen = tall(50);
        let shown = text(&mut screen, 20);
        assert_eq!(shown.len(), 20);
        assert!(shown[17].contains("below (f)"), "{shown:#?}");
        assert!(!shown.iter().any(|l| l.contains("other49")));
    }

    #[test]
    fn f_shows_the_rest_and_b_goes_back() {
        let mut screen = tall(50);
        for _ in 0..5 {
            screen.key(Key::Char('f'), 17);
        }
        let shown = text(&mut screen, 20);
        assert!(shown.iter().any(|l| l.contains("other49")), "{shown:#?}");
        assert!(shown[17].contains("above (b)"));
        assert!(!shown[17].contains("below"));

        for _ in 0..5 {
            screen.key(Key::Char('b'), 17);
        }
        assert!(text(&mut screen, 20)[0].starts_with("Pick the clones"));
    }

    #[test]
    fn the_list_follows_the_cursor() {
        let mut screen = Screen::new(
            (0..40)
                .map(|i| row(&format!("clone{i}"), Group::CanBringIn))
                .collect(),
            Lang::En,
        );
        for _ in 0..39 {
            screen.key(Key::ArrowDown, 17);
        }
        let shown = text(&mut screen, 20);
        assert!(
            shown.iter().any(|l| l.starts_with("> [ ] clone39")),
            "{shown:#?}"
        );
    }

    #[test]
    fn picking_after_paging_brings_the_cursor_back_into_view() {
        let mut screen = tall(50);
        screen.key(Key::Char('f'), 17);
        screen.key(Key::Char('f'), 17);
        screen.key(Key::Char(' '), 17);
        let shown = text(&mut screen, 20);
        assert!(
            shown.iter().any(|l| l.starts_with("> [x] first")),
            "{shown:#?}"
        );
    }

    #[test]
    fn a_tiny_terminal_gets_no_more_lines_than_it_has_and_no_panic() {
        for height in 0..4 {
            assert!(screen().lines(height, 1).len() <= height.max(1));
        }
        let mut confirming = screen();
        confirming.key(Key::Char('a'), 1);
        confirming.key(Key::Enter, 1);
        assert_eq!(confirming.lines(2, 1).len(), 2);
    }

    #[test]
    fn lines_are_cut_to_the_width() {
        let mut screen = screen();
        assert!(
            screen
                .lines(40, 30)
                .iter()
                .all(|line| console::measure_text_width(line) < 30)
        );
    }

    fn refreshed(porcelain: &str, fetch: Fetch) -> Refreshed {
        Refreshed {
            found: FoundClone {
                name: "repo".into(),
                path: PathBuf::from("repo"),
                state: Ok(status::parse(porcelain)),
            },
            fetch,
        }
    }

    fn tracking(ab: &str, dirty: bool) -> String {
        let change = if dirty {
            "1 .M N... 100644 100644 100644 e69de29 e69de29 README.md\n"
        } else {
            ""
        };
        format!(
            "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n# branch.head main\n\
             # branch.upstream origin/main\n# branch.ab {ab}\n{change}"
        )
    }

    #[test]
    fn a_behind_clone_can_be_brought_in_unless_dirty_or_unplanned() {
        let behind = refreshed(&tracking("+0 -1", false), Fetch::Fetched);
        assert_eq!(
            describe(&behind, None, None),
            (Group::CanBringIn, Text::SayCanBringIn(1))
        );
        assert_eq!(
            describe(&behind, None, Some("bad\nref")),
            (Group::NeedsYou, Text::SayUnplanned(1, "bad; ref".into()))
        );

        let dirty = refreshed(&tracking("+0 -2", true), Fetch::Fetched);
        assert_eq!(
            describe(&dirty, None, None),
            (Group::NeedsYou, Text::SayDirty(2))
        );
    }

    #[test]
    fn a_diverged_clone_carries_its_prediction() {
        let diverged = refreshed(&tracking("+1 -2", false), Fetch::Fetched);
        let conflicts = Prediction::Conflicts(vec!["todo.md".into()]);
        assert_eq!(
            describe(&diverged, Some(&conflicts), None),
            (
                Group::NeedsYou,
                Text::SayDiverged(1, 2, Outlook::Conflicts(vec!["todo.md".into()]))
            )
        );
        assert_eq!(
            describe(&diverged, Some(&Prediction::NeedsNewerGit), None).1,
            Text::SayDiverged(1, 2, Outlook::NeedsNewerGit)
        );
        assert_eq!(
            describe(&diverged, None, None).1,
            Text::SayDiverged(1, 2, Outlook::Unknown)
        );
    }

    #[test]
    fn the_rest_need_you_or_are_up_to_date() {
        let up_to_date = refreshed(&tracking("+0 -0", false), Fetch::Fetched);
        assert_eq!(
            describe(&up_to_date, None, None),
            (Group::UpToDate, Text::SaySame)
        );

        let ahead = refreshed(&tracking("+1 -0", false), Fetch::Fetched);
        assert_eq!(
            describe(&ahead, None, None),
            (Group::NeedsYou, Text::SayAhead(1))
        );

        let failed = refreshed(&tracking("+0 -1", false), Fetch::Failed("no\nroute".into()));
        assert_eq!(
            describe(&failed, None, None),
            (Group::NeedsYou, Text::SayFetchFailed("no; route".into()))
        );

        let refused = refreshed(
            &tracking("+0 -1", false),
            Fetch::Refused("core.gitProxy".into()),
        );
        assert_eq!(
            describe(&refused, None, None),
            (
                Group::NeedsYou,
                Text::SayFetchRefused("core.gitProxy".into())
            )
        );

        let unreadable = Refreshed {
            found: FoundClone {
                name: "broken".into(),
                path: PathBuf::from("broken"),
                state: Err("fatal: not a git repository".into()),
            },
            fetch: Fetch::Skipped,
        };
        assert_eq!(describe(&unreadable, None, None).0, Group::NeedsYou);
    }
}
