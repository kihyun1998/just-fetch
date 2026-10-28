//! PROTOTYPE, throwaway: three ways to pick clones to fast-forward, on made-up clones.
//! Nothing here touches git. Run one of:
//!   cargo run --example prototype_select -- a   (today: table, then a checklist)
//!   cargo run --example prototype_select -- b   (one list: every clone, only behind ones pickable)
//!   cargo run --example prototype_select -- c   (full screen: list plus a detail pane)
//!   cargo run --example prototype_select -- d   (plain words, grouped, confirm first; `l` switches 한국어/English)

use console::{Key, Term, style};

struct Fake {
    name: &'static str,
    branch: &'static str,
    standing: &'static str,
    /// Why it cannot be picked; `None` when it can.
    blocked: Option<&'static str>,
    detail: &'static str,
}

const CLONES: &[Fake] = &[
    Fake {
        name: "autonamu",
        branch: "main",
        standing: "ahead 2",
        blocked: Some("ahead: push first"),
        detail: "2 local commits not on origin/main",
    },
    Fake {
        name: "boring_avatars",
        branch: "main",
        standing: "behind 2  dirty",
        blocked: Some("dirty: commit or stash"),
        detail: "M src/index.ts\nM README.md",
    },
    Fake {
        name: "cover-letters",
        branch: "main",
        standing: "behind 1",
        blocked: None,
        detail: "a1b2c3d Update 2026 letter",
    },
    Fake {
        name: "flutter_oss_manager",
        branch: "main",
        standing: "behind 33",
        blocked: None,
        detail: "33 commits, newest: 9f8e7d6 Release 2.1.0",
    },
    Fake {
        name: "notes",
        branch: "main",
        standing: "diverged +1 -2  conflict: todo.md",
        blocked: Some("diverged: merge by hand"),
        detail: "predicted conflict in todo.md",
    },
    Fake {
        name: "scratch",
        branch: "main",
        standing: "no upstream  3 untracked",
        blocked: Some("no upstream"),
        detail: "branch tracks nothing",
    },
    Fake {
        name: "justscene",
        branch: "master",
        standing: "fetch failed: repository not found",
        blocked: Some("fetch failed"),
        detail: "remote: Repository not found.",
    },
    Fake {
        name: "just-pdf-web",
        branch: "master",
        standing: "behind 2",
        blocked: None,
        detail: "e5f6a7b Fix print margins\n0c1d2e3 Bump deps",
    },
    Fake {
        name: "kubeckup",
        branch: "main",
        standing: "up to date",
        blocked: Some("up to date"),
        detail: "nothing new",
    },
];

fn table() -> String {
    let width = CLONES.iter().map(|c| c.name.len()).max().unwrap_or(0);
    CLONES
        .iter()
        .map(|c| format!("{:<width$}  {}  {}\n", c.name, c.branch, c.standing))
        .collect()
}

fn report(picked: &[usize]) {
    println!();
    if picked.is_empty() {
        println!("(nothing picked: nothing would change)");
    }
    for &i in picked {
        println!("{}: would fast-forward", CLONES[i].name);
    }
}

/// A: what just-fetch does today.
fn variant_a() {
    print!("{}", table());
    let offer: Vec<usize> = (0..CLONES.len())
        .filter(|&i| CLONES[i].blocked.is_none())
        .collect();
    let labels: Vec<String> = offer
        .iter()
        .map(|&i| format!("{}  {}", CLONES[i].name, CLONES[i].standing))
        .collect();
    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Pick clones to fast-forward (space picks, enter confirms, esc leaves)")
        .items(&labels)
        .interact_opt()
        .unwrap();
    match picked {
        Some(p) => report(&p.iter().map(|&k| offer[k]).collect::<Vec<_>>()),
        None => report(&[]),
    }
}

/// B: one list. Every clone is a row; only behind ones can be checked, the rest say why.
fn variant_b() {
    let term = Term::stdout();
    let mut cursor = 0;
    let mut picked = vec![false; CLONES.len()];
    let width = CLONES.iter().map(|c| c.name.len()).max().unwrap_or(0);
    loop {
        term.clear_screen().unwrap();
        println!(
            "{}",
            style("↑↓ move  space pick  a all behind  enter fast-forward  q leave").dim()
        );
        for (i, c) in CLONES.iter().enumerate() {
            let mark = match (c.blocked, picked[i]) {
                (Some(_), _) => style("[-]").dim().to_string(),
                (None, true) => style("[x]").green().to_string(),
                (None, false) => "[ ]".to_string(),
            };
            let pointer = if i == cursor { ">" } else { " " };
            let line = format!(
                "{pointer} {mark} {:<width$}  {:<7} {}",
                c.name, c.branch, c.standing
            );
            match c.blocked {
                Some(why) => println!("{}  {}", style(line).dim(), style(format!("({why})")).dim()),
                None => println!("{line}"),
            }
        }
        let chosen: Vec<&str> = (0..CLONES.len())
            .filter(|&i| picked[i])
            .map(|i| CLONES[i].name)
            .collect();
        println!("\n{} {}", style("picked:").bold(), chosen.join(", "));
        match term.read_key().unwrap() {
            Key::ArrowUp => cursor = cursor.saturating_sub(1),
            Key::ArrowDown => cursor = (cursor + 1).min(CLONES.len() - 1),
            Key::Char(' ') if CLONES[cursor].blocked.is_none() => picked[cursor] = !picked[cursor],
            Key::Char('a') => (0..CLONES.len())
                .filter(|&i| CLONES[i].blocked.is_none())
                .for_each(|i| picked[i] = true),
            Key::Enter => {
                return report(&(0..CLONES.len()).filter(|&i| picked[i]).collect::<Vec<_>>());
            }
            Key::Char('q') | Key::Escape => return report(&[]),
            _ => {}
        }
    }
}

/// C: full screen. The list on top, and below it what the highlighted clone would get.
fn variant_c() {
    let term = Term::stdout();
    term.hide_cursor().unwrap();
    let mut cursor = 0;
    let mut picked = vec![false; CLONES.len()];
    let width = CLONES.iter().map(|c| c.name.len()).max().unwrap_or(0);
    let result = loop {
        term.clear_screen().unwrap();
        let behind = CLONES.iter().filter(|c| c.blocked.is_none()).count();
        println!(
            "{}",
            style(format!(
                " just-fetch  {} clones  {} can fast-forward ",
                CLONES.len(),
                behind
            ))
            .reverse()
        );
        println!();
        for (i, c) in CLONES.iter().enumerate() {
            let mark = if c.blocked.is_some() {
                " "
            } else if picked[i] {
                "●"
            } else {
                "○"
            };
            let row = format!(" {mark} {:<width$}  {:<7} {}", c.name, c.branch, c.standing);
            if i == cursor {
                println!("{}", style(format!("{row:<90}")).reverse());
            } else if c.blocked.is_some() {
                println!("{}", style(row).dim());
            } else {
                println!("{row}");
            }
        }
        let c = &CLONES[cursor];
        println!("\n{}", style(format!("─ {} ", c.name)).bold());
        println!("{}", c.detail);
        if let Some(why) = c.blocked {
            println!("{}", style(format!("cannot fast-forward: {why}")).yellow());
        }
        println!(
            "\n{}",
            style("↑↓ move  space pick  a all behind  enter fast-forward  q leave").dim()
        );
        match term.read_key().unwrap() {
            Key::ArrowUp => cursor = cursor.saturating_sub(1),
            Key::ArrowDown => cursor = (cursor + 1).min(CLONES.len() - 1),
            Key::Char(' ') if c.blocked.is_none() => picked[cursor] = !picked[cursor],
            Key::Char('a') => (0..CLONES.len())
                .filter(|&i| CLONES[i].blocked.is_none())
                .for_each(|i| picked[i] = true),
            Key::Enter => break (0..CLONES.len()).filter(|&i| picked[i]).collect::<Vec<_>>(),
            Key::Char('q') | Key::Escape => break Vec::new(),
            _ => {}
        }
    };
    term.show_cursor().unwrap();
    term.clear_screen().unwrap();
    report(&result);
}

/// D: say what each clone means in plain words, group them, and confirm before acting.
#[derive(Clone, Copy, PartialEq)]
enum Group {
    CanUpdate,
    NeedsYou,
    Fine,
}

/// (group, Korean sentence, English sentence) for each made-up clone, in CLONES order.
const PLAIN: &[(Group, &str, &str)] = &[
    (
        Group::NeedsYou,
        "내 커밋 2개가 아직 원격에 없음 — 먼저 push 하세요",
        "2 of your commits aren't on the remote yet. Push them first",
    ),
    (
        Group::NeedsYou,
        "원격에 새 커밋 2개, 그런데 저장 안 한 수정이 있음 — 커밋하거나 stash 하세요",
        "2 new commits upstream, but you have unsaved edits. Commit or stash first",
    ),
    (
        Group::CanUpdate,
        "원격에 새 커밋 1개 — 받아올 수 있어요",
        "1 new commit upstream. Ready to bring in",
    ),
    (
        Group::CanUpdate,
        "원격에 새 커밋 33개 — 받아올 수 있어요",
        "33 new commits upstream. Ready to bring in",
    ),
    (
        Group::NeedsYou,
        "내 커밋과 원격 커밋이 갈라짐 — 합치면 todo.md에서 충돌 예상",
        "Your commits and upstream's have split. Merging would conflict in todo.md",
    ),
    (
        Group::NeedsYou,
        "연결된 원격 브랜치가 없음 — 비교할 대상이 없어요",
        "No remote branch is linked, so there's nothing to compare against",
    ),
    (
        Group::NeedsYou,
        "원격에 접속 실패: 저장소를 찾을 수 없음",
        "Couldn't reach the remote: repository not found",
    ),
    (
        Group::CanUpdate,
        "원격에 새 커밋 2개 — 받아올 수 있어요",
        "2 new commits upstream. Ready to bring in",
    ),
    (
        Group::Fine,
        "원격과 같음 — 할 일 없음",
        "Same as the remote. Nothing to do",
    ),
];

fn variant_d() {
    let term = Term::stdout();
    term.hide_cursor().unwrap();
    let mut korean = true;
    let pickable: Vec<usize> = (0..CLONES.len())
        .filter(|&i| PLAIN[i].0 == Group::CanUpdate)
        .collect();
    let mut at = 0;
    let mut picked = vec![false; CLONES.len()];
    let width = CLONES.iter().map(|c| c.name.len()).max().unwrap_or(0);
    let t = |korean: bool, ko: &'static str, en: &'static str| if korean { ko } else { en };
    let result = loop {
        term.clear_screen().unwrap();
        println!("{}", style(t(korean,
            "원격에 새 커밋이 있는 저장소를 골라 최신으로 받아옵니다. 내가 저장 안 한 수정은 건드리지 않아요.",
            "Pick the clones you want to bring up to date with their remote. Your unsaved edits are never touched.")).bold());
        println!();
        let sections = [
            (
                Group::CanUpdate,
                t(korean, "받아올 수 있음", "Can bring in"),
            ),
            (
                Group::NeedsYou,
                t(
                    korean,
                    "직접 처리해야 함 (여기서는 못 고름)",
                    "Needs you first (can't be picked here)",
                ),
            ),
            (Group::Fine, t(korean, "이미 최신", "Already up to date")),
        ];
        for (group, title) in sections {
            let rows: Vec<usize> = (0..CLONES.len()).filter(|&i| PLAIN[i].0 == group).collect();
            println!(
                "{}",
                style(format!("{title} ({})", rows.len())).underlined()
            );
            for i in rows {
                let sentence = if korean { PLAIN[i].1 } else { PLAIN[i].2 };
                if group == Group::CanUpdate {
                    let here = pickable[at] == i;
                    let box_ = if picked[i] {
                        style("[✓]").green().to_string()
                    } else {
                        "[ ]".to_string()
                    };
                    let line = format!(" {box_} {:<width$}  {sentence}", CLONES[i].name);
                    if here {
                        println!("{}", style(format!("{line:<100}")).reverse())
                    } else {
                        println!("{line}")
                    }
                } else {
                    println!(
                        "{}",
                        style(format!("     {:<width$}  {sentence}", CLONES[i].name)).dim()
                    );
                }
            }
            println!();
        }
        let n = picked.iter().filter(|&&p| p).count();
        let all = pickable.iter().all(|&i| picked[i]);
        let a_key = match (korean, all) {
            (true, false) => "a 전부 고르기",
            (true, true) => "a 전부 풀기",
            (false, false) => "a pick all",
            (false, true) => "a unpick all",
        };
        let hint = if korean {
            format!("↑↓ 이동   스페이스 고르기   {a_key}   엔터 다음   q 그만   l English")
        } else {
            format!("↑↓ move   space pick   {a_key}   enter next   q quit   l 한국어")
        };
        println!("{}", style(hint).dim());
        println!(
            "{}",
            if korean {
                format!("{n}개 골랐어요")
            } else {
                format!("{n} picked")
            }
        );
        match term.read_key().unwrap() {
            Key::ArrowUp | Key::Char('k') => at = at.saturating_sub(1),
            Key::ArrowDown | Key::Char('j') => at = (at + 1).min(pickable.len() - 1),
            Key::Char(' ') => picked[pickable[at]] = !picked[pickable[at]],
            Key::Char('a') => {
                let all = pickable.iter().all(|&i| picked[i]);
                pickable.iter().for_each(|&i| picked[i] = !all);
            }
            Key::Char('l') => korean = !korean,
            Key::Enter => {
                let chosen: Vec<usize> = (0..CLONES.len()).filter(|&i| picked[i]).collect();
                if chosen.is_empty() {
                    continue;
                }
                term.clear_screen().unwrap();
                println!(
                    "{}",
                    style(if korean {
                        format!("이 {}개를 원격 최신으로 받아옵니다:", chosen.len())
                    } else {
                        format!("Bring these {} up to date:", chosen.len())
                    })
                    .bold()
                );
                for &i in &chosen {
                    println!(
                        "  {}  {}",
                        CLONES[i].name,
                        if korean { PLAIN[i].1 } else { PLAIN[i].2 }
                    );
                }
                println!(
                    "
{}",
                    style(t(
                        korean,
                        "엔터 진행   다른 키 돌아가기",
                        "enter go   any other key back"
                    ))
                    .dim()
                );
                if term.read_key().unwrap() == Key::Enter {
                    break chosen;
                }
            }
            Key::Char('q') | Key::Escape => break Vec::new(),
            _ => {}
        }
    };
    term.show_cursor().unwrap();
    term.clear_screen().unwrap();
    report(&result);
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("a") => variant_a(),
        Some("b") => variant_b(),
        Some("c") => variant_c(),
        Some("d") => variant_d(),
        _ => eprintln!("pick a variant: cargo run --example prototype_select -- a|b|c"),
    }
}
