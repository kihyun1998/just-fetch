# Fast-forwarding

How just-fetch moves the clones a person picks up to their upstream: `pull::pullable` decides who may be offered, `main::offer_fast_forward` asks, and `pull::fast_forward` does it. This is the only step that changes a working tree.

## Who is offered

A clone that is Behind and not Dirty. Untracked files alone do not block it: git refuses a fast-forward that would overwrite an untracked file and leaves the file as it was (tested). Every other clone stays out of the prompt. The table already says why: dirty, ahead, diverged, up to date, no upstream, upstream gone, fetch failed, or an error. A failed fetch blocks even a Behind clone, because its comparison is stale.

## Asking

A `dialoguer` multi-select with nothing picked to start with. Space picks, enter confirms, esc leaves. Leaving, or confirming with nothing picked, changes nothing. The prompt appears only when stdin, stdout and stderr are all terminals, so a piped or scripted run, `just-fetch | less` included, prints the table and changes nothing, with nothing on stderr either. A prompt that fails to start ends the run with an error rather than a silent success.

The prompt is tested through a pseudo-terminal with `portable-pty`. That is a Unix pty on macOS and ConPTY on Windows, the console layer Windows Terminal hosts. CI runs the tests on macos-latest, windows-latest and windows-11-arm. Three things about ConPTY shaped the harness, each seen failing on CI first:

- **The child must be attached to the pseudo-console explicitly.** expectrl's `conpty` backend starts the child without `STARTF_USESTDHANDLES`. Under a test runner whose stdout is redirected, the child then wrote its table to the runner's output and was never a terminal. `portable-pty` sets the flag with invalid handles.
- **ConPTY asks where the cursor is (`ESC[6n`) and holds the child's output until it is answered.** A real terminal answers by itself. The harness answers `ESC[1;1R`.
- **ConPTY may redraw a run of spaces as a cursor move**, so screen text is compared with escape sequences and whitespace stripped. For the same reason, the harness waits for the process to exit, then for the output to settle, rather than for end-of-file.

What no test sees is how Windows Terminal itself paints the prompt.

## The call

Before asking, `pull::plan` reads each candidate's HEAD and upstream commit (`rev-parse HEAD @{upstream}`). After a person picks, `pull::fast_forward` reads the clone's status again and does nothing (`skipped: …`) if it has turned dirty or its HEAD has moved since the list was made: a person can sit at the prompt for minutes, and `--ff-only` alone lets a dirty tree through when the incoming commits do not touch the edited files (measured). Then `git merge --ff-only --quiet <the planned upstream commit>`, through `git::run_with_clone_hooks`.

- **Merge, not pull.** `git pull` fetches again, so it can bring commits the table never showed. Merging the planned upstream commit applies exactly the commits shown as "behind N", even if the upstream moves while the prompt is open. Chosen by the maintainer.
- **The clone's own hooks run**: `post-merge`, `post-checkout`, whatever the clone has. Unlike the same command typed in the clone, stdin is empty, output is captured, fsmonitor is off and git does not prompt. A person picked this clone and asked for this change. That is the same reason auth settings are left alone in [fetching-clones](fetching-clones.md). Chosen by the maintainer. The automatic steps (status, fetch, prediction) still run with hooks off.
- **Filters still run.** Checking out the new commits runs any smudge filter the config names (git-lfs, for one). Turning filters off would break those clones, so they are left on.
- `--ff-only` refuses anything but a fast-forward, and a refusal leaves HEAD, the index and the working tree as they were.

## Showing it

One line per picked clone after the prompt: `name: updated`, `name: skipped: <what changed> since the list was made`, or `name: failed: <git's message on one line>`.
