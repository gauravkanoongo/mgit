# mgit Enhancement & Modernization Plan

> **Philosophy**: *Frictionless simplicity without sacrificing power.*  
> Running `mgit status` should "just work" instantly without ceremony, while advanced options remain clean, discoverable, and intuitive.

---

## 1. Core Challenge: Distinguishing `mgit` vs. `git` Arguments

A common failure mode in multi-repo CLI wrappers is command-line ambiguity: if a user types `mgit -p ...` or `mgit --quiet ...`, does `-p` belong to `mgit` or `git` (e.g. `git -p` for paginate / `git log -p`)?

### Recommended Strategy: Two-Tier Parsing with Standard Conventions

We will support **three frictionless patterns**, giving users zero mental friction for common usage and bulletproof disambiguation when needed.

#### Strategy A: Direct Pass-Through by Default (Zero-Friction Everyday Mode)
If no `mgit`-specific flags are provided, everything is directly forwarded to `git`.
```bash
mgit status
mgit pull --rebase --autostash
mgit log -n 5 --oneline
```
The user never has to think about complex escaping for everyday commands.

#### Strategy B: Standard POSIX `--` Delimiter (Bulletproof Disambiguation)
Everything before `--` belongs to `mgit`; everything after `--` is forwarded strictly to `git`:
```bash
mgit --quiet --jobs 8 -- git status
# or without 'git' prefix:
mgit -q -j 8 -- status -s
mgit --dir /projects -- pull --ff-only
```

#### Strategy C: Smart Prefix / Flag Sniffing (Intuitive Ergonomics)
To avoid forcing the user to type `--` constantly:
1. `mgit` inspects leading arguments.
2. If an argument matches a recognized `mgit` option (e.g., `-j`, `--jobs`, `-q`, `--quiet`, `-C`, `--dir`, `--depth`, `--help`, `-V`), `mgit` consumes it.
3. As soon as the first unrecognized argument or git command is reached (e.g. `status`, `fetch`, `pull`, `checkout`, `branch`), or if `--` is encountered, parsing stops and all subsequent arguments are treated as the payload for `git`.
4. If a flag conflict could exist (e.g. `git -C`), `--` serves as the explicit separator.

#### Summary of CLI Syntax

```text
USAGE:
    mgit [MGIT_OPTIONS] [GIT_SUBCOMMAND] [GIT_OPTIONS]...
    mgit [MGIT_OPTIONS] -- <GIT_COMMAND>...

EXAMPLES:
    mgit status
    mgit -q pull --rebase
    mgit -C /path/to/workspaces -j 16 fetch --all
    mgit --only-changes status -s
    mgit --help
```

---

## 2. Proposed Feature Roadmap

### Phase 1: Robust CLI & Disambiguation Engine
- [ ] Add [`clap`](https://docs.rs/clap) (v4 with `derive` and `trailing_var_arg = true`) or a lightweight zero-dependency custom parser tailored to dual-mode CLI forwarding.
- [ ] Implement essential `mgit` flags:
  - `-C, --dir <PATH>`: Root directory to search from (defaults to `.`).
  - `-j, --jobs <N>`: Thread count / parallel jobs (defaults to `MGIT_PARALLEL` or `cpus * 8`).
  - `-q, --quiet`: Only print repositories that have errors, warnings, or output.
  - `--only-changes`: Only show repos where `git` had non-empty output (ideal for `mgit status -s`).
  - `-d, --max-depth <N>`: Maximum directory depth for repo discovery.
  - `--no-color` / `NO_COLOR` env support: Honor standard terminal color conventions (`ColorChoice::Auto`).
  - `-h, --help` and `-V, --version`: Built-in self-documentation.

### Phase 2: Traversal & Discovery Overhaul
- [ ] **Support Git Worktrees & Submodules**: Detect both `.git` directories and `.git` files (`gitdir: ...`).
- [ ] **Smart Directory Pruning**: Stop traversing into `.git/`, `node_modules/`, `target/`, `.cargo/`, `vendor/`, `build/`. This prevents millions of wasted stat calls and makes traversal orders of magnitude faster on large workspaces.
- [ ] **Symlink Safety**: Avoid recursive loops caused by directory symlinks or Windows NTFS junctions.

### Phase 3: UX, Output & Concurrency
- [ ] **Exit Codes**: Exit with status `1` (or non-zero) if any repository fails or errors, enabling reliable CI/CD and scripting.
- [ ] **Unified Terminal Output**: Streamline stdout and stderr together per repository to prevent scrambled output when piping or redirecting.
- [ ] **Clean Status Summaries**:
  ```text
  [12/12] Finished in 1.4s (10 clean, 2 with changes, 0 errors)
  ```
- [ ] **Modern Concurrency**: Consider migrating from legacy `threadpool` to `rayon` or an async worker model (`tokio` / `async-process`) for smoother throughput and cancellation on Ctrl+C.

### Phase 4: Modernization & Quality Assurance
- [ ] Bump Rust edition to `2021` in `Cargo.toml`.
- [ ] Remove deprecated `extern crate` syntax in `main.rs`.
- [ ] Convert `GitStatus` enum to idiomatic Rust naming (`Success`, `Failure`, `Error`).
- [ ] Replace `unwrap()` / `expect()` with structured error handling.
- [ ] Add unit and integration tests using temporary Git repositories (`tempfile`).
- [ ] Add GitHub Actions CI workflow for cross-platform validation (Windows, macOS, Linux).

---

## 3. Minimal Example of the New CLI Interface

```rust
// Conceptual clap definition for mgit
#[derive(Parser, Debug)]
#[command(name = "mgit", author, version, about = "Run git on multiple projects simultaneously.")]
pub struct Cli {
    /// Directory to search for git repositories
    #[arg(short = 'C', long = "dir", default_value = ".")]
    pub root_dir: PathBuf,

    /// Number of concurrent workers (defaults to MGIT_PARALLEL or cpus * 8)
    #[arg(short = 'j', long = "jobs")]
    pub jobs: Option<usize>,

    /// Only display output for repositories with warnings, errors, or changes
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Only display repositories with non-empty git output
    #[arg(long = "only-changes")]
    pub only_changes: bool,

    /// Maximum search depth for discovering repositories
    #[arg(short = 'd', long = "max-depth")]
    pub max_depth: Option<usize>,

    /// Arguments forwarded to git
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub git_args: Vec<String>,
}
```

---

## 4. Next Steps
1. Review and refine configuration options.
2. Implement Phase 1 (CLI parsing + argument separator).
3. Test locally against workspaces containing multiple repositories, worktrees, and ignored subdirectories.

