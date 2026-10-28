# just-fetch

Fetches every git clone in the folders you track, shows how each one stands against its upstream, and fast-forwards the ones you pick. For Windows and macOS.

```text
$ just-fetch setup   # once: pick the language and the folders to track
$ just-fetch
Pick the clones you want to bring up to date with their remote. Your unsaved edits are never touched.

Can bring in (1)
> [ ] cover-letters   1 new commit upstream. Ready to bring in

Needs you first (can't be picked here) (4)
      autonamu        2 commits of yours aren't on the remote yet. Push first
      boring_avatars  2 new commits upstream, but you have unsaved edits. Commit or stash first
      notes           You have 1 new commit and upstream has 2 new commits. Merging would conflict in todo.md
      scratch         No remote branch is linked, so there's nothing to compare against

Already up to date (0)


↑↓ move   space pick   a pick all   enter next   q quit   l 한국어
0 picked
```

## Install

**macOS** with [Homebrew](https://brew.sh):

```sh
brew install kihyun1998/tap/just-fetch
```

**Windows** with [Scoop](https://scoop.sh):

```powershell
scoop bucket add kihyun1998 https://github.com/kihyun1998/scoop-bucket
scoop install just-fetch
```

**Either**, by hand: download the archive for your machine from [Releases](https://github.com/kihyun1998/just-fetch/releases), check it against `SHA256SUMS`, and put `just-fetch` (or `just-fetch.exe`) on your `PATH`.

| Machine | Archive |
|---|---|
| Mac, Apple silicon | `just-fetch-v<version>-aarch64-apple-darwin.tar.gz` |
| Mac, Intel | `just-fetch-v<version>-x86_64-apple-darwin.tar.gz` |
| Windows, x64 | `just-fetch-v<version>-x86_64-pc-windows-msvc.zip` |
| Windows, ARM | `just-fetch-v<version>-aarch64-pc-windows-msvc.zip` |

just-fetch runs the `git` already installed, so it signs in the way your git does. Conflict prediction needs git 2.38 or later; with an older git that column says it is unavailable.

## Use

```sh
just-fetch setup     # add folders to track
just-fetch list      # show the tracked folders and where the list is kept
just-fetch remove    # pick folders to stop tracking
just-fetch upgrade   # bring just-fetch itself up to date
just-fetch           # cover every tracked folder
just-fetch PATH      # cover PATH this once
```

`setup` asks for the language first (English or 한국어), then for folders one at a time; leave the answer empty to finish. Each folder is either a clone, or a folder whose subfolders are clones (one level down). They are scanned again on every run, so a clone you make later shows up by itself. The list is kept in `config.toml` under `~/Library/Application Support/just-fetch` on macOS and `%APPDATA%\just-fetch` on Windows.

`list` shows each tracked folder with how many clones it holds, or `missing` with the reason when it cannot be read. `remove` lets you pick folders to drop (space picks, enter confirms, esc leaves), asks once more, and only then saves; it needs a terminal.

`upgrade` uses what installed just-fetch. With Homebrew it runs `brew update` and then `brew upgrade kihyun1998/tap/just-fetch`. With Scoop it tells you to run `scoop update just-fetch` once just-fetch has exited, since Scoop will not update a program that is still running. Installed by hand, it points to the releases page.

`PATH` covers one clone or folder for this run and leaves the tracked list alone. With nothing tracked and no path, just-fetch starts `setup` in a terminal, and otherwise says to run it.

For each clone, just-fetch fetches the remote its current branch tracks. When the output is piped or scripted, it prints one line per clone, under each folder's path when more than one folder is tracked:

| Status | Meaning |
|---|---|
| `up to date` | nothing new on either side |
| `behind N` | the remote has N commits you don't, and you have none it doesn't |
| `ahead N` | you have N commits the remote doesn't |
| `diverged +A -B` | both sides have new commits, followed by `merges cleanly` or `conflict: <files>` from a trial merge that changes nothing |
| `upstream gone` | the branch it tracks was deleted on the remote |
| `no upstream` | the branch tracks nothing, so it isn't fetched |
| `fetch failed: …` | git's message, e.g. for a remote you can't reach |
| `not fetched: …` | the clone's own config sets a key that would run a command during the fetch (`core.gitProxy`, its remote's `vcs`, `core.alternateRefsCommand`), so it is left alone |

`dirty` means uncommitted changes to tracked files. `N untracked` counts new files; they don't count as dirty.

In a terminal, the same clones show as the screen above instead, in three groups and in plain words: the ones it can bring up to date (behind, nothing uncommitted), the ones that need you first, and the ones already up to date. The cursor moves only over the first group (↑↓ or j/k). Space picks a clone, `a` picks all of them or, once all are picked, none, and enter shows what will be brought in; enter again does it, and any other key goes back. `q` or esc leaves with nothing changed, and `l` switches the language. A list taller than the terminal scrolls with the cursor, and `f` and `b` move it a screen at a time. Clones with the same name in two tracked folders show their folder in front (`work/api`). A clone that changes while the screen is open (an edit, a new commit) is skipped. With nothing to bring in, the groups are printed and nothing is asked. A piped or scripted run only prints the lines above and changes nothing.

### Coming from 0.1

`just-fetch` with no path used to cover the current folder. It now covers the tracked folders, and starts `setup` when there are none. `just-fetch .` covers the current folder as before.

## Language

just-fetch speaks English or Korean: the language `setup` saved, else your OS language (Korean when it is Korean, English otherwise). On the pick screen, `l` switches to the other one for the rest of that run. git's own messages and `--help` stay in English.

## License

MIT OR Apache-2.0, at your option.
