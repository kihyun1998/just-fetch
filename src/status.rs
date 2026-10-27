/// Where a clone's HEAD points.
#[derive(Debug, PartialEq, Eq)]
pub enum Head {
    Branch(String),
    /// A branch with no commits yet.
    Unborn(String),
    Detached,
}

/// What a clone's own working tree and branch look like, without asking the remote.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalState {
    pub head: Head,
    pub upstream: Option<String>,
    pub dirty: bool,
    pub untracked: usize,
}

/// Reads the output of `git status --porcelain=v2 --branch`.
pub fn parse(porcelain: &str) -> LocalState {
    let mut unborn = false;
    let mut branch = None;
    let mut upstream = None;
    let mut dirty = false;
    let mut untracked = 0;
    for line in porcelain.lines() {
        if line == "# branch.oid (initial)" {
            unborn = true;
        } else if let Some(name) = line.strip_prefix("# branch.head ") {
            branch = (name != "(detached)").then(|| name.to_string());
        } else if let Some(name) = line.strip_prefix("# branch.upstream ") {
            upstream = Some(name.to_string());
        } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
            dirty = true;
        } else if line.starts_with("? ") {
            untracked += 1;
        }
    }
    let head = match branch {
        Some(name) if unborn => Head::Unborn(name),
        Some(name) => Head::Branch(name),
        None => Head::Detached,
    };
    LocalState {
        head,
        upstream,
        dirty,
        untracked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_with_upstream_and_clean_tree() {
        let out = "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
                   # branch.head main\n\
                   # branch.upstream origin/main\n\
                   # branch.ab +0 -0\n";
        assert_eq!(
            parse(out),
            LocalState {
                head: Head::Branch("main".into()),
                upstream: Some("origin/main".into()),
                dirty: false,
                untracked: 0,
            }
        );
    }

    #[test]
    fn detached_head_has_no_branch() {
        let out = "# branch.oid 374dd5d54309009b8db27121dcfa58c55c88839a\n\
                   # branch.head (detached)\n";
        assert_eq!(parse(out).head, Head::Detached);
    }

    #[test]
    fn branch_without_commits_is_unborn() {
        let out = "# branch.oid (initial)\n# branch.head main\n";
        assert_eq!(parse(out).head, Head::Unborn("main".into()));
    }

    #[test]
    fn no_upstream_line_means_no_upstream() {
        let out = "# branch.oid (initial)\n# branch.head main\n";
        assert_eq!(parse(out).upstream, None);
    }

    #[test]
    fn changed_tracked_file_makes_dirty() {
        let out = "# branch.oid (initial)\n# branch.head main\n\
                   1 .M N... 100644 100644 100644 e69de29 e69de29 README.md\n";
        assert!(parse(out).dirty);
    }

    #[test]
    fn renamed_and_unmerged_entries_make_dirty() {
        let renamed = "# branch.head main\n\
                       2 R. N... 100644 100644 100644 e69de29 e69de29 R100 new.md\told.md\n";
        let unmerged = "# branch.head main\n\
                        u UU N... 100644 100644 100644 100644 e69de29 e69de29 e69de29 a.md\n";
        assert!(parse(renamed).dirty);
        assert!(parse(unmerged).dirty);
    }

    #[test]
    fn untracked_files_alone_are_counted_but_not_dirty() {
        let out = "# branch.head main\n? notes.txt\n? scratch/\n";
        let state = parse(out);
        assert!(!state.dirty);
        assert_eq!(state.untracked, 2);
    }
}
