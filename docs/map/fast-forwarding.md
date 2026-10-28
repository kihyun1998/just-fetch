# Fast-forwarding

How just-fetch moves the clones a person picks up to their upstream: `fast_forward::offerable` decides who may be offered, `prompt::offer_fast_forward` asks through the pick screen, and `fast_forward::fast_forward` does it. This is the only step that changes a working tree.

## Who is offered

A clone that is Behind and not Dirty. Untracked files alone do not block it: git refuses a fast-forward that would overwrite an untracked file and leaves the file as it was (tested). Every other clone stays out of the pick: the screen and the table say why: dirty, ahead, diverged, up to date, no upstream, upstream gone, fetch failed, or an error. A failed fetch blocks even a Behind clone, because its comparison is stale; `Refreshed::fresh_status` holds that rule for both this and prediction.

## Asking

The pick screen (`pick::Screen`, driven by `prompt::offer_fast_forward`) replaces the table at a terminal. It is the prototype the maintainer chose, variant d on the `prototype/select-ux` branch: a line saying what the screen does, then three groups with counts, *Can bring in*, *Needs you first (can't be picked here)* and *Already up to date*, each clone with a sentence instead of git terms (`pick::describe`). A clone that cannot be planned goes under *Needs you first* with the reason. The sentences are in English or Korean; `l` switches between them (see [choosing-the-language](choosing-the-language.md)).

- **Keys.** The cursor moves only over *Can bring in* (↑↓, j/k) and stops at either end. Space picks, `a` picks every row or, when every row is already picked, none. Enter with something picked shows *Bring these N up to date:*; enter there does it, any other key goes back to the list as it was. `q`, esc and Ctrl-C leave with nothing changed. Enter with nothing picked does nothing.
- **Height.** The screen is drawn on the terminal's alternate screen, cut to its height and width, so the shell's own lines come back when it closes and only the outcome lines stay. A taller list follows the cursor, and `f`/`b` move it a screen down and up, marked `↓ N more lines below (f)`. PgDn and PgUp are not used: `console`, which reads the keys, maps neither on Windows (0.15.11 and 0.16.6 read); the maintainer chose `f`/`b` on every OS.
- **Windows.** `console` turns on VT processing (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`) only when asked whether the terminal supports colours, so `prompt::show` asks before writing the alternate-screen sequence, and clears with `ESC[H ESC[2J` rather than `Term::clear_screen`, which on Windows clears the console buffer through the Win32 API instead. A terminal that cannot turn VT on gets an error rather than a garbled screen. Read from console 0.15.11's source; not seen on a real Windows console.
- **Nothing to bring in.** The groups are printed as plain lines and nothing is asked, so a run with nothing to do needs no key press.
- **Ctrl-C between two keys.** `console` turns raw mode on only while it reads a key. A Ctrl-C that lands while the screen is redrawing reaches the process as SIGINT, which ends it without leaving the alternate screen or showing the cursor again.

The screen appears only when stdin, stdout and stderr are all terminals, so a piped or scripted run, `just-fetch | less` included, prints the table and changes nothing, with nothing on stderr either. A screen that fails to start ends the run with an error rather than a silent success.

The screen is tested through a pseudo-terminal with `portable-pty`. That is a Unix pty on macOS and ConPTY on Windows, the console layer Windows Terminal hosts. CI runs the tests on macos-latest, windows-latest and windows-11-arm. Three things about ConPTY shaped the harness, each seen failing on CI first:

- **The child must be attached to the pseudo-console explicitly.** expectrl's `conpty` backend starts the child without `STARTF_USESTDHANDLES`. Under a test runner whose stdout is redirected, the child then wrote its table to the runner's output and was never a terminal. `portable-pty` sets the flag with invalid handles.
- **ConPTY asks where the cursor is (`ESC[6n`) and holds the child's output until it is answered.** A real terminal answers by itself. The harness answers `ESC[1;1R`.
- **ConPTY may redraw a run of spaces as a cursor move**, so screen text is compared with escape sequences and whitespace stripped. For the same reason, the harness waits for the process to exit, then for the output to settle, rather than for end-of-file.

What no test sees is how Windows Terminal itself paints the screen. The screen's state (cursor, picks, scrolling, the lines drawn) is also tested without a terminal, in `pick`'s unit tests.

## The call

Before asking, `fast_forward::plan` reads each candidate's HEAD and upstream commit (`git::head_and_upstream`, which prediction uses too). After a person picks, `fast_forward::fast_forward` reads the clone's status again and does nothing (`skipped: …`) if it has turned dirty or its HEAD has moved since the list was made: a person can sit at the screen for minutes, and `--ff-only` alone lets a dirty tree through when the incoming commits do not touch the edited files (measured). Then `git merge --ff-only --quiet <the planned upstream commit>`, through `git::run_with_clone_hooks`.

- **Merge, not pull.** `git pull` fetches again, so it can bring commits the table never showed. Merging the planned upstream commit applies exactly the commits shown as "behind N", even if the upstream moves while the screen is open. Chosen by the maintainer.
- **The clone's own hooks run**: `post-merge`, `post-checkout`, whatever the clone has. Unlike the same command typed in the clone, stdin is empty, output is captured, fsmonitor is off and git does not prompt. A person picked this clone and asked for this change. That is the same reason auth settings are left alone in [fetching-clones](fetching-clones.md). Chosen by the maintainer. The automatic steps (status, fetch, prediction) still run with hooks off.
- **Filters still run.** Checking out the new commits runs any smudge filter the config names (git-lfs, for one). Turning filters off would break those clones, so they are left on.
- `--ff-only` refuses anything but a fast-forward, and a refusal leaves HEAD, the index and the working tree as they were.

## Showing it

One line per picked clone after the screen closes, by the same label the screen used: `name: updated`, `name: skipped: <what changed> since the list was made`, or `name: failed: <git's message on one line>`.
