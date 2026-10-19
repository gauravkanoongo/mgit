# mgit Enhancement & Modernization Plan

> **Philosophy**: *Zero ambiguity, maximum simplicity, fully async performance with in-order deterministic output, and first-class AI-agent & workflow friendliness.*

---

## 1. Disambiguation Strategy: The Single-Rule Approach

### The Rule: "Last `--` Delimiter"

#### Rule Definition:
- If **no `--`** is passed on the command line:  
  **All** arguments are passed directly to `git`. `mgit` acts with default settings.
- If **one or more `--`** is passed:
  - Find the **last `--`** in the argument list.
  - Everything **before the last `--`** (including any earlier `--` symbols and their parameters) is forwarded **directly and untouched to `git`**.
  - Everything **after the last `--`** belongs exclusively to **`mgit`** (e.g. `--jobs`, `--quiet`, `--dir`, `--json`, etc.).

#### Verification:
1. **100% Unambiguous:** Mathematically eliminates flag collisions between `git` and `mgit`.
   ```bash
   # Git with its own '--' works out-of-the-box:
   mgit checkout main -- file.txt -- --quiet --jobs 8
   #                    ^         ^
   #         forwarded to git     last '--': mgit flags
   ```
2. **Simple Mental Model:** *"Run your normal git command. If you need mgit options, append `-- <mgit-options>` at the very end."*

---

## 2. Fully Async Architecture (`tokio`) with Guaranteed In-Order Sequential Output

### The Core Problem:
Running 50+ repositories concurrently under `tokio` finishes tasks out-of-order (a small repo finishes in 5ms, while a large repo fetching remote objects takes 3s).  
If output is emitted as soon as each finishes:
1. Output order changes every run (non-deterministic).
2. Diffs and logs become impossible to compare across runs.
3. Output becomes jarring and difficult for humans and AI agents to track.

### The Solution: Concurrent Async Execution + Deterministic Ordered Streaming
We achieve **maximum parallel speed** while maintaining **strict, deterministic, in-order output**:

```text
Discovery Phase:
[Repo 0: frontend] -> Assigned index 0  (lexicographically sorted or traversal order)
[Repo 1: backend]  -> Assigned index 1
[Repo 2: services] -> Assigned index 2
         │
         ▼
Tokio Execution Pool (bounded by Semaphore / --jobs):
Spawn async tasks for all repos concurrently (tokio::process::Command).
Each worker stores its result in memory or pushes it tagged with its index.
         │
         ▼
In-Order Output Streamer:
Output consumer awaits items in strict index sequence: 0, 1, 2, ...
- If Repo 0 is done: prints Repo 0 immediately.
- If Repo 1 is still running: waits for Repo 1 to finish and prints it before printing Repo 2.
- Repo 2, 3, etc. that finished early wait buffered in memory until their turn.
```

#### Why This Is Superior:
1. **Deterministic Order:** Output order is always identical (e.g. alphabetically sorted by repository path).
2. **Instant Printing:** If Repo 0 finishes quickly, output begins streaming to the terminal immediately without waiting for the entire batch.
3. **No Terminal Scrambling:** Repositories never interleave their stdout/stderr.

---

## 3. Making `mgit` AI-Agent & Automated Workflow Friendly

AI agents (LLMs running in automated coding loops, CI/CD runners, and automation pipelines) have specific needs that standard human-centric CLIs often neglect:

### A. Machine-Readable Structured Output (`--json` / `--jsonl`)
When an AI agent runs `mgit status -s -- --json`, parsing ANSI terminal colors and text tables is brittle and error-prone.
- **Flag:** `--json` (or `--jsonl` for streaming newline-delimited JSON).
- **Format:**
  ```json
  {
    "repo": "services/auth",
    "status": "success",
    "exit_code": 0,
    "stdout": " M src/jwt.rs\n?? tests/mock.rs\n",
    "stderr": "",
    "has_changes": true
  }
  ```
- **Benefit:** AI agents can directly parse the output with 1 line of JSON parsing and take immediate decisions (e.g. staging modified files, rolling back failures).

### B. Context & Token Conservation (`--only-changes` & `--summary-only`)
LLM context windows are precious. Running `mgit status` across 100 repositories where 98 are clean produces hundreds of lines of useless status text that bloats LLM context and costs tokens.
- `--only-changes`: Suppresses clean repositories entirely, outputting only repos that had actual git changes or warnings.
- `--summary-only`: Emits only high-level metrics (`Total: 40, Clean: 38, Modified: 2, Errors: 0`).
- `--compact`: Outputs one-line per repo (e.g., `[services/auth] 2 files modified`).

### C. Reliable Exit Codes & Failure Propagation
- If any repository returns a non-zero exit code or encounters an OS execution error, `mgit` will exit with a **non-zero exit code (`1`)**.
- `--fail-fast`: Immediately cancels remaining running async tasks on the first error encountered, saving agent execution time and CI compute budget.

### D. Clean Stream Separation & ANSI Neutrality
- Respect standard `NO_COLOR=1` environment variable and auto-detect when `stdout` is not a TTY (piped to another tool or agent subprocess).
- When redirected or run under `--json`, all ANSI color codes and cursor movement escapes are cleanly stripped.
- During interactive discovery, show a lightweight dotted repository-count status on stderr and
  leave its completion line visible. Keep it separate from stdout; place a separator before the
  ordered per-repository output. Disable this status for JSON output and when stderr is not a TTY.

---

## 4. Modernizing Directory Traversal & Discovery

### A. Pruning & File-Type Detection
- **Detect Git Worktrees & Submodules:** Match both `.git` directories and `.git` pointer files (`gitdir: ...`).
- **Prune Irrelevant Folders:** Using `walkdir::WalkDir::filter_entry` to skip descending into:
  - `.git`
  - `node_modules`
  - `target`
  - `.cargo`
  - `dist` / `build`
  - `vendor`
- **Result:** Traversal speed increases by 10x–100x on large enterprise workspaces.

---

## 5. Summary of `mgit` Options (Configured after last `--`)

All flags after the final `--` are parsed for `mgit`:

| Option | Flag | Description |
| :--- | :--- | :--- |
| **`--jobs <N>`** | `-j` | Maximum concurrent Git commands and Tokio worker threads (defaults to `MGIT_PARALLEL` or `2`). |
| **`--dir <PATH>`** | `-C` | Root directory to search for git repositories (default: current directory). |
| **`--quiet`** | `-q` | Only print repositories that produced output or errors. |
| **`--only-changes`** | | Only print repositories with non-empty git stdout. |
| **`--json`** | | Emit structured JSON output (ideal for AI agents and scripts). |
| **`--summary-only`** | | Print only the final tally summary. |
| **`--fail-fast`** | | Cancel remaining async tasks immediately upon the first failure. |
| **`--timer`** | | Display per-repository and aggregate execution durations. |
| **`--no-banner`** | | Suppress the ASCII startup banner. |
| **`--max-depth <N>`**| `-d` | Limit directory discovery recursion depth. |
| **`--help`** | `-h` | Display usage and help. |

### Example Usages:

```bash
# Everyday human use (zero mgit flags needed):
mgit status
mgit pull --rebase

# Human use with mgit options:
mgit pull --rebase -- -j 16 -q
mgit status -s -- --only-changes

# AI Agent / CI Workflow use:
mgit status -s -- --json --only-changes
mgit fetch --all -- --fail-fast --summary-only
```

---

## 6. Technical Stack & Rust Edition

- **Rust Edition:** `2024` (supported on your active `rustc 1.99.0`).
- **Runtime:** [`tokio`](https://docs.rs/tokio) (macros, rt-multi-thread, process, sync).
- **Serialization:** [`serde`](https://docs.rs/serde) + [`serde_json`](https://docs.rs/serde_json) for AI/machine output.
- **Directory Traversal:** [`walkdir`](https://docs.rs/walkdir) with custom prefix pruning.
- **Terminal Styling:** [`anstream`](https://docs.rs/anstream) + [`anstyle`](https://docs.rs/anstyle) (modern zero-cost ANSI styling that automatically strips colors when redirected or when `NO_COLOR` is set).

---

## 7. Implementation Roadmap & Steps

- [ ] **Step 1: Argument Separator & Parser Engine**
  - Implement last `--` partition.
  - Implement CLI argument parser for options after the last `--`.
  - Add unit tests validating git vs. mgit argument isolation.
- [ ] **Step 2: Rust 2024 & Dependency Upgrade**
  - Update `Cargo.toml` (`edition = "2024"`, add `tokio`, `serde`, `serde_json`, update `walkdir`).
  - Clean up legacy `extern crate` and deprecations.
- [ ] **Step 3: Fast Pruning Repository Walker**
  - Implement directory pruning for `.git`, `node_modules`, `target`, etc.
  - Add support for `.git` worktrees and submodule files.
  - Sort discovered repository paths for deterministic ordering.
- [ ] **Step 4: Async Tokio Engine with In-Order Sequential Output**
  - Spawn tasks with bounded concurrency using `tokio::sync::Semaphore`.
  - Stream results strictly in original/sorted order.
- [ ] **Step 5: AI-Friendly Output Formatting**
  - Implement `--json`, `--only-changes`, `--summary-only`, and `--fail-fast`.
  - Ensure correct exit codes (`0` for all success, `1` for failures).
- [ ] **Step 6: Comprehensive Automated Integration Tests**
  - Integration tests creating temporary repos with `tempfile` testing both text and JSON outputs, `--` splitting, and failure codes.

---

## 8. Maintained Terminal UX Direction

The maintained terminal UX is deliberately simple and deterministic: write repository output in
repository order, then print the final summary. The current interactive dotted discovery status is
written to stderr, gated on TTY/non-JSON output, and finishes with a visible completion line.
Evaluating a colored dotted/block spinner is a proposed visual enhancement, not a commitment to a
progress bar or TUI over command output. `--timer`, `--no-banner`, and existing formatter styling
remain independent options.

The `v0.1.0` tag is the stable behavior reference; `v0.2.0` is the starting point for this
simplification. The failed progress/status-bar and TUI experiments are closed and retained only
for historical reference. See [branch-retirement.md](./branch-retirement.md) for the comparison,
carry-forward decisions, and branch archival record.

---

## 9. Follow-up Work

### A. Persistent mgit configuration
- [x] Load all current mgit options from optional `~/.config/mgit.toml`.
- [x] Use precedence: built-in defaults < TOML values < environment overrides < explicit mgit
  options after the final `--`; retain `MGIT_PARALLEL` as the environment override for jobs.
- [x] Add `--save-config` to persist only explicitly supplied mgit CLI options, updating only
  those TOML keys and preserving unspecified settings and unrelated sections such as exclusions.
  Saving occurs before discovery and command execution.
- [x] Resolve the config file under `~/.config` using `HOME` or `USERPROFILE`; report invalid
  config with an error.
- [x] Keep Git arguments before the final `--` untouched and independent of TOML parsing.

### B. Repository exclusions
- [ ] Add a multiline exclusion-pattern list as a section of `~/.config/mgit.toml`; do not add a
  standalone `.mgitignore` or other ignore file.
- [ ] Support absolute and discovery-root-relative paths, glob/wildcard patterns (including
  recursive `**/*`-style patterns), and both `/` and `\` separators.
- [ ] Normalize paths before matching, prune excluded repository directories during discovery,
  and define/test case sensitivity and glob edge cases across Windows and Unix-style paths.

### C. CLI help and quiet behavior
- [x] Explain in help that standalone `-h`/`--help` and `-V`/`--version` are mgit exceptions
  before the final-delimiter forwarding rule, and that invocation without Git arguments is an
  mgit usage error.
- [ ] Preserve the existing `--quiet` behavior for this work; revisit its meaning only in a
  separate change with explicit behavior decisions and tests.

### D. Discovery indicator
- [x] Replace the plain dotted indicator with a colored rotating dotted marker, preserving TTY
  gating, stderr-only progress, and JSON silence.
- [ ] Manually validate terminal rendering on macOS and Linux.
