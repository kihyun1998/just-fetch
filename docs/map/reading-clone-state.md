# Reading a clone's local state

How just-fetch tells what a clone looks like without touching the network: one `git status` per clone, run by `status::read` and parsed by `status::parse`, from `scan::scan` and again after each fetch (see [fetching-clones](fetching-clones.md)).

A git call either answers, fails for this clone alone (`GitError::Failed`, its message stored with the clone), or finds no git at all (`GitError::Missing`), which ends the whole run. `git::split_missing` makes that split wherever a result is stored or acted on: a clone's state, fetch, prediction and fast-forward, the fast-forward plan, and git's version.

`tests/git_missing.rs` empties `PATH` for its whole process to make git vanish after the scan, so it holds that one test alone: a second test in the same crate would run beside it without git.

## One call, and why these flags

`git status --porcelain=v2 --branch --untracked-files=all`, started through `git::run`, which adds `-c core.fsmonitor=false`, `--no-optional-locks` and the settings listed in [fetching-clones](fetching-clones.md).

- **`--porcelain=v2 --branch`** is the format git documents as stable for scripts: it does not change with the user's locale or git version, and its `# branch.*` headers carry the branch, `(detached)`, `(initial)` for a branch with no commits, and the upstream. Lines starting `1`, `2` or `u` are changes to tracked files (Dirty); `?` lines are untracked files.
- **`-c core.fsmonitor=false`** keeps a clone's own config from running a command during the scan. `core.fsmonitor` in `.git/config` names a program `status` executes, so a clone unpacked from somewhere else could run anything the moment just-fetch walked past it. A command-line `-c` outranks the clone's config. The cost is fsmonitor's speed-up on very large clones, which a 15 ms-a-clone scan does not need. Chosen by the maintainer.
- **`--no-optional-locks`** stops `status` from refreshing and rewriting the index, so a scan never takes `index.lock` while an editor or another git process is using the clone.
- **`--untracked-files=all`** makes the untracked count a count of files. Without it git folds a new folder into one `?` line, so a folder of three new files read as one. Measured on 58 clones: no slower than the default mode.

## What counts as a clone

A folder with a `.git` entry, directory or file. The file form is what a worktree or submodule has, so checking for a directory alone would miss them.

## Names

A clone's name is the last component of its canonical path, so running with `.` inside a clone names the clone rather than `.`.

## Cost

Calls run one clone at a time: about 15 ms a clone on 58 clones, the same as a shell loop of the same command (0.87 s against 0.88 s, warm). The time is git's.
