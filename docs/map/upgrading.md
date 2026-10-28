# Upgrading

How `just-fetch upgrade` brings just-fetch itself to its newest release (the glossary's *Upgrade*): `upgrade::installed_by` reads what installed it from its own path, and `upgrade::upgrade` acts on that. just-fetch never replaces its own file and never asks GitHub for the latest version; the package manager that installed it owns both. Agreed with the maintainer in `/grill-with-docs`, recorded in #38.

## What installed it

The running program's path, links followed (`std::env::current_exe`, then `canonicalize`): brew's `bin/just-fetch` is a link into `Cellar/just-fetch/<version>/bin/`, and scoop runs the real program under `apps\just-fetch\` through a shim. `JUST_FETCH_EXE_PATH` stands in for that path in the tests.

- **brew**: the path holds `Cellar/just-fetch/`. The folder above `Cellar` is the prefix (`/opt/homebrew` on Apple silicon, `/usr/local` on Intel), and `<prefix>/bin/brew` is run, so a brew that is not on `PATH` still works.
  A Cellar moved away from its prefix with `HOMEBREW_CELLAR` leaves no brew at that place, so `upgrade` fails saying where it looked rather than guessing.
- **scoop**: the path holds `scoop\apps\just-fetch\`, in any case and with either separator. A scoop installed somewhere not named `scoop` counts as by hand, which only points to the releases; kept by the maintainer over looking for scoop's `shims` folder.
- **by hand**: anything else, a `cargo install` included.

## What each gets

- **brew**: `brew update`, then `brew upgrade kihyun1998/tap/just-fetch`, each with brew's own output, and just-fetch ends with the exit code of the first that fails. The update comes first because brew refreshes its formula list on its own only once a day by default (`HOMEBREW_AUTO_UPDATE_SECS`, 86400, in `man brew` of Homebrew 7.0.11), so a release minutes old would read as already installed. Chosen by the maintainer.
- **scoop**: one line naming `scoop update just-fetch`, and success. scoop skips an app whose program is still running (`test_running_process` in ScoopInstaller/Scoop `lib/install.ps1`, lines 533–550, used at `libexec/scoop-update.ps1` 307–309: "Running process detected, skip updating."), so a just-fetch that ran scoop and waited would never be upgraded. Starting scoop and exiting at once was turned down by the maintainer: whether scoop still sees just-fetch running is a matter of timing.
- **by hand**: one line with the releases page, success, no network.

## Tests

`tests/cli.rs` runs the binary with `JUST_FETCH_EXE_PATH` pointing into a made-up prefix whose `bin/brew` is a script that logs its arguments and fails on the step it is told to. That runs on Unix only; the scoop and hand-install cases run on every OS. `upgrade`'s unit tests read paths with both separators on every OS. No test runs a real brew.
