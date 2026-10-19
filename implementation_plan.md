# mgit Phased Implementation & 100% Test Coverage Plan

> **Goal**: Fully implement the architecture specified in [`low_level_design.md`](./low_level_design.md) in order of increasing complexity (simple to complex), ensuring 100% test coverage at each milestone.

---

## Phase Structure Overview

```
Phase 1: Project Setup & Argument Parser (Zero side-effects, pure logic)
   │
   ▼
Phase 2: Discovery Engine & Directory Pruning (Filesystem operations, worktrees)
   │
   ▼
Phase 3: Async In-Order Output & Formatter Engine (Output formatting, JSON serialization)
   │
   ▼
Phase 4: Tokio Async Execution Engine (Subprocess spawning, concurrency control, cancellation)
   │
   ▼
Phase 5: CLI Integration & End-to-End Test Suite (Full pipeline, exit codes, CI harness)
```

---

## Phase 1: Toolchain Modernization & Argument Parser Engine

### Objective:
Upgrade project to Rust Edition 2024 and build the `args` module that implements the "last `--`" splitting rule with pure unit-testable logic.

### 1.1 Tasks:
1. Update [`Cargo.toml`](./Cargo.toml) to `edition = "2024"`.
2. Add dependencies: `tokio`, `tokio-util`, `serde`, `serde_json`, `lexopt`, `walkdir`, `num_cpus`, `anstream`, `anstyle`, `tempfile`.
3. Create `src/args.rs`:
   - `MgitConfig` struct with full `Default` implementation and accessors.
   - `parse_cli(argv: impl IntoIterator<Item = String>) -> Result<(Vec<String>, MgitConfig), ArgsError>`.
   - Parsing of `--dir` / `-C`, `--jobs` / `-j`, `--quiet` / `-q`, `--only-changes`, `--json`, `--summary-only`, `--fail-fast`, `--max-depth` / `-d`, and `--help` / `-h`.
4. Export module in `src/main.rs` (or `src/lib.rs`).

### 1.2 100% Test Coverage Suite (`src/args.rs` unit tests):
- [ ] `test_no_arguments_returns_error`: Empty input generates usage error.
- [ ] `test_pure_git_command_no_separator`: `mgit status` -> `git: ["status"]`, `config: default`.
- [ ] `test_git_command_with_internal_separator`: `mgit checkout main -- file.txt` -> `git: ["checkout", "main", "--", "file.txt"]`, `config: default`.
- [ ] `test_separator_with_all_mgit_flags`:
  `mgit status -- -C /tmp -j 12 -q --only-changes --json --summary-only --fail-fast -d 3`
  -> verifies all fields on `MgitConfig` match.
- [ ] `test_multiple_separators_last_takes_precedence`:
  `mgit log --oneline -- file.txt -- -j 4 -- -q`
  -> `git: ["log", "--oneline", "--", "file.txt", "--", "-j", "4"]`, `config.quiet: true`.
- [ ] `test_invalid_mgit_flag_returns_error`: Unknown flags (e.g. `--invalid-flag`) return `ArgsError::UnknownOption`.
- [ ] `test_invalid_jobs_parameter_returns_error`: Non-numeric `-j abc` returns descriptive error.
- [ ] `test_help_flag_triggers_help_signal`: `-h` or `--help` sets or returns the help request.
- [ ] `test_mgit_parallel_env_fallback`: When `-j` is not passed, reads `MGIT_PARALLEL`; falls back to `num_cpus * 8` if unset or invalid.

---

## Phase 2: Repository Discovery Engine & Pruning

### Objective:
Implement `src/discovery.rs` to fast-scan directory trees, prune irrelevant directories, detect worktrees/submodules, and return deterministically sorted `RepoEntry` records.

### 2.1 Tasks:
1. Create `src/discovery.rs`.
2. Define `RepoEntry { pub id: usize, pub path: PathBuf }`.
3. Implement `discover_repos(config: &MgitConfig) -> Result<Vec<RepoEntry>, std::io::Error>`:
   - Use `WalkDir::new(&config.dir).min_depth(1)`.
   - If `config.max_depth` is specified, configure `.max_depth()`.
   - Use `.filter_entry` to prune: `.git`, `node_modules`, `target`, `.cargo`, `dist`, `build`, `vendor`, `.venv`.
   - Check if directory contains `.git` (as a directory) OR `.git` (as a file with `gitdir:`).
   - Once a repository root is found, do NOT descend into its subdirectories unless configured.
   - Sort paths lexicographically and assign sequential `id` from `0..N`.

### 2.2 100% Test Coverage Suite (`src/discovery.rs` hermetic unit tests via `tempfile`):
- [ ] `test_discover_empty_directory`: Returns empty vector when no repos present.
- [ ] `test_discover_single_standard_git_repo`: Finds directory containing `.git` folder.
- [ ] `test_discover_worktree_submodule_git_file`: Finds directory containing `.git` regular file.
- [ ] `test_prune_ignored_directories`:
  Creates dummy repos inside `node_modules/bad_repo` and `target/debug/bad_repo` and asserts they are **ignored**.
- [ ] `test_nested_repos_handling`: Ensures parent `.git` doesn't infinitely recurse into internal `.git`.
- [ ] `test_deterministic_path_sorting`: Creates repos `repo_c`, `repo_a`, `repo_b` and verifies returned IDs and paths are strictly sorted `repo_a (0)`, `repo_b (1)`, `repo_c (2)`.
- [ ] `test_max_depth_limiting`: Repos beyond `max_depth` are not included in discovery.
- [ ] `test_symlink_and_junction_handling`: Validates loop-safe traversal.

---

## Phase 3: In-Order Output Streaming & Formatters

### Objective:
Implement `src/output.rs` with the in-order buffering algorithm and two formatters: Terminal/Human Formatter (with ANSI support) and AI/JSON Formatter.

### 3.1 Tasks:
1. Create `src/output.rs`.
2. Define `RepoStatus` (`Success`, `Failure`, `Error`) and `RepoResult`.
3. Implement `SummaryStats` with accumulators (`success`, `failure`, `error`, `elapsed_ms`).
4. Implement `OutputFormatter` trait:
   - `render_header(&self, command: &[String], count: usize)`
   - `render_repo(&self, result: &RepoResult, config: &MgitConfig)`
   - `render_summary(&self, stats: &SummaryStats, config: &MgitConfig)`
5. Implement `TerminalFormatter` (using `anstream` / `anstyle` for terminal output with `NO_COLOR` support).
6. Implement `JsonFormatter` (serializing structured JSON with all per-repo fields).
7. Implement `stream_in_order` buffer algorithm:
   - Receives results from `tokio::sync::mpsc::Receiver<RepoResult>`.
   - Stores out-of-order results in a `HashMap<usize, RepoResult>`.
   - Emits strictly by expected `next_id: 0, 1, 2, ...`.

### 3.2 100% Test Coverage Suite (`src/output.rs` tests):
- [ ] `test_in_order_streaming_out_of_order_arrival`:
  Feed results in order `[2, 0, 1]`; assert output stream prints `[0, 1, 2]`.
- [ ] `test_terminal_formatter_quiet_mode`:
  Verify clean success repo prints nothing when `--quiet` is true.
- [ ] `test_terminal_formatter_only_changes_mode`:
  Verify clean repo with empty stdout prints nothing; repo with modified files prints.
- [ ] `test_terminal_formatter_failure_and_error`:
  Verify yellow/red warning headers and error messages print correctly.
- [ ] `test_json_formatter_valid_schema`:
  Validate generated JSON deserializes cleanly into structured schema and includes timing, exit codes, and diffs.
- [ ] `test_summary_only_mode`:
  Verify per-repo outputs are suppressed and only the summary totals are output.

---

## Phase 4: Async Process Execution Engine (`tokio`)

### Objective:
Implement `src/engine.rs` to run `git` subprocesses concurrently across repos using Tokio, bounded by a semaphore, with clean cancellation support.

### 4.1 Tasks:
1. Create `src/engine.rs`.
2. Implement `execute_repo(entry: RepoEntry, git_args: &[String], cancel_token: CancellationToken) -> RepoResult`:
   - Spawns `tokio::process::Command::new("git")`.
   - Sets working directory to `entry.path`.
   - Non-blocking wait for stdout/stderr and exit status.
   - Measures `elapsed_ms`.
   - Returns structured `RepoResult`.
3. Implement `run_engine(repos: Vec<RepoEntry>, git_args: Vec<String>, config: MgitConfig, tx: mpsc::Sender<RepoResult>)`:
   - Creates `Arc<Semaphore>` sized to `config.jobs`.
   - If `config.fail_fast` is true, trigger `cancel_token.cancel()` when any job returns `Failure` or `Error`.
   - Spawns Tokio tasks with `tokio::spawn`.

### 4.2 100% Test Coverage Suite (`src/engine.rs` tests):
- [ ] `test_execute_git_status_success`: Runs `git status` on a temp git repo and asserts `RepoStatus::Success`.
- [ ] `test_execute_git_failure_status`: Runs invalid git command (e.g. `checkout invalid-branch-xyz`) and asserts `RepoStatus::Failure` with non-zero exit code.
- [ ] `test_execute_nonexistent_binary_error`: Asserts `RepoStatus::Error` when process fails to spawn.
- [ ] `test_semaphore_concurrency_bounding`: Verify number of active workers never exceeds `config.jobs`.
- [ ] `test_fail_fast_cancellation`: Verify subsequent pending tasks are cancelled when `--fail-fast` is active and a task fails.

---

## Phase 5: Pipeline Integration, Exit Codes & End-to-End Tests

### Objective:
Wire up `main.rs`, verify exit codes, and write end-to-end integration tests that run the compiled binary on temporary git workspaces.

### 5.1 Tasks:
1. Refactor `src/main.rs`:
   - Parse CLI via `args::parse_cli`.
   - Print help and exit if `-h` was requested.
   - Run `discovery::discover_repos`.
   - Spawn Tokio runtime and launch `engine::run_engine` + `output::stream_in_order`.
   - Exit with code `0` if all succeed; exit with `1` if failures/errors occurred; exit with `2` on bad args.
2. Create `tests/e2e_tests.rs`:
   - End-to-end integration testing using `std::process::Command` against `mgit`.

### 5.2 100% Test Coverage Suite (`tests/e2e_tests.rs`):
- [ ] `test_e2e_normal_git_status`: Runs `mgit status` across multiple temp repos; asserts exit code 0.
- [ ] `test_e2e_separator_syntax`: Runs `mgit status -- -q` and `mgit status -- --json`.
- [ ] `test_e2e_json_output_machine_readable`: Captures stdout and asserts valid JSON containing all repositories.
- [ ] `test_e2e_failure_exit_code`: Runs failing command and asserts exit code 1.
- [ ] `test_e2e_only_changes_filtering`: Only modified repository is printed.
- [ ] `test_e2e_no_color_environment`: Asserts no ANSI codes present when `NO_COLOR=1`.

---

## Follow-up: Configuration, Exclusions, Help, and Discovery UX

Config loading, partial config saving, help clarification, and the colored dotted discovery
indicator are implemented. Repository exclusions remain planned. Preserve the current `--quiet`
behavior.

### A. Configuration and repository exclusions
- [x] Load mgit settings from `~/.config/mgit.toml`, covering all mgit options (directory,
  concurrency, output modes, traversal depth, and related flags). Define and test precedence:
  built-in defaults, config file, environment overrides, then explicit CLI options.
- [x] Add `--save-config` to persist only mgit settings explicitly supplied on this invocation.
  Update those keys in `~/.config/mgit.toml` while preserving unspecified existing values and
- [x] Update only explicitly supplied keys and preserve unspecified values and unrelated
  sections; save before discovery/command execution and test partial updates.
- [ ] Add a repository-exclusion section to that same TOML file; do not add a separate ignore
  file. Support a multiline list of patterns, absolute and relative paths, glob wildcards
  (including `**/*`-style patterns), and both `/` and `\` path separators.
- [ ] Define path normalization and matching: relative patterns are evaluated relative to the
  discovery root, absolute patterns against normalized absolute repository paths; excluded
  repository directories are skipped during discovery. Specify glob edge cases before
  implementation and test matching on Windows and Unix-style paths.
- [ ] Test config discovery, missing/default config behavior, malformed TOML diagnostics,
  precedence, and excluded repository pruning without changing the argument-routing contract.

### B. Help and argument behavior
- [x] Update `--help` to explain that arguments before the final `--` go to Git, mgit options
  after the final `--` configure mgit, and standalone `-h`/`--help` and `-V`/`--version`
  requests are handled by mgit as exceptions. Document that no Git arguments is an mgit usage
  error. Keep examples aligned with actual parser behavior.

### C. Discovery indicator evaluation
- [x] Replace the plain dotted discovery indicator with a colored rotating dotted marker while
  preserving TTY/JSON gating and stderr-only output.
- [ ] Manually validate Unicode/color rendering and terminal width on macOS and Linux.

### D. Quiet behavior
- [ ] Leave `--quiet` semantics unchanged in this follow-up; any future behavioral redesign
  requires a separate decision and tests.
