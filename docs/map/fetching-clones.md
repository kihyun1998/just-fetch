# Fetching clones

How just-fetch brings each clone's view of its remote up to date and says how the clone stands against it: `fetch::fetch_all`, through `git::run`, read back by `status`.

## The flow

Read status → fetch the clones that have an upstream → read status again. The second read is where `# branch.ab +<ahead> -<behind>` gets its numbers against the freshly fetched remote-tracking branch. A clone with no upstream, or one git could not read, is not fetched.

## The fetch call, and why these flags

`git fetch --prune --quiet`, with no remote named.

- **No remote named** fetches the remote the current branch's upstream lives on (git-fetch: "When no remote is specified, by default the origin remote will be used, unless there's an upstream branch configured for the current branch"), so a clone's other remotes are left alone.
- **`--prune`** deletes remote-tracking branches whose branch is gone from the remote. Without it a deleted upstream branch keeps its last remote-tracking ref and reads `+0 -0`, as if up to date; with it `# branch.ab` disappears, which is how a Gone upstream shows. Local branches and the working tree are untouched. Chosen by the maintainer.

## What every git call carries

`git::run` and `git::run_for_exit_code` start every git call except the fast-forward, so these hold for status, fetch and prediction alike. The fast-forward goes through `git::run_with_clone_hooks`, which keeps everything here but the hooks guard ([fast-forwarding](fast-forwarding.md)).

- **`-c core.hooksPath=<a path under the just-fetch executable>`**: fetch runs hooks a clone configures (`reference-transaction` fires on every ref update, `pre-auto-gc` before auto maintenance). The path sits under the executable, which is a file, so no folder can exist there and no hook can be planted in it; a temp folder could be. It must be absolute: git reads a relative hooksPath from inside the clone. Where the executable's own path is unknown, the call fails for that clone rather than run without the guard. Chosen by the maintainer, the same call as `core.fsmonitor` in [reading-clone-state](reading-clone-state.md).
- **Auth settings are left as they are.** `core.sshCommand` and `credential.helper` also run commands, but they are how the user's fetch authenticates; a fetch through just-fetch runs them exactly as a `git fetch` typed in the clone would. Chosen by the maintainer.
- **`GIT_TERMINAL_PROMPT=0` and an empty stdin**: a fetch that needs a username or password fails with git's message instead of waiting on a prompt nobody sees. No test can observe this: a test has no terminal, so git never prompts there either way. SSH prompts (an unknown host key, a key passphrase) read the terminal directly and are not stopped by it.

## What else a clone's config can run during a fetch

Each of these was probed with the real binary, with a command planted in a clone's own config, and confirmed to run under a plain `git fetch` first. The maintainer decided each one.

| Key | Runs during a fetch? | Decision |
|---|---|---|
| `fetch.recurseSubmodules` | Yes. With `yes`, or when fetched commits move a submodule pointer, the submodule is fetched too, under its own config. | Blocked: `--no-recurse-submodules`. just-fetch shows no submodule state, so nothing is lost. |
| `remote.<name>.uploadpack` | Yes, when the remote is a local path. For ssh it names the server's program; https does not use it. | Blocked for local remotes only: `--upload-pack=git-upload-pack`. ssh users legitimately point it at the server's upload-pack, so ssh and https keep theirs. |
| `protocol.ext.allow` with an `ext::` URL | Yes, when the clone's own config allows `ext`. The URL itself is the command. | Blocked: `-c protocol.ext.allow=never`. |
| `core.gitProxy` | Yes, on a `git://` remote. | When the clone's own config (local or worktree scope) sets it, that clone is not fetched, and the row says why. A gitProxy in the user's global config still works. |
| `core.askPass` | Yes, whenever the remote asks for credentials. `GIT_TERMINAL_PROMPT=0` does not stop it. | Left alone. It is authentication, like `credential.helper` and `core.sshCommand`. |
| `fetch.all` | It widens a bare `git fetch` to every remote, including ones whose upload-pack is not pinned (reported by review). | The fetch names the upstream's remote, so only that remote is fetched, as [#2](https://github.com/kihyun1998/just-fetch/issues/2) asked. |
| `remote.<name>.vcs` | It picks the remote helper git starts (reported by review). | When the clone's own config sets it for the upstream's remote, that clone is not fetched, the same as `core.gitProxy`. |

"Local" follows git's own rule (`url_is_local_not_ssh` and `transport_get` in git's source): a `file://` URL, a URL with no colon, one whose first `/` comes before its first colon, or, on Windows only, a drive-letter path. That rule is `git::is_local_url`. The remote is read from `branch.<name>.remote` and its URL from `git remote get-url`, which applies `insteadOf`.

**This narrows what a clone's config can run during a fetch; it does not close it.** The authentication settings above are left on purpose, and still run whatever a clone's config names. `core.alternateRefsCommand` (review found it runs during a fetch when the clone's object store names an alternate) is tracked in [#24](https://github.com/kihyun1998/just-fetch/issues/24). A user's own environment wins over these guards too: `GIT_ALLOW_PROTOCOL` beats `-c protocol.ext.allow=never`. That is the user's choice, not the clone's.

**A command-line `-c` does not override every key.** `remote.<name>.uploadpack` and `core.gitProxy` take the *first* value git finds. The clone's own file is read before the command line, so `-c remote.origin.uploadpack=git-upload-pack` still ran the clone's value; git even warns "more than one uploadpack given, using the first". That is why upload-pack is pinned with the fetch flag, and why gitProxy cannot be overridden: with `GIT_PROXY_COMMAND=none`, git tries to start a program named `none` and every `git://` fetch fails.

**A probe can lie about prompts.** An environment may preset `GIT_ASKPASS`, `GIT_TERMINAL_PROMPT` and `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_*` (for example `credential.interactive=false`). With those set, askPass looked as if it never ran. They have to be cleared first, and a plain `git fetch` has to be seen running the command before the probe means anything.

## Doing several at once

Eight clones fetch at a time (`WORKERS`), each worker taking the next clone until none are left; results keep the scan's order. The test that proves it fetches four clones over a fake ssh (`core.sshCommand` set to a script that sleeps two seconds, then runs upload-pack locally) and requires all four to finish in under 6 s; one at a time takes at least 8. A planted upload-pack can no longer slow a local remote, because local remotes now get git's own. Measured on 58 real clones: 7.2 s with eight workers, 42.1 s with one.

## Showing it

A git message can run to several lines (`fatal: …` then `fatal: Could not read from remote repository.` then advice); the row joins them with `; ` so every clone stays on one line.
