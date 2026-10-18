# mgit

[![Version](https://img.shields.io/crates/v/mgit.svg)](https://crates.io/crates/mgit)
[![License](https://img.shields.io/crates/l/mgit.svg)](https://github.com/koozz/mgit/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/mgit.svg)](https://crates.io/crates/mgit)

Fast, asynchronous multi-repository Git CLI utility with guaranteed deterministic in-order output, intelligent directory pruning, and first-class AI-agent workflow support.

---

## Features

- **Fully Asynchronous (`tokio`)**: Runs parallel Git operations across dozens/hundreds of repositories without spawning unbounded OS thread stacks.
- **Strict In-Order Sequential Output**: Concurrently executes tasks, but buffers and streams stdout/stderr in deterministic alphabetical repository order.
- **Unambiguous Argument Model**: All arguments before the **last `--`** go to `git`; options after the **last `--`** configure `mgit`.
- **Fast Directory Pruning**: Avoids traversing into `.git/`, `node_modules/`, `target/`, `.cargo/`, `vendor/`, `build/`, and virtual environments.
- **Worktree & Submodule Support**: Automatically identifies both `.git` directories and `.git` pointer files (`gitdir: ...`).
- **AI-Agent & Scripting Ready (`--json`)**: Emits structured JSON schemas with per-repository exit codes, diffs, changes detection, and execution timings.
- **Human-readable discovery feedback**: Interactive runs show a lightweight repository-count spinner on stderr, retain the completion status, and separate discovery from ordered command output.

---

## Usage

```sh
# Run git commands across all repositories in current workspace:
mgit status
mgit pull --ff-only
mgit pull --rebase --autostash
mgit log -n 3 --oneline

# Commands with internal git '--' work out-of-the-box:
mgit checkout main -- file.txt
```

### Passing Options to `mgit` (After Last `--`)

Any arguments after the **last `--`** configure `mgit`:

```sh
# Only display repositories with changes, warnings, or errors
mgit status -s -- -q

# Only display repositories with non-empty git stdout
mgit status -s -- --only-changes

# Run on a custom directory with 16 parallel workers
mgit fetch --all -- -C /path/to/workspace -j 16

# Limit both concurrent Git commands and Tokio worker threads to 4
mgit status -- -j 4

# Machine-readable JSON output (ideal for AI coding agents & scripts)
mgit status -s -- --json

# Abort remaining tasks on first failure
mgit pull -- --fail-fast

# Include per-repository and total execution timings
mgit status -- --timer

# Hide the startup banner
mgit status -- --no-banner
```

### Available `mgit` Options

| Flag | Option | Description |
| :--- | :--- | :--- |
| `-C` | `--dir <PATH>` | Root directory to search for git repositories (default: `.`) |
| `-j` | `--jobs <N>` | Maximum concurrent Git commands and Tokio worker threads (default: `MGIT_PARALLEL` or `2`) |
| `-q` | `--quiet` | Suppress output for clean repositories |
| | `--only-changes` | Only display repositories that produced non-empty git stdout |
| | `--json` | Output structured JSON report for automation and AI agents |
| | `--summary-only` | Print only the final tally summary |
| | `--fail-fast` | Cancel remaining async operations on first error |
| | `--timer` | Display per-repository and total execution duration |
| | `--no-banner` | Suppress the ASCII startup banner |
| `-d` | `--max-depth <N>` | Limit directory traversal depth |
| `-h` | `--help` | Display usage and help |
| `-V` | `--version` | Display the build's semantic version |

Interactive runs show discovery progress only when stderr is a terminal and JSON output is not
selected. The dotted status line reports the number of repositories found and remains visible
after discovery completes. A separator precedes repository results; results retain the existing
deterministic order. Progress is written to stderr, leaving stdout and JSON output free of status
text.

`-j N` (or `--jobs N`) sets both the maximum number of concurrent Git commands and Tokio's
asynchronous worker-thread count. The default is 2; `MGIT_PARALLEL` can override the default.

---

## Building Locally

Every build reports a full SemVer version through `mgit --version`. The version
contains the 40-character Git commit SHA as SemVer build metadata. A dirty local
working tree adds `.dirty`; a local commit after the latest release gets a
`-dev.N` prerelease identifier. `Cargo.toml` supplies the starting version before
the first release tag. After that, local builds derive the next development
version from the latest stable tag and Conventional Commits.

### Local builds

Build and inspect the current local development version:

```sh
cargo build
cargo run -- --version
```

For an optimized local build:

```sh
cargo build --release
cargo run --release -- --version
```

The executable is `target/release/mgit` on Unix and
`target\release\mgit.exe` on Windows.

At a clean stable release tag, `cargo build --release` reports the same version
as the release binary, including the tag commit SHA. To locally build a specific
SemVer prerelease (the build script adds the current SHA automatically):

```sh
# Build the exact source revision used for a published stable release
git checkout "v1.5.0+<full-git-sha>"
cargo build --release
cargo run --release -- --version

# PowerShell
$env:MGIT_BUILD_VERSION = "1.5.1-nightly.20261017T203000Z"
cargo run --release -- --version
Remove-Item Env:MGIT_BUILD_VERSION

# Bash
MGIT_BUILD_VERSION=1.5.1-nightly.20261017T203000Z cargo run --release -- --version
```

`MGIT_BUILD_VERSION` is a base version without `+build-metadata`; the build
script always appends the current commit SHA and, when needed, `.dirty`.

Install the current local build into the Cargo binary directory:

```sh
cargo install --path .
```

### GitHub Actions builds and releases

The single workflow at `.github/workflows/release.yml` supports automatic
releases and three manual build types:

- **Automatic stable releases:** pushes to the repository's default branch are
  inspected since the latest stable `vX.Y.Z` tag. A `fix:` or `perf:` commit
  requests a patch bump, `feat:` requests a minor bump, and a breaking change
  requests a major bump. When one or more recognized commits are found, the
  workflow builds all targets and publishes a GitHub release. Other commits do
  not trigger a release build.
- **Manual builds:** in GitHub, choose **Actions → Build and Release → Run
  workflow**. Select `local` to build downloadable artifacts without publishing
  a GitHub release, `nightly` to publish a timestamped prerelease, or `release`
  to publish a stable release. For a manual stable release, `auto` uses the
  Conventional Commit messages since the last stable tag; selecting `patch`,
  `minor`, or `major` explicitly overrides that calculation.

The same manual runs can be started with GitHub CLI from the branch to build:

```sh
# Build artifacts only (no tag or GitHub release)
gh workflow run release.yml --ref main -f build_type=local -f bump=auto

# Build and publish a timestamped nightly prerelease
gh workflow run release.yml --ref main -f build_type=nightly -f bump=auto

# Calculate a stable release from Conventional Commits
gh workflow run release.yml --ref main -f build_type=release -f bump=auto

# Explicitly choose the SemVer component to increment
gh workflow run release.yml --ref main -f build_type=release -f bump=minor
```

Replace `main` with the repository's default branch. The `local` run's
platform binaries are available as workflow artifacts from its Actions run.
Nightly and stable runs publish assets as GitHub releases. To trigger automatic
stable release detection, push a Conventional Commit to the default branch, for
example `git commit -m "fix: handle empty repository list"` followed by
`git push`.

For example, a stable release from `1.4.2` produces:

| Change | Commit example | Next version |
| --- | --- | --- |
| Patch | `fix: handle empty repository list` | `1.4.3+<full-git-sha>` |
| Minor | `feat: add JSON summary output` | `1.5.0+<full-git-sha>` |
| Major | `feat!: change the command-line output schema` | `2.0.0+<full-git-sha>` |

A breaking change can also be declared with a `BREAKING CHANGE:` footer. When
several Conventional Commits land between releases, the highest applicable
bump wins. Nightly builds use the current stable version as their base, for
example `1.4.2-nightly.20261017T203000Z.17+<full-git-sha>`. A local development
build might report `1.4.3-dev.2+<full-git-sha>.dirty`.
The GitHub Actions `local` build is marked as a prerelease version too, such as
`1.4.2-local.20261017T203000Z.18+<full-git-sha>`, but it only uploads workflow
artifacts and does not create a release or tag. The final numeric component is
the unique GitHub Actions run number.

The GitHub release tag itself includes the same SHA build metadata
(`v1.5.0+<full-git-sha>`), making each release uniquely identify its source
commit. Release automation does not require separate release configuration
files or a manual Cargo version bump.

### Running tests

```sh
cargo test
```

---

## Multi-Target OS & Architecture Support

`mgit` is written in pure, portable Rust (using Tokio, WalkDir, Serde, and standard OS process spawning). It has **no C/C++ library dependencies (no OpenSSL, no libgit2, no C toolchain requirements)**, making cross-compilation straightforward across major operating systems and architectures.

### Primary Targets

| Target Triple | OS | Architecture | Status |
| :--- | :--- | :--- | :--- |
| `x86_64-pc-windows-msvc` / `gnu` | Windows | AMD64 (64-bit Intel/AMD) | Tier 1 Supported |
| `aarch64-pc-windows-msvc` | Windows | ARM64 (Surface / Snapdragon) | Tier 1 Supported |
| `x86_64-unknown-linux-musl` / `gnu` | Linux | AMD64 (Standard Linux / CI) | Tier 1 Supported |
| `aarch64-unknown-linux-musl` / `gnu` | Linux | ARM64 (AWS Graviton / Raspberry Pi) | Tier 1 Supported |
| `aarch64-apple-darwin` | macOS | Apple Silicon (M1/M2/M3/M4) | Tier 1 Supported |

### Cross-Compiling with `cross` or `cargo-zigbuild`

Because `mgit` contains only pure Rust dependencies:
```sh
# Install cross
cargo install cross --git https://github.com/cross-rs/cross

# Build for Linux ARM64 (musl - fully static zero-dependency binary):
cross build --target aarch64-unknown-linux-musl --release

# Build for Linux AMD64 (musl - fully static):
cross build --target x86_64-unknown-linux-musl --release

# Build for Windows ARM64:
cargo build --target aarch64-pc-windows-msvc --release
```

---

## License

[MIT licensed](./LICENSE)
