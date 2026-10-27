# Predicting conflicts

How just-fetch says whether a diverged clone would merge cleanly with its upstream, without changing anything: `predict::predict`, which `predict::predict_diverged` calls for each clone whose fresh status (`Refreshed::fresh_status`: readable, and its fetch did not fail) is diverged.

## The call

1. In the clone: `git rev-parse HEAD @{upstream}` for the two commits, and `rev-parse --show-object-format --path-format=absolute --git-path objects` for its object format and object store. In a linked worktree `--git-path objects` gives the shared store.
2. A scratch bare repository in the temp folder, made with the same object format, whose `objects/info/alternates` names the clone's object store.
3. In the scratch repository: `git merge-tree --write-tree --name-only --no-messages -z <ours> <theirs>`, through `git::run_for_exit_code`.

- **Exit 0 is a clean merge, exit 1 is a conflict**, anything else means no merge could be made (git-merge-tree, EXIT STATUS). This is why `run_for_exit_code` hands back the code: `run` reads every non-zero code as a failure.
- **`--name-only -z`**: after the tree's id, each conflicted file once, NUL-separated. The parser's fixture was recorded from git 2.50.1; with `--name-only` git lists a file once, not once per stage.
- **The comparison is between commits.** Uncommitted changes in a dirty clone are not part of the prediction.
- **Exit 1 is not proof of a file in conflict.** With the clone's objects unreachable (seen while alternates were passed through an environment variable and split on a `:` in the path), git exited 1 with no names.

## Why a scratch repository

The maintainer asked for two things: no merge driver the clone names may run, and nothing written to the clone. Running `merge-tree` in the clone met neither. It wrote the merged trees and blobs into the clone's object store (measured: three loose objects for a two-file conflict). It also ran any merge driver that the clone's config defines and an attribute names, whether the attribute came from a committed `.gitattributes` or from `.git/info/attributes` (probed both). `--attr-source=<empty tree>` covered only the committed file.

The scratch repository reads none of the clone's config, `info/attributes` or hooks, and it has no working tree, so committed `.gitattributes` are not read either (probed: a driver from the user's own global config, named by a committed attribute, did not run there and did run in the clone). New objects land in the scratch store, which is deleted afterwards. The user's global and system config still apply.

The cost, accepted by the maintainer: no attribute applies, so a file marked `binary` or `merge=union` is predicted as plain text. A prediction can therefore differ from what a real `git merge` would do.

## The git it needs

`merge-tree --write-tree`, `--name-only` and `--no-messages` all arrived in git 2.38 (RelNotes 2.38.0; git-merge-tree at v2.38.0). That is the floor (`MINIMUM_GIT`). Below it, or when `git version` cannot be read, the row says the check is unavailable and every other column still works. `git version` is read once per run, and only when some clone is diverged.

## Showing it

A row names up to five conflicted files (`CONFLICTS_SHOWN`) and counts the rest: `conflict: a, b, c, d, e (+2 more)`. A newline in a file name is shown as `\n`, so the row stays on one line.
