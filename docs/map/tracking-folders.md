# Tracking folders

How a run with no path knows what to cover: `just-fetch setup` (`setup::setup`) asks for folders one at a time and saves them with `tracked::save`; a run with no path loads them with `tracked::load`, scans each with `scan::scan_folders`, and fetches every clone found in one batch (see [fetching-clones](fetching-clones.md)). Scanning on every run is what makes a clone made after setup show up without setting anything up again.

## Where the list lives

`config.toml` under the OS config folder, from the `dirs` crate: `~/Library/Application Support/just-fetch` on macOS, `%APPDATA%\just-fetch` (roaming AppData) on Windows. `JUST_FETCH_CONFIG_DIR` names another folder instead; the tests point it at a temp folder so they never read or write the person's own list, and `tests/cli.rs`'s `just_fetch()` points it at a folder that does not exist, so a test that gives no path sees nothing tracked. Both chosen by the maintainer.

A missing file is an empty list, not an error, so the first run needs no file.

## How a folder is stored

As the absolute path the person gave (`std::path::absolute`), not the canonical one. `canonicalize` on Windows returns the `\\?\C:\…` form, and on macOS turns `/var` into `/private/var`; either would then show in the headings and in the file. Whether a folder is already tracked is decided on canonical paths, so `~/code` and `~/code/sub/..` are one folder. A path that does not exist, or a file, is refused.

`setup` reads `~` and `~/…` as the home folder, and drops one pair of surrounding quotes, since a typed answer does not pass through a shell and Windows Explorer's *Copy as path* quotes what it copies. Each folder is saved the moment it is added, so leaving setup with Ctrl-C keeps what was added before.

## A run over several folders

- **Headings**: each folder's rows sit under a line with its path, only when more than one folder is tracked. One folder prints the same rows a run with a path does. Chosen by the maintainer.
- **A folder that cannot be read** (moved, unplugged) prints its heading and the reason, and the other folders still run; the run succeeds unless no folder could be read, so a script notices when every folder is gone (chosen by the maintainer). A path given on the command line that cannot be read still fails the run, as before.
- **A folder listed twice** (a hand-edited file, another spelling of the same path) is covered once, at its first place.
- **A clone reached through two tracked folders** (one folder inside another, or a symlink) is listed once, under the first folder that reaches it, compared by canonical path.
- **The pick prompt** offers clones from every folder in one list.

## With nothing tracked

A run with no path and an empty list starts `setup` when stdin, stdout and stderr are all terminals (`prompt::at_terminal`, the same test the pick prompt uses), then covers what was just added; a setup that adds nothing ends the run with the same line as below. Anywhere else it fails with a line naming `just-fetch setup`, so a script never waits on a question.

## The name `setup`

`setup` is a subcommand, so `just-fetch setup` never means a folder named `setup` in the current folder; `just-fetch ./setup` does.
