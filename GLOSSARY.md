# just-fetch

Finds which of my local clones are out of step with their remotes, and lets me pick which ones to fetch.

## Language

**Clone**:
A local git repository found under the scan root.
_Avoid_: Repo (ambiguous with the GitHub-side repository), project

**Scan root**:
The folder I point just-fetch at: either a clone itself, or a folder holding clones.
_Avoid_: Workspace, base dir

## Clone status

**Up to date**:
The clone and its remote have the same commits on the upstream branch, as of the fetch just made.
_Avoid_: Synced, clean (clean is about the working tree)

**Behind**:
The remote has commits the clone lacks, and the clone has none the remote lacks, so it can be fast-forwarded.
_Avoid_: Outdated, stale

**Ahead**:
The clone has commits the remote lacks, and the remote has none the clone lacks.

**Diverged**:
Both the clone and the remote have commits the other lacks.
_Avoid_: Conflicted (that is a predicted conflict, a narrower thing)

**Predicted conflict**:
A diverged clone whose trial merge with its remote would leave files in conflict. A forecast only; nothing is changed.
_Avoid_: Conflict, merge conflict

**Dirty**:
A clone with uncommitted changes to tracked files. Untracked files alone do not make a clone dirty. A dirty clone is never pulled.
_Avoid_: Modified, unclean

**No upstream**:
A clone whose current branch tracks no remote branch, so there is nothing to compare it against.

**Gone upstream**:
A clone whose current branch still names an upstream, but that branch no longer exists on the remote.
_Avoid_: Deleted upstream, stale upstream
