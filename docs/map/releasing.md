# Releasing

How a `v*` tag becomes a GitHub Release: `.github/workflows/release.yml`, modelled on just-shield's release workflow (`docs/agents/thegraph.md`, example). The homebrew-tap and scoop-bucket `auto-update.yml` workflows read what it publishes.

## The jobs

**gate → build → verify → release.** `release` runs only on a tag. A change to the workflow file in a pull request runs the first three as a dry run, so a broken release shows up before anything is tagged.

- **gate** runs fmt, clippy and the tests on windows-latest and macos-latest. On a tag it first checks that the tag is `v` + the version in `Cargo.toml`.
- **build** makes one archive per target: `x86_64-apple-darwin` and `aarch64-apple-darwin` on macos-latest, `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` on windows-latest.
- **verify** unpacks each archive on a runner of its own platform and runs the binary there: macos-latest (arm64), macos-15-intel, windows-latest (x64) and windows-11-arm. Platform labels come from GitHub's runner reference. A binary must print `just-fetch <Cargo.toml version>` for `--version`, list an empty folder, and list the checkout with no `error:` row. Only then is anything published.
- **release** writes `SHA256SUMS` over the four archives and creates the release with generated notes. It is the only job with `contents: write`.

## What the package managers depend on

Changing any of these breaks the tap or the bucket, whose auto-update scripts build URLs and look up hashes from them.

- **Archive names** `just-fetch-v<version>-<target>.tar.gz` (macOS) and `.zip` (Windows). Each extracts to a folder of the same name holding `just-fetch` or `just-fetch.exe`, `README.md`, `LICENSE-MIT` and `LICENSE-APACHE`.
- **`SHA256SUMS`** lines are `<hash>  <file name>`. Both scripts take their hashes from this file rather than downloading the archives again.
- The version in the names comes from `Cargo.toml`, not from the git ref, so a pull-request dry run names its archives the same way a tag run does.

## What it leaves out

- No build-provenance attestation. The maintainer chose `SHA256SUMS` alone, where just-shield attests its archives.
- No Linux targets. just-fetch is for Windows and macOS.
- No CHANGELOG. Each release's notes are generated from its pull requests (`gh release create --generate-notes`).
- The gate runs on windows-latest and macos-latest, not on windows-11-arm, which CI covers on every pull request. The verify job still runs the arm64 Windows binary on windows-11-arm before anything is published.
- No third-party build tool. Like just-shield, the workflow is written by hand, and every action is pinned to a commit.
