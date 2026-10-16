# mgit

[![Version](https://img.shields.io/crates/v/mgit.svg)](https://crates.io/crates/mgit)
[![License](https://img.shields.io/crates/l/mgit.svg)](https://github.com/koozz/mgit/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/mgit.svg)](https://crates.io/crates/mgit)

Fast, asynchronous multi-repository Git CLI utility with guaranteed deterministic in-order output, intelligent directory pruning, and first-class AI-agent workflow support.

---

## Features

- **Fully Asynchronous (`tokio`)**: Runs parallel Git operations across dozens/hundreds of repositories without spawning unbounded OS thread stacks.
- **Strict In-Order Sequential Output**: Concurrently executes tasks, but buffers and streams stdout/stderr in deterministic alphabetical repository order.
- **Single Interactive Status Line**: In terminals, one coordinated status bar stays below scrolling repository output, which streams line by line in deterministic repository order. The bar clears before the final summary. Non-interactive and JSON output remain unchanged.
- **Unambiguous Argument Model**: All arguments before the **last `--`** go to `git`; options after the **last `--`** configure `mgit`.
- **Fast Directory Pruning**: Avoids traversing into `.git/`, `node_modules/`, `target/`, `.cargo/`, `vendor/`, `build/`, and virtual environments.
- **Worktree & Submodule Support**: Automatically identifies both `.git` directories and `.git` pointer files (`gitdir: ...`).
- **AI-Agent & Scripting Ready (`--json`)**: Emits structured JSON schemas with per-repository exit codes, diffs, changes detection, and execution timings.

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

# Machine-readable JSON output (ideal for AI coding agents & scripts)
mgit status -s -- --json

# Abort remaining tasks on first failure
mgit pull -- --fail-fast
```

### Available `mgit` Options

| Flag | Option | Description |
| :--- | :--- | :--- |
| `-C` | `--dir <PATH>` | Root directory to search for git repositories (default: `.`) |
| `-j` | `--jobs <N>` | Maximum concurrent async workers (default: `MGIT_PARALLEL` or `cpus * 8`) |
| `-q` | `--quiet` | Suppress output for clean repositories |
| | `--only-changes` | Only display repositories that produced non-empty git stdout |
| | `--json` | Output structured JSON report for automation and AI agents |
| | `--summary-only` | Print only the final tally summary |
| | `--fail-fast` | Cancel remaining async operations on first error |
| `-d` | `--max-depth <N>` | Limit directory traversal depth |
| `-h` | `--help` | Display usage and help |

---

## Building Locally

### Release Build

To compile an optimized production release build locally:

```sh
# Standard release build (output in target/release/mgit)
cargo build --release

# Install locally into your cargo binary PATH (~/.cargo/bin or %USERPROFILE%\.cargo\bin):
cargo install --path .
```

### Running Tests

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
