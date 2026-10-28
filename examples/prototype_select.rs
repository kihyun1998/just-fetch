//! PROTOTYPE, throwaway: three ways to pick clones to fast-forward, on made-up clones.
//! Nothing here touches git. Run one of:
//!   cargo run --example prototype_select -- a   (today: table, then a checklist)
//!   cargo run --example prototype_select -- b   (one list: every clone, only behind ones pickable)
//!   cargo run --example prototype_select -- c   (full screen: list plus a detail pane)

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

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("a") => variant_a(),
        Some("b") => variant_b(),
        Some("c") => variant_c(),
        _ => eprintln!("pick a variant: cargo run --example prototype_select -- a|b|c"),
    }
}
