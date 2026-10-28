use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::text::Text;

/// The variable that stands in for the path of the running just-fetch.
pub const EXE_PATH_VAR: &str = "JUST_FETCH_EXE_PATH";

/// Where just-fetch's releases are published.
pub const RELEASES: &str = "https://github.com/kihyun1998/just-fetch/releases";

/// The formula brew upgrades.
const FORMULA: &str = "kihyun1998/tap/just-fetch";

/// What put just-fetch where it is.
#[derive(Debug, PartialEq, Eq)]
pub enum Installed {
    /// Homebrew, whose prefix holds the `Cellar` just-fetch is in.
    Brew {
        prefix: PathBuf,
    },
    Scoop,
    ByHand,
}

/// What put the just-fetch at `exe` there, read from the path alone, with `/` or `\`.
pub fn installed_by(exe: &str) -> Installed {
    for separator in ['/', '\\'] {
        let cellar = format!("{separator}Cellar{separator}just-fetch{separator}");
        if let Some(at) = exe.find(&cellar) {
            return Installed::Brew {
                prefix: PathBuf::from(&exe[..at]),
            };
        }
    }
    let folded = exe.replace('/', "\\").to_ascii_lowercase();
    if folded.contains("\\scoop\\apps\\just-fetch\\") {
        return Installed::Scoop;
    }
    Installed::ByHand
}

/// The running just-fetch's own path, with links followed: `JUST_FETCH_EXE_PATH`, else the
/// executable's.
fn own_path() -> Result<PathBuf, Box<dyn Error>> {
    let exe = match std::env::var_os(EXE_PATH_VAR) {
        Some(path) => PathBuf::from(path),
        None => std::env::current_exe()?,
    };
    Ok(exe.canonicalize().unwrap_or(exe))
}

/// Brings just-fetch to its newest release through what installed it: brew is run, scoop and
/// a hand install are told what to do.
pub fn upgrade() -> Result<ExitCode, Box<dyn Error>> {
    let exe = own_path()?;
    match installed_by(&exe.to_string_lossy()) {
        Installed::Brew { prefix } => with_brew(&prefix.join("bin").join("brew")),
        Installed::Scoop => {
            println!("{}", Text::UpgradeWithScoop);
            Ok(ExitCode::SUCCESS)
        }
        Installed::ByHand => {
            println!("{}", Text::UpgradeByHand(RELEASES.to_string()));
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// `brew update`, then `brew upgrade` of the formula, each shown as brew prints it. The first
/// that fails ends it with brew's exit code.
fn with_brew(brew: &Path) -> Result<ExitCode, Box<dyn Error>> {
    if !brew.is_file() {
        return Err(Text::BrewMissing(brew.display().to_string())
            .to_string()
            .into());
    }
    println!("{}", Text::UpgradingWithBrew(brew.display().to_string()));
    for args in [vec!["update"], vec!["upgrade", FORMULA]] {
        let status = Command::new(brew).args(&args).status()?;
        if !status.success() {
            let code = status.code().map_or(1, |code| code.clamp(1, 255) as u8);
            eprintln!("{}", Text::BrewFailed(args.join(" ")));
            return Ok(ExitCode::from(code));
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_in_a_brew_cellar_is_brew_with_the_prefix_above_it() {
        for (exe, prefix) in [
            (
                "/opt/homebrew/Cellar/just-fetch/0.2.0/bin/just-fetch",
                "/opt/homebrew",
            ),
            (
                "/usr/local/Cellar/just-fetch/0.2.0/bin/just-fetch",
                "/usr/local",
            ),
            (r"C:\odd\Cellar\just-fetch\1.0\bin\just-fetch", r"C:\odd"),
        ] {
            assert_eq!(
                installed_by(exe),
                Installed::Brew {
                    prefix: PathBuf::from(prefix)
                },
                "{exe}"
            );
        }
    }

    #[test]
    fn a_path_in_scoop_apps_is_scoop_in_any_case_or_separator() {
        for exe in [
            r"C:\Users\me\scoop\apps\just-fetch\current\just-fetch.exe",
            r"\\?\C:\Users\me\Scoop\Apps\Just-Fetch\0.2.0\just-fetch.exe",
            "/home/me/scoop/apps/just-fetch/current/just-fetch.exe",
        ] {
            assert_eq!(installed_by(exe), Installed::Scoop, "{exe}");
        }
    }

    #[test]
    fn anything_else_is_by_hand() {
        for exe in [
            "/Users/me/bin/just-fetch",
            r"C:\tools\just-fetch.exe",
            r"D:\tools\apps\just-fetch\just-fetch.exe",
            "/opt/homebrew/Cellar/other/1.0/bin/just-fetch",
            "/Users/me/.cargo/bin/just-fetch",
        ] {
            assert_eq!(installed_by(exe), Installed::ByHand, "{exe}");
        }
    }
}
