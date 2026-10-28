use std::fmt;

use crate::fast_forward::Change;
use crate::lang::{self, Lang};
use crate::predict::MINIMUM_GIT;

/// What a diverged clone's trial merge says, for the pick screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outlook {
    Clean,
    Conflicts(Vec<String>),
    /// git's reason.
    Unavailable(String),
    NeedsNewerGit,
    /// No prediction was made.
    Unknown,
}

/// Everything just-fetch says on its own, in either language. Paths, names and git's own
/// messages ride along as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    // The table.
    RowError(String),
    NoCommitsHead(String),
    Detached,
    FetchFailed(String),
    FetchRefused(String),
    NoUpstream,
    UpstreamGone,
    UpToDate,
    Behind(u32),
    Ahead(u32),
    Diverged(u32, u32),
    MergesCleanly,
    Conflict(Vec<String>),
    ConflictCheckUnavailable(String),
    ConflictCheckNeedsGit,
    Dirty,
    Untracked(usize),

    // The pick screen.
    Title,
    GroupCanBringIn,
    GroupNeedsYou,
    GroupUpToDate,
    SayUnreadable(String),
    SayFetchFailed(String),
    SayFetchRefused(String),
    SayDirty(u32),
    SayUnplanned(u32, String),
    SayCanBringIn(u32),
    SaySame,
    SayAhead(u32),
    SayDiverged(u32, u32, Outlook),
    SayNoUpstream,
    SayUpstreamGone,
    SayNoCommits,
    MoreAbove(usize),
    MoreBelow(usize),
    Keys { all_picked: bool },
    Picked(usize),
    BringThese(usize),
    AndMore(usize),
    GoOrBack,

    // After the pick screen.
    Updated(String),
    Skipped(String, Change),
    Failed(String, String),

    // setup, list and remove.
    LanguagePrompt,
    FolderToTrack,
    Tracking(String, usize),
    AlreadyTracked(String),
    SavedTo(String),
    SetupNeedsTerminal,
    ListClones(String, usize),
    ListMissing(String, String),
    ConfigAt(String),
    NothingTracked,
    NothingToRemove,
    RemoveNeedsTerminal,
    MissingMark,
    StopTrackingPrompt,
    StopTracking(usize),

    // Errors.
    CannotRead(String, String),
    CannotWrite(String, String),
    CannotTrack(String, String),
    NotAFolder,
    GitMissing,
    NoOwnPath,
    NoConfigFolder,
    NoneRead,
    ScreenFailed(String),
    CannotDraw,
    UnexpectedGitOutput(String),
    UpgradingWithBrew(String),
    BrewMissing(String),
    BrewFailed(String),
    UpgradeWithScoop,
    UpgradeByHand(String),
}

/// `n` followed by `noun`, adding an `s` unless `n` is 1.
fn count(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        _ => format!("{n} {noun}s"),
    }
}

/// How many conflicted files are named before counting the rest.
const CONFLICTS_SHOWN: usize = 5;

/// The first conflicted files, each kept on one line, then how many more there are.
fn conflict_list(files: &[String], lang: Lang) -> String {
    let shown = files[..files.len().min(CONFLICTS_SHOWN)]
        .iter()
        .map(|name| name.replace('\n', "\\n"))
        .collect::<Vec<_>>()
        .join(", ");
    match (files.len().saturating_sub(CONFLICTS_SHOWN), lang) {
        (0, _) => shown,
        (more, Lang::En) => format!("{shown} (+{more} more)"),
        (more, Lang::Ko) => format!("{shown} (외 {more}개)"),
    }
}

impl Text {
    /// The text in `lang`.
    pub fn in_lang(&self, lang: Lang) -> String {
        let (en, ko) = self.both();
        match lang {
            Lang::En => en,
            Lang::Ko => ko,
        }
    }

    /// The English and the Korean, side by side.
    fn both(&self) -> (String, String) {
        use Text::*;
        let git = format!("git {}.{}", MINIMUM_GIT.major, MINIMUM_GIT.minor);
        let s = |en: &str, ko: &str| (en.to_string(), ko.to_string());
        match self {
            RowError(m) => (format!("error: {m}"), format!("오류: {m}")),
            NoCommitsHead(name) => (
                format!("{name} (no commits)"),
                format!("{name} (커밋 없음)"),
            ),
            Detached => s("(detached)", "(분리된 HEAD)"),
            FetchFailed(m) => (format!("fetch failed: {m}"), format!("가져오기 실패: {m}")),
            FetchRefused(key) => (
                format!(
                    "not fetched: this clone's own config sets {key}, which would run a command"
                ),
                format!(
                    "가져오지 않음: 이 저장소의 설정이 {key}를 정해 두어 명령이 실행될 수 있음"
                ),
            ),
            NoUpstream => s("no upstream", "추적 브랜치 없음"),
            UpstreamGone => s("upstream gone", "추적 브랜치 사라짐"),
            UpToDate => s("up to date", "최신"),
            Behind(n) => (format!("behind {n}"), format!("{n}개 뒤처짐")),
            Ahead(n) => (format!("ahead {n}"), format!("{n}개 앞섬")),
            Diverged(a, b) => (format!("diverged +{a} -{b}"), format!("갈라짐 +{a} -{b}")),
            MergesCleanly => s("merges cleanly", "충돌 없이 합쳐짐"),
            Conflict(files) => (
                format!("conflict: {}", conflict_list(files, Lang::En)),
                format!("충돌: {}", conflict_list(files, Lang::Ko)),
            ),
            ConflictCheckUnavailable(why) => (
                format!("conflict check unavailable: {why}"),
                format!("충돌 확인 불가: {why}"),
            ),
            ConflictCheckNeedsGit => (
                format!("conflict check unavailable: needs {git} or later"),
                format!("충돌 확인 불가: {git} 이상 필요"),
            ),
            Dirty => s("dirty", "수정됨"),
            Untracked(n) => (format!("{n} untracked"), format!("새 파일 {n}개")),

            Title => s(
                "Pick the clones you want to bring up to date with their remote. Your unsaved edits are never touched.",
                "원격에 새 커밋이 있는 저장소를 골라 최신으로 받아와요. 저장 안 한 수정은 건드리지 않아요.",
            ),
            GroupCanBringIn => s("Can bring in", "받아올 수 있음"),
            GroupNeedsYou => s(
                "Needs you first (can't be picked here)",
                "직접 처리해야 함 (여기서는 못 고름)",
            ),
            GroupUpToDate => s("Already up to date", "이미 최신"),
            SayUnreadable(m) => (
                format!("git couldn't read it: {m}"),
                format!("git이 읽지 못함: {m}"),
            ),
            SayFetchFailed(m) => (
                format!("Couldn't reach the remote: {m}"),
                format!("원격에 접속 실패: {m}"),
            ),
            SayFetchRefused(key) => (
                format!("Not fetched: its own config sets {key}, which would run a command"),
                format!("가져오지 않음: 이 저장소 설정의 {key}가 명령을 실행할 수 있어요"),
            ),
            SayDirty(n) => (
                format!(
                    "{} upstream, but you have unsaved edits. Commit or stash first",
                    count(*n as usize, "new commit")
                ),
                format!(
                    "원격에 새 커밋 {n}개, 그런데 저장 안 한 수정이 있음 — 커밋하거나 stash 하세요"
                ),
            ),
            SayUnplanned(n, why) => (
                format!(
                    "{} upstream, but they couldn't be read: {why}",
                    count(*n as usize, "new commit")
                ),
                format!("원격에 새 커밋 {n}개, 그런데 읽지 못함: {why}"),
            ),
            SayCanBringIn(n) => (
                format!(
                    "{} upstream. Ready to bring in",
                    count(*n as usize, "new commit")
                ),
                format!("원격에 새 커밋 {n}개 — 받아올 수 있어요"),
            ),
            SaySame => s(
                "Same as the remote. Nothing to do",
                "원격과 같음 — 할 일 없음",
            ),
            SayAhead(n) => (
                format!(
                    "{} of yours {} on the remote yet. Push first",
                    count(*n as usize, "commit"),
                    if *n == 1 { "isn't" } else { "aren't" }
                ),
                format!("내 커밋 {n}개가 아직 원격에 없음 — 먼저 push 하세요"),
            ),
            SayDiverged(ahead, behind, outlook) => {
                let en_split = format!(
                    "You have {} and upstream has {}.",
                    count(*ahead as usize, "new commit"),
                    count(*behind as usize, "new commit")
                );
                let ko_split = format!("내 새 커밋 {ahead}개와 원격 새 커밋 {behind}개로 갈라짐 —");
                let (en, ko) = match outlook {
                    Outlook::Clean => s(
                        "Merging would go cleanly. Merge by hand",
                        "합치면 충돌 없음, 직접 합치세요",
                    ),
                    Outlook::Conflicts(files) => (
                        format!(
                            "Merging would conflict in {}",
                            conflict_list(files, Lang::En)
                        ),
                        format!("합치면 {}에서 충돌 예상", conflict_list(files, Lang::Ko)),
                    ),
                    Outlook::Unavailable(why) => (
                        format!("Conflicts couldn't be predicted: {why}"),
                        format!("충돌을 예측하지 못함: {why}"),
                    ),
                    Outlook::NeedsNewerGit => (
                        format!("Predicting conflicts needs {git} or later. Merge by hand"),
                        format!("충돌 예측에는 {git} 이상이 필요함, 직접 합치세요"),
                    ),
                    Outlook::Unknown => s("Merge by hand", "직접 합치세요"),
                };
                (format!("{en_split} {en}"), format!("{ko_split} {ko}"))
            }
            SayNoUpstream => s(
                "No remote branch is linked, so there's nothing to compare against",
                "연결된 원격 브랜치가 없음 — 비교할 대상이 없어요",
            ),
            SayUpstreamGone => s(
                "The remote branch it followed is gone",
                "따라가던 원격 브랜치가 사라짐",
            ),
            SayNoCommits => s(
                "No commits yet, so there's nothing to compare",
                "아직 커밋이 없음 — 비교할 게 없어요",
            ),
            MoreAbove(n) => (
                format!("↑ {} above (b)", count(*n, "more line")),
                format!("↑ 위에 {n}줄 더 (b)"),
            ),
            MoreBelow(n) => (
                format!("↓ {} below (f)", count(*n, "more line")),
                format!("↓ 아래에 {n}줄 더 (f)"),
            ),
            Keys { all_picked } => {
                let (en, ko) = match all_picked {
                    false => ("a pick all", "a 전부 고르기"),
                    true => ("a unpick all", "a 전부 풀기"),
                };
                (
                    format!("↑↓ move   space pick   {en}   enter next   q quit   l 한국어"),
                    format!("↑↓ 이동   스페이스 고르기   {ko}   엔터 다음   q 그만   l English"),
                )
            }
            Picked(n) => (format!("{n} picked"), format!("{n}개 골랐어요")),
            BringThese(n) => (
                format!("Bring these {} up to date:", count(*n, "clone")),
                format!("이 {n}개를 원격 최신으로 받아옵니다:"),
            ),
            AndMore(n) => (format!("  … and {n} more"), format!("  … 외 {n}개")),
            GoOrBack => s(
                "enter go   any other key back",
                "엔터 진행   다른 키 돌아가기",
            ),

            Updated(label) => (format!("{label}: updated"), format!("{label}: 받아옴")),
            Skipped(label, change) => {
                let (en, ko) = match change {
                    Change::Dirty => ("now dirty", "수정이 생김"),
                    Change::HeadMoved => ("HEAD moved", "HEAD가 바뀜"),
                };
                (
                    format!("{label}: skipped: {en} since the list was made"),
                    format!("{label}: 건너뜀: 목록을 만든 뒤 {ko}"),
                )
            }
            Failed(label, m) => (
                format!("{label}: failed: {m}"),
                format!("{label}: 실패: {m}"),
            ),

            LanguagePrompt => s("Language", "언어"),
            FolderToTrack => s(
                "Folder to track (leave empty to finish)",
                "추적할 폴더 (비워 두고 엔터면 끝)",
            ),
            Tracking(path, n) => (
                format!("tracking {path} ({})", count(*n, "clone")),
                format!("추적함: {path} (저장소 {n}개)"),
            ),
            AlreadyTracked(path) => (
                format!("already tracked: {path}"),
                format!("이미 추적 중: {path}"),
            ),
            SavedTo(path) => (format!("saved to {path}"), format!("저장함: {path}")),
            SetupNeedsTerminal => s(
                "setup asks questions, so it needs a terminal",
                "setup은 질문을 하므로 터미널이 필요해요",
            ),
            ListClones(path, n) => (
                format!("{path}  {}", count(*n, "clone")),
                format!("{path}  저장소 {n}개"),
            ),
            ListMissing(path, e) => (
                format!("{path}  missing: {e}"),
                format!("{path}  없음: {e}"),
            ),
            ConfigAt(path) => (format!("config: {path}"), format!("설정 파일: {path}")),
            NothingTracked => s(
                "no folders are tracked yet: run `just-fetch setup` to add some",
                "추적하는 폴더가 아직 없음: `just-fetch setup`으로 추가하세요",
            ),
            NothingToRemove => s(
                "nothing is tracked, so there is nothing to remove",
                "추적하는 폴더가 없어 뺄 것이 없음",
            ),
            RemoveNeedsTerminal => s(
                "remove asks which folders to drop, so it needs a terminal",
                "remove는 뺄 폴더를 물어보므로 터미널이 필요해요",
            ),
            MissingMark => s("(missing)", "(없음)"),
            StopTrackingPrompt => s(
                "Folders to stop tracking (space picks, enter confirms, esc leaves)",
                "추적을 그만둘 폴더 (스페이스 고르기, 엔터 확인, esc 나가기)",
            ),
            StopTracking(n) => (
                format!("Stop tracking {}?", count(*n, "folder")),
                format!("폴더 {n}개 추적을 그만둘까요?"),
            ),

            CannotRead(path, e) => (
                format!("cannot read {path}: {e}"),
                format!("{path} 읽을 수 없음: {e}"),
            ),
            CannotWrite(path, e) => (
                format!("cannot write {path}: {e}"),
                format!("{path} 쓸 수 없음: {e}"),
            ),
            CannotTrack(path, why) => (
                format!("cannot track {path}: {why}"),
                format!("{path} 추적할 수 없음: {why}"),
            ),
            NotAFolder => s("not a folder", "폴더가 아님"),
            GitMissing => s("git was not found on PATH", "PATH에서 git을 찾지 못함"),
            NoOwnPath => s(
                "cannot find just-fetch's own path, so the clone's hooks cannot be turned off",
                "just-fetch 자신의 경로를 찾지 못해 저장소의 훅을 끌 수 없음",
            ),
            NoConfigFolder => s(
                "cannot find a folder for the config file",
                "설정 파일을 둘 폴더를 찾지 못함",
            ),
            NoneRead => s(
                "none of the tracked folders could be read",
                "추적하는 폴더를 하나도 읽지 못함",
            ),
            ScreenFailed(e) => (
                format!("the pick screen failed: {e}"),
                format!("선택 화면을 띄우지 못함: {e}"),
            ),
            CannotDraw => s(
                "this terminal cannot draw it",
                "이 터미널은 화면을 그릴 수 없음",
            ),
            UpgradingWithBrew(brew) => (
                format!("upgrading just-fetch with {brew}"),
                format!("{brew}로 just-fetch를 올립니다"),
            ),
            BrewMissing(brew) => (
                format!("installed with brew, but there is no brew at {brew}"),
                format!("brew로 설치됐지만 {brew}에 brew가 없음"),
            ),
            BrewFailed(step) => (
                format!("brew {step} failed, so just-fetch was not upgraded"),
                format!("brew {step}이 실패해 just-fetch를 올리지 못함"),
            ),
            UpgradeWithScoop => s(
                "installed with scoop: run `scoop update just-fetch` once just-fetch has exited",
                "scoop으로 설치됨: just-fetch가 끝난 뒤 `scoop update just-fetch`를 실행하세요",
            ),
            UpgradeByHand(url) => (
                format!("installed by hand: get the newest archive from {url}"),
                format!("직접 설치함: {url}에서 새 압축 파일을 받으세요"),
            ),
            UnexpectedGitOutput(out) => (
                format!("unexpected output from git rev-parse: {out}"),
                format!("git rev-parse의 출력이 예상과 다름: {out}"),
            ),
        }
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.in_lang(lang::current()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every kind of text.
    fn every() -> Vec<Text> {
        use Text::*;
        let m = || "msg".to_string();
        let files = || vec!["a.txt".to_string(); 7];
        vec![
            RowError(m()),
            NoCommitsHead(m()),
            Detached,
            FetchFailed(m()),
            FetchRefused(m()),
            NoUpstream,
            UpstreamGone,
            UpToDate,
            Behind(2),
            Ahead(2),
            Diverged(1, 2),
            MergesCleanly,
            Conflict(files()),
            ConflictCheckUnavailable(m()),
            ConflictCheckNeedsGit,
            Dirty,
            Untracked(2),
            Title,
            GroupCanBringIn,
            GroupNeedsYou,
            GroupUpToDate,
            SayUnreadable(m()),
            SayFetchFailed(m()),
            SayFetchRefused(m()),
            SayDirty(2),
            SayUnplanned(2, m()),
            SayCanBringIn(2),
            SaySame,
            SayAhead(2),
            SayDiverged(1, 2, Outlook::Clean),
            SayDiverged(1, 2, Outlook::Conflicts(files())),
            SayDiverged(1, 2, Outlook::Unavailable(m())),
            SayDiverged(1, 2, Outlook::NeedsNewerGit),
            SayDiverged(1, 2, Outlook::Unknown),
            SayNoUpstream,
            SayUpstreamGone,
            SayNoCommits,
            MoreAbove(2),
            MoreBelow(2),
            Keys { all_picked: false },
            Keys { all_picked: true },
            Picked(2),
            BringThese(2),
            AndMore(2),
            GoOrBack,
            Updated(m()),
            Skipped(m(), Change::Dirty),
            Skipped(m(), Change::HeadMoved),
            Failed(m(), m()),
            LanguagePrompt,
            FolderToTrack,
            Tracking(m(), 2),
            AlreadyTracked(m()),
            SavedTo(m()),
            SetupNeedsTerminal,
            ListClones(m(), 2),
            ListMissing(m(), m()),
            ConfigAt(m()),
            NothingTracked,
            NothingToRemove,
            RemoveNeedsTerminal,
            MissingMark,
            StopTrackingPrompt,
            StopTracking(2),
            CannotRead(m(), m()),
            CannotWrite(m(), m()),
            CannotTrack(m(), m()),
            NotAFolder,
            GitMissing,
            NoOwnPath,
            NoConfigFolder,
            NoneRead,
            ScreenFailed(m()),
            CannotDraw,
            UnexpectedGitOutput(m()),
            UpgradingWithBrew(m()),
            BrewMissing(m()),
            BrewFailed(m()),
            UpgradeWithScoop,
            UpgradeByHand(m()),
        ]
    }

    fn hangul(text: &str) -> bool {
        text.chars().any(|c| ('가'..='힣').contains(&c))
    }

    /// The English hint names Korean, the one place English text holds Hangul.
    fn english_part(text: &str) -> &str {
        text.split("l 한국어").next().unwrap()
    }

    #[test]
    fn every_text_is_korean_in_korean_and_english_in_english() {
        for text in every() {
            let (en, ko) = (text.in_lang(Lang::En), text.in_lang(Lang::Ko));
            assert!(hangul(&ko), "{text:?} has no Korean: {ko}");
            assert!(!hangul(english_part(&en)), "{text:?} is not English: {en}");
        }
    }

    /// Literals outside this file that hold words but are not said to a person.
    const NOT_SAID: [&str; 4] = [
        "core.gitProxy",
        "core.alternateRefsCommand",
        "just-fetch: {message}",
        "just added",
    ];

    /// What comes right before a literal that is shown or raised as an error.
    const SAID_AFTER: [&str; 9] = [
        "println!(",
        "eprintln!(",
        "print!(",
        "write!(f,",
        "with_prompt(",
        "io::Error::other(",
        "GitError::Failed(",
        "Err(",
        "panic!(",
    ];

    /// What comes right after a literal that is turned into a message.
    const SAID_BEFORE: [&str; 3] = [".to_string()", ".into()", ".to_owned()"];

    /// The string literals in `code`, each with the code before and after it.
    fn literals(code: &str) -> Vec<(&str, &str, &str)> {
        let bytes = code.as_bytes();
        let mut found = Vec::new();
        let mut i = 0;
        while let Some(open) = code[i..].find('"').map(|at| i + at) {
            if open > 0 && bytes[open - 1] == b'\'' {
                i = open + 1;
                continue;
            }
            let mut close = open + 1;
            while bytes[close] != b'"' {
                close += if bytes[close] == b'\\' { 2 } else { 1 };
            }
            found.push((&code[..open], &code[open + 1..close], &code[close + 1..]));
            i = close + 1;
        }
        found
    }

    /// Whether `literal` holds a word, outside its `{…}` placeholders.
    fn has_word(literal: &str) -> bool {
        let mut depth = 0;
        let mut run = 0;
        for c in literal.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                c if depth == 0 && c.is_ascii_alphabetic() => {
                    run += 1;
                    if run >= 2 {
                        return true;
                    }
                    continue;
                }
                _ => {}
            }
            run = 0;
        }
        false
    }

    #[test]
    fn no_other_file_says_words_of_its_own() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = 0;
        for entry in std::fs::read_dir(src).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().unwrap() == "text.rs" {
                continue;
            }
            files += 1;
            let source = std::fs::read_to_string(&path).unwrap();
            let code: String = source
                .split("#[cfg(test)]")
                .next()
                .unwrap()
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            for (before, literal, after) in literals(&code) {
                let before: String = before.chars().filter(|c| !c.is_whitespace()).collect();
                let said = SAID_AFTER
                    .iter()
                    .any(|m| before.ends_with(&m.replace(' ', "")))
                    || SAID_BEFORE.iter().any(|m| after.starts_with(m));
                assert!(
                    !said || !has_word(literal) || NOT_SAID.contains(&literal),
                    "{} says {literal:?} outside text.rs",
                    path.display()
                );
            }
        }
        assert!(files >= 15, "read only {files} files");
    }

    #[test]
    fn english_counts_are_singular_for_one() {
        assert_eq!(
            Text::SayCanBringIn(1).in_lang(Lang::En),
            "1 new commit upstream. Ready to bring in"
        );
        assert_eq!(
            Text::StopTracking(1).in_lang(Lang::En),
            "Stop tracking 1 folder?"
        );
        assert_eq!(
            Text::ListClones("p".into(), 2).in_lang(Lang::En),
            "p  2 clones"
        );
    }

    #[test]
    fn the_git_a_prediction_needs_is_named() {
        assert_eq!(
            Text::ConflictCheckNeedsGit.in_lang(Lang::En),
            "conflict check unavailable: needs git 2.38 or later"
        );
        assert_eq!(
            Text::ConflictCheckNeedsGit.in_lang(Lang::Ko),
            "충돌 확인 불가: git 2.38 이상 필요"
        );
    }

    #[test]
    fn a_long_conflict_list_is_cut_short_in_either_language() {
        let files: Vec<String> = (1..=7).map(|i| format!("f{i}")).collect();
        assert_eq!(
            Text::Conflict(files.clone()).in_lang(Lang::En),
            "conflict: f1, f2, f3, f4, f5 (+2 more)"
        );
        assert_eq!(
            Text::Conflict(files).in_lang(Lang::Ko),
            "충돌: f1, f2, f3, f4, f5 (외 2개)"
        );
    }
}
