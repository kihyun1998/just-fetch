# just-fetch

Fetches every git clone in a folder, shows how each one stands against its upstream, and fast-forwards the ones you pick. For Windows and macOS.

```text
$ just-fetch ~/code
autonamu          main  -> origin/main  ahead 2
boring_avatars    main  -> origin/main  behind 2  dirty
cover-letters     main  -> origin/main  behind 1
notes             main  -> origin/main  diverged +1 -2  conflict: todo.md
scratch           main  no upstream  3 untracked
Pick clones to fast-forward (space picks, enter confirms, esc leaves):
> [ ] cover-letters  behind 1
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
just-fetch [PATH]
```

`PATH` is either a clone, or a folder whose subfolders are clones (one level down). With no path, it uses the current folder.

For each clone, just-fetch fetches the remote its current branch tracks, then shows:

| Status | Meaning |
|---|---|
| `up to date` | nothing new on either side |
| `behind N` | the remote has N commits you don't, and you have none it doesn't |
| `ahead N` | you have N commits the remote doesn't |
| `diverged +A -B` | both sides have new commits, followed by `merges cleanly` or `conflict: <files>` from a trial merge that changes nothing |
| `upstream gone` | the branch it tracks was deleted on the remote |
| `no upstream` | the branch tracks nothing, so it isn't fetched |
| `fetch failed: …` | git's message, e.g. for a remote you can't reach |

`dirty` means uncommitted changes to tracked files. `N untracked` counts new files; they don't count as dirty.

In a terminal, it then offers every clone that is behind and not dirty. Space picks a clone, enter fast-forwards the ones picked, and esc leaves with nothing changed. A clone that changes while the prompt is open (an edit, a new commit) is skipped. When the output is piped or scripted, it only prints the list and changes nothing.

## License

MIT OR Apache-2.0, at your option.
