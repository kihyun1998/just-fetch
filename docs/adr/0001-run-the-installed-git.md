# Run the installed git rather than a Rust git library

just-fetch runs the `git` already on the machine for fetch, status, pull and trial merges, instead of linking `git2` or `gix`. The tool's whole job is talking to remotes, and the installed git already authenticates the way I have set it up on each machine — Git Credential Manager on Windows, the keychain and SSH agent on macOS — where a library would make just-fetch reimplement that per platform. The cost is a runtime dependency on git, and a floor of git 2.38 for `merge-tree --write-tree`, which predicted conflicts need.
