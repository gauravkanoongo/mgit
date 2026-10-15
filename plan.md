# mgit Enhancement & Modernization Plan

> **Philosophy**: *Zero ambiguity, maximum simplicity, ultra-low mental overhead, high performance.*

---

## 1. Disambiguation Strategy: The Single-Rule Approach

### The User-Proposed Rule: "Last `--` Delimiter"

#### Rule Definition:
- If **no `--`** is passed on the command line:  
  **All** arguments are passed directly to `git`. `mgit` acts with default settings.
- If **one or more `--`** is passed:
  - Find the **last `--`** in the argument list.
  - Everything **before the last `--`** (including any earlier `--` symbols and their parameters) is forwarded **directly and untouched to `git`**.
  - Everything **after the last `--`** belongs exclusively to **`mgit`** (e.g. `--jobs`, `--quiet`, `--dir`, etc.).

#### Verification & Analysis of this Rule:

1. **Is it 100% Unambiguous?**
   - **Yes.** By pinning `mgit` arguments strictly behind the *final* `--`, there is mathematically zero ambiguity. Even if `git` uses `--` (for instance, `git checkout main -- file.txt`), that first `--` is safely forwarded to `git` as part of the git payload:
     ```bash
     mgit checkout main -- file.txt -- --quiet --jobs 4
     #                            ^                  ^
     #                forwarded to git            last '--': mgit args start here
     ```
   - Normal git invocations with no `mgit` options require zero extra punctuation:
     ```bash
     mgit status
     mgit pull --rebase --autostash
     mgit log -n 5 --oneline
     ```
   - Invocations requiring `mgit` flags:
     ```bash
     mgit status -- -q
     mgit fetch --all -- --jobs 16 --dir /projects
     mgit diff --cached --stat -- --only-changes
     ```

2. **Is it Simple & Ergonomic?**
   - **Simplicity:** The mental model is trivial: *"Type your normal git command. If you want mgit options, tack on `--` at the very end followed by your mgit flags."*
   - **Parsing Speed:** Can be split with a single `rposition(|arg| arg == "--")` call in standard Rust without needing complex or heavy CLI parsers for the git portion.
   - **No Flag Collisions:** `git` has hundreds of flags (`-C`, `-p`, `-q`, `-v`, `--depth`). Because `mgit` does not inspect flags before the last `--`, `mgit` never mistakenly swallows a git flag or vice-versa.

---

## 2. Technical Stack Evaluation: CLI, Threading & Concurrency

### A. CLI Argument Parsing
- **Git portion:** Raw `Vec<String>` slice (0 to `last_dash_dash_idx`). Zero parsing needed, preserving exact user tokens and quotes.
- **Mgit portion:** Parsed with either:
  1. **`clap` (v4):** Rich `--help`, auto-generated error messages, type validation for numbers/paths.
  2. **`lexopt` or custom lightweight parser:** Near zero binary size footprint, ~0ms compile time.
  - *Recommendation:* `clap` (derive) or `lexopt`. Since `mgit` flags are small and self-contained, `lexopt` or `clap` both offer clean DX.

### B. Concurrency & Parallel Execution Models
Currently, `mgit` uses `threadpool` (v1.8.1) + OS threads + `std::process::Command`.

| Model | Library | Pros | Cons | Recommendation |
| :--- | :--- | :--- | :--- | :--- |
| **Worker Thread Pool (Current)** | `threadpool` | Already present, simple. | Outdated crate, fixed thread stacks, hard to cancel cleanly. | Deprecate in favor of Rayon or Tokio. |
| **Data Parallelism** | [`rayon`](https://docs.rs/rayon) | Idiomatic in Rust, work-stealing scheduler, minimal boilerplate (`par_iter`), lightweight. | Synchronous/blocking process spawning; high job counts block worker threads. | Excellent for CPU/traversal, but suboptimal if 100+ concurrent network git calls are running. |
| **Async Process I/O** | [`tokio`](https://docs.rs/tokio) + `tokio::process` | Non-blocking OS subprocess handles; can run hundreds of concurrent `fetch`/`pull` operations using very few OS threads. Clean Ctrl+C handling & timeouts. | Adds async runtime dependency (`tokio`). | **Best for scalability** (e.g. fetching 200 repos simultaneously without spawning 200 OS threads). |
| **Lightweight Async Channels** | `smol` / `async-process` | Lighter than full Tokio, highly efficient async process spawning. | Smaller ecosystem than Tokio. | Solid alternative to Tokio. |

*Verdict:* 
- For an I/O and process-heavy tool like `mgit` where network calls (`fetch`, `pull`, `clone`) dominate execution time, an **async process runner (`tokio::process` or `async-process`)** or **bounded Rayon threadpool** prevents thread exhaustion.

---

## 3. Directory Traversal & Discovery Architecture

### A. Fast, Pruned Walk
- Use [`walkdir`](https://docs.rs/walkdir) or [`ignore`](https://docs.rs/ignore).
- **Pruning rule (`filter_entry`):**
  - Never recurse into `.git/`, `node_modules/`, `target/`, `dist/`, `.cargo/`, `vendor/`.
  - When `.git` is found, record the parent directory as a repository root and **do not** descend into that `.git` folder.
- **Git Worktrees & Submodules:**
  - A `.git` entry can be a **directory** (standard git repo) OR a **file** containing `gitdir: <path>` (git worktree or git submodule).
  - Both must be matched: `entry.file_name() == ".git"`.

### B. Streaming Pipeline
Instead of the current two-phase approach (walk everything first -> block -> then join):
- Run repo discovery and task execution concurrently via channels.
- As soon as repo #1 is found, execution begins immediately while traversal continues in the background.

---

## 4. Output, UX & Terminal Polish

### A. Unified Stream & Formatting
- **Standardized output:** Output both stdout and stderr sequentially into the per-repo report block.
- **No scrambled output:** Flush entire per-repo report atomically to `stdout` to avoid interleaving between concurrent tasks.
- **Color auto-detection:** Respect `NO_COLOR`, `TERM=dumb`, or piped redirection using `ColorChoice::Auto` or [`anstream`](https://docs.rs/anstream) / [`owo-colors`](https://docs.rs/owo-colors).

### B. Output Modes (Configurable after last `--`)
- `--quiet` / `-q`: Only display repos that produced output, warnings, or errors.
- `--only-changes`: Only display repos where git command output was non-empty (great for `mgit status -s -- --only-changes`).
- `--fail-fast`: Abort remaining queued commands on the first error.

### C. Live Progress & Status
- Display dynamic progress counter: `[24/85 repos completed...]` using [`indicatif`](https://docs.rs/indicatif) (disabled when stdout is redirected or `--quiet` is enabled).
- **Exit Code:** Return code `1` if any repository fails or errors, enabling scripting and CI/CD pipelines.

---

## 5. Rust Edition & Toolchain Modernization

### Rust Edition
- Rust releases editions every three years: **2015**, **2018**, **2021**, and **2024**.
- There is **no Rust edition 2026** (Rust 2024 is the latest edition, stabilized in Rust 1.85+; Rust edition 2027 will be the next cycle).
- The current toolchain is `rustc 1.99.0` (2026).
- **Action:** Upgrade `Cargo.toml` from `edition = "2018"` to `edition = "2024"`.

### Modern Rust Idioms:
- Remove legacy `extern crate` declarations.
- Replace `unwrap()` / `expect()` with typed errors (`anyhow` / `thiserror`).
- Standardize enum variants: `GitStatus::Success`, `GitStatus::Failure`, `GitStatus::Error`.

---

## 6. Implementation Stages & Checklist

- [ ] **Stage 1: Argument Separator Engine**
  - Implement splitting at the last `--`.
  - Validate forwarding with unit tests (e.g. `mgit status -- -q`, `mgit checkout main -- file.txt -- -j 8`).
- [ ] **Stage 2: Modernization & Traversal Fixes**
  - Update `Cargo.toml` to `edition = "2024"`.
  - Fix `.git` file detection (worktrees & submodules).
  - Implement directory pruning (`node_modules`, `target`, `.git`).
- [ ] **Stage 3: Process Execution & Concurrency**
  - Implement bounded parallel execution (Rayon or Tokio Process).
  - Add atomic terminal writing with `anstream`/`termcolor`.
  - Ensure correct exit codes.
- [ ] **Stage 4: Automated Testing**
  - Write integration tests using temporary directories with `git init`.
