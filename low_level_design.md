# mgit Low-Level Design (LLD) Blueprint

> **Document Purpose**: Machine-readable & human-accessible technical specification and low-level architecture for `mgit`. Designed for deterministic implementation by AI coding systems and human engineers.
> **Target Edition**: Rust 2024 (`rustc >= 1.85`)
> **Core Tenet**: Zero argument ambiguity, fully async execution via `tokio`, deterministic ordered output, and first-class AI/agent interoperability.

---

## 1. System Architecture Overview

```
                      +-----------------------------+
                      |   CLI Invocation (argv)     |
                      +-----------------------------+
                                     |
                                     v
                 +---------------------------------------+
                 |       Module: `args::split`          |
                 |  Find LAST '--' in args (rposition)   |
                 +---------------------------------------+
                        /                         \
                       /                           \
                      v                             v
           +--------------------+         +--------------------+
           |  Git Raw Payload   |         |    Mgit Options    |
           |   &args[..last]    |         |   &args[last+1..]  |
           | (Vec<String>)      |         |  (lexopt / clap)   |
           +--------------------+         +--------------------+
                      \                             |
                       \                            v
                        \                 +--------------------+
                         \                |   MgitConfig struct|
                          \               +--------------------+
                           \                        |
                            +-----------+-----------+
                                        |
                                        v
                       +---------------------------------+
                       |    Module: `discovery::find`    |
                       | Fast WalkDir with path pruning  |
                       | Returns: Vec<RepoEntry> (sorted)|
                       +---------------------------------+
                                        |
                                        v
                       +---------------------------------+
                       |     Module: `engine::run`       |
                       | Tokio async worker pool         |
                       | Semaphore-bounded concurrency   |
                       | tokio::process::Command         |
                       +---------------------------------+
                                        |
                                        v
                       +---------------------------------+
                       |     Module: `output::render`    |
                       | In-Order Sequential Streamer    |
                       | Formatter: Text / JSON / Stream |
                       +---------------------------------+
                                        |
                                        v
                       +---------------------------------+
                       | std::process::exit(status_code) |
                       +---------------------------------+
```

---

## 2. Module Specifications & Data Structures

### 2.1. Module: `args` (`src/args.rs`)

Responsible for the 100% unambiguous separation of `git` arguments vs. `mgit` configuration.

#### Function Signature:
```rust
pub fn parse_cli(argv: impl IntoIterator<Item = String>) -> Result<(Vec<String>, MgitConfig), ArgsError>;
```

#### Splitting Logic:
```rust
let args: Vec<String> = argv.into_iter().skip(1).collect();

let (git_args, mgit_args) = match args.iter().rposition(|s| s == "--") {
    Some(idx) => {
        let (git, mgit) = args.split_at(idx);
        (git.to_vec(), mgit[1..].to_vec()) // omit the delimiter itself
    }
    None => (args, Vec::new()), // All arguments forwarded to git
};
```

#### Data Model: `MgitConfig`
```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MgitConfig {
    pub dir: PathBuf,              // Root path to search (default: ".")
    pub jobs: usize,              // Max concurrent workers (default: env(MGIT_PARALLEL) or cpus * 8)
    pub quiet: bool,              // Suppress output for clean/warning-free repos
    pub only_changes: bool,       // Only show repos with non-empty stdout
    pub json: bool,               // Output structured JSON (one array or JSONL)
    pub summary_only: bool,       // Skip per-repo output, only print summary
    pub fail_fast: bool,          // Abort remaining jobs on first non-zero status
    pub max_depth: Option<usize>, // Directory traversal recursion limit
    pub timer: bool,              // Display execution durations
    pub no_banner: bool,          // Suppress the ASCII startup banner
}

impl Default for MgitConfig {
    fn default() -> Self {
        let default_jobs = std::env::var("MGIT_PARALLEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| num_cpus::get() * 8);

        Self {
            dir: PathBuf::from("."),
            jobs: default_jobs,
            quiet: false,
            only_changes: false,
            json: false,
            summary_only: false,
            fail_fast: false,
            max_depth: None,
            timer: false,
            no_banner: false,
        }
    }
}
```

---

### 2.2. Module: `discovery` (`src/discovery.rs`)

Fast repository discovery, respecting nested structures, worktrees, submodules, and pruning large build trees.

#### Pruning Rules:
Do **not** descend into directory names matching:
- `.git` (once detected, record repository and stop descending)
- `node_modules`
- `target`
- `.cargo`
- `dist`
- `build`
- `vendor`
- `.venv` / `venv`

#### Detection Rule:
A directory contains a Git repository if it contains an entry named `.git`:
- **Directory**: Standard repository.
- **File**: Git worktree pointer or submodule pointer (format: `gitdir: <path>`).

#### Deterministic Ordering:
The resulting list of repositories **must be sorted alphabetically by path** to ensure deterministic execution and reproducible AI output.

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepoEntry {
    pub id: usize,          // Deterministic sequence index (0..N)
    pub path: PathBuf,      // Root path of the repo
}

pub fn discover_repos(config: &MgitConfig) -> Result<Vec<RepoEntry>, std::io::Error>;
```

---

### 2.3. Module: `engine` (`src/engine.rs`)

Asynchronous concurrent process executor utilizing Tokio.

#### Data Models:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    Success,  // Git exited with status 0
    Failure,  // Git exited with status > 0
    Error,    // Failed to spawn or OS/IO error
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoResult {
    pub id: usize,
    pub repo: String,         // Path as string (normalized)
    pub status: RepoStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub has_changes: bool,    // Convenience: !stdout.trim().is_empty()
    pub elapsed_ms: u64,
}
```

#### Concurrency & Execution Control:
- Use `tokio::sync::Semaphore` initialized with `config.jobs`.
- Use `tokio_util::sync::CancellationToken` for clean `--fail-fast` and `SIGINT` (Ctrl+C) handling.
- Concurrently execute via `tokio::process::Command`:
  - `stdin(Stdio::null())`
  - `stdout(Stdio::piped())`
  - `stderr(Stdio::piped())`

```rust
pub async fn execute_repo(
    entry: RepoEntry,
    git_args: &[String],
    cancel_token: CancellationToken,
) -> RepoResult;
```

---

### 2.4. Module: `output` (`src/output.rs`)

Guarantees **in-order sequential streaming**. Even though repositories finish out-of-order in the async runtime, output is emitted in strict sequential order `0, 1, 2, ...`.

Human-readable terminal output uses normal-weight text and ordinary ordered writes. The banner
starts without a leading blank line, and the output header is followed by a separator before
repository results. Status markers use colored circles consistently: blue for the command header,
green for success, orange for a Git failure/warning, and red for an execution error. Each
repository marker is followed by a space before the repository path.

During discovery, interactive non-JSON runs show a dotted repository-count status on stderr.
The spinner redraws that status in place and leaves a final completion line visible. It does not
write to stdout, so it cannot interleave with or contaminate command output or JSON. When discovery
finishes, repository commands run in the existing deterministic sorted order and their results are
rendered in that order as soon as the next expected result is available.

#### In-Order Streamer Algorithm:
```rust
use tokio::sync::mpsc;
use std::collections::HashMap;

pub async fn stream_in_order(
    total: usize,
    mut rx: mpsc::Receiver<RepoResult>,
    config: &MgitConfig,
    formatter: &dyn OutputFormatter,
) -> SummaryStats {
    let mut pending = HashMap::<usize, RepoResult>::new();
    let mut next_expected_id = 0;
    let mut stats = SummaryStats::default();

    while next_expected_id < total {
        // Check if the next expected result is already buffered
        if let Some(result) = pending.remove(&next_expected_id) {
            stats.record(&result);
            formatter.render_repo(&result, config);
            next_expected_id += 1;
            continue;
        }

        // Wait for incoming results from workers
        match rx.recv().await {
            Some(result) => {
                if result.id == next_expected_id {
                    stats.record(&result);
                    formatter.render_repo(&result, config);
                    next_expected_id += 1;
                } else {
                    pending.insert(result.id, result);
                }
            }
            None => break, // All workers finished
        }
    }
    stats
}
```

#### Output Formats:

##### 1. Human-Readable Terminal Formatter:
- Follows existing styling: Colored path header (Green on success, Yellow on failure, Red on error).
- Standardizes stdout/stderr flush per repo atomically.
- Respects `--quiet` (skips rendering clean repos) and `--only-changes` (skips repos with empty stdout).
- Automatic color suppression on `NO_COLOR=1` or non-TTY pipes via `anstream` / `supports-color`.

##### 2. AI / Machine-Readable JSON Formatter (`--json`):
Outputs a top-level JSON document containing execution metadata and individual repo results:
```json
{
  "command": ["status", "-s"],
  "total_repos": 2,
  "summary": {
    "success": 2,
    "failure": 0,
    "error": 0,
    "elapsed_ms": 142
  },
  "results": [
    {
      "repo": "crates/core",
      "status": "success",
      "exit_code": 0,
      "stdout": " M src/lib.rs\n",
      "stderr": "",
      "has_changes": true,
      "elapsed_ms": 78
    },
    {
      "repo": "crates/cli",
      "status": "success",
      "exit_code": 0,
      "stdout": "",
      "stderr": "",
      "has_changes": false,
      "elapsed_ms": 64
    }
  ]
}
```

---

## 3. Exit Code Contract

| Condition | Exit Code | Rationale |
| :--- | :---: | :--- |
| All repos succeeded (`status == Success` for all) | `0` | Standard Unix success |
| One or more repos failed (`Failure` or `Error`) | `1` | Machine-detectable failure for CI/CD & AI agents |
| Invalid `mgit` CLI arguments after last `--` | `2` | Usage error |
| Git executable not found on `$PATH` | `127` | Standard command-not-found code |

---

## 4. Dependencies (`Cargo.toml`)

```toml
[package]
name = "mgit"
version = "0.2.0"
edition = "2024"
authors = ["Jan van den Berg <rust@koozz.nl>"]
license = "MIT"
description = "Fast, async parallel git execution across multiple repositories with deterministic ordering."

[dependencies]
# Async Runtime
tokio = { version = "1.43", features = ["macros", "rt-multi-thread", "process", "sync", "time"] }
tokio-util = "0.7"

# Serialization & CLI parsing
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
lexopt = "0.3"          # Lightweight, zero-overhead argument parser for mgit flags

# File system & Utilities
walkdir = "2.5"
num_cpus = "1.16"

# Terminal Output & Colors
anstream = "0.6"        # Cross-platform stream colorizer with NO_COLOR & pipe support
anstyle = "1.0"

[dev-dependencies]
tempfile = "3.17"       # For hermetic integration tests
```

---

## 5. Test Matrix & Verification Specifications

### 5.1. Unit Test Coverage:
1. `args::test_split_last_dash_dash`:
   - `["status"]` -> `git: ["status"]`, `mgit: []`
   - `["checkout", "main", "--", "file.txt"]` -> `git: ["checkout", "main", "--", "file.txt"]`, `mgit: []`
   - `["status", "--", "--quiet", "-j", "4"]` -> `git: ["status"]`, `mgit: ["--quiet", "-j", "4"]`
   - `["checkout", "main", "--", "file.txt", "--", "--json"]` -> `git: ["checkout", "main", "--", "file.txt"]`, `mgit: ["--json"]`
2. `discovery::test_pruning`:
   - Verify walker does not enter `node_modules`, `target`, or `.git` subdirectories.
   - Verify `.git` worktree pointer files are recognized as valid repositories.

### 5.2. Integration Test Coverage:
1. `tests::test_in_order_execution`:
   - Create 5 temporary git repos (`repo_a` through `repo_e`).
   - Run `mgit status`.
   - Verify output lines appear strictly sorted `repo_a` -> `repo_e`.
2. `tests::test_json_schema`:
   - Run with `-- --json`.
   - Deserialize output to `serde_json::Value` and validate schema integrity.
3. `tests::test_exit_codes`:
   - Run a command that fails in one repo (e.g. `mgit checkout non-existent-branch`).
   - Verify process exits with code `1`.
