# Fetching clones

How just-fetch brings each clone's view of its remote up to date and says how the clone stands against it: `fetch::fetch_all`, through `git::run`, read back by `status`.

## The flow

Read status → fetch the clones that have an upstream → read status again. The second read is where `# branch.ab +<ahead> -<behind>` gets its numbers against the freshly fetched remote-tracking branch. A clone with no upstream, or one git could not read, is not fetched.

## The fetch call, and why these flags

`git fetch --prune --quiet`, with no remote named.

- **No remote named** fetches the remote the current branch's upstream lives on (git-fetch: "When no remote is specified, by default the origin remote will be used, unless there's an upstream branch configured for the current branch"), so a clone's other remotes are left alone.
- **`--prune`** deletes remote-tracking branches whose branch is gone from the remote. Without it a deleted upstream branch keeps its last remote-tracking ref and reads `+0 -0`, as if up to date; with it `# branch.ab` disappears, which is how a Gone upstream shows. Local branches and the working tree are untouched. Chosen by the maintainer.

## What every git call carries

`git::run` is the one place git is started, so these hold for status and fetch alike, and for whatever comes next.

- **`-c core.hooksPath=<a path under the just-fetch executable>`**: fetch runs hooks a clone configures (`reference-transaction` fires on every ref update, `pre-auto-gc` before auto maintenance). The path sits under the executable, which is a file, so no folder can exist there and no hook can be planted in it; a temp folder could be. It must be absolute: git reads a relative hooksPath from inside the clone. Where the executable's own path is unknown, the call fails for that clone rather than run without the guard. Chosen by the maintainer, the same call as `core.fsmonitor` in [reading-clone-state](reading-clone-state.md).
- **Auth settings are left as they are.** `core.sshCommand` and `credential.helper` also run commands, but they are how the user's fetch authenticates; a fetch through just-fetch runs them exactly as a `git fetch` typed in the clone would. Chosen by the maintainer. Not yet looked at: `remote.<name>.uploadpack`, `core.askPass`, `fetch.recurseSubmodules`.
- **`GIT_TERMINAL_PROMPT=0` and an empty stdin**: a fetch that needs a username or password fails with git's message instead of waiting on a prompt nobody sees. No test can observe this: a test has no terminal, so git never prompts there either way. SSH prompts (an unknown host key, a key passphrase) read the terminal directly and are not stopped by it.

## Doing several at once

Eight clones fetch at a time (`WORKERS`), each worker taking the next clone until none are left; results keep the scan's order. The test that proves it gives four clones an upload-pack that sleeps two seconds and requires all four to finish in under 6 s; one at a time takes at least 8. Measured on 58 real clones: 7.2 s with eight workers, 42.1 s with one.

## Showing it

A git message can run to several lines (`fatal: …` then `fatal: Could not read from remote repository.` then advice); the row joins them with `; ` so every clone stays on one line.
