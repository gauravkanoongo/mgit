# mgit Implementation Task Tracker (`todo.md`)

> Current Branch: `feature/enhancements-2026`  
> Target: Complete Phase 1 through Phase 5 with 100% test coverage.

---

## Overall Progress

- [x] **Phase 1: Toolchain Modernization & Argument Parser Engine**
- [x] **Phase 2: Repository Discovery Engine & Pruning**
- [x] **Phase 3: In-Order Output Streaming & Formatters**
- [x] **Phase 4: Async Process Execution Engine (`tokio`)**
- [x] **Phase 5: CLI Integration & End-to-End Test Suite**

---

## Detailed Task Breakdown

### Phase 1: Toolchain Modernization & Argument Parser Engine
- [x] Upgrade `Cargo.toml` to `edition = "2024"` and add required dependencies
- [x] Implement `src/args.rs`:
  - [x] `MgitConfig` struct with full defaults (`dir`, `jobs`, `quiet`, `only_changes`, `json`, `summary_only`, `fail_fast`, `max_depth`)
  - [x] `parse_cli` function splitting at the **last `--`** delimiter
  - [x] `lexopt` parser for tokens following the last `--`
- [x] Unit Tests for `src/args.rs`:
  - [x] `test_no_arguments_returns_error`
  - [x] `test_pure_git_command_no_separator`
  - [x] `test_git_command_with_internal_separator`
  - [x] `test_separator_with_all_mgit_flags`
  - [x] `test_multiple_separators_last_takes_precedence`
  - [x] `test_invalid_mgit_flag_returns_error`
  - [x] `test_invalid_jobs_parameter_returns_error`
  - [x] `test_help_flag_triggers_help_signal`
  - [x] `test_mgit_parallel_env_fallback`

---

### Phase 2: Repository Discovery Engine & Pruning
- [x] Implement `src/discovery.rs`:
  - [x] `RepoEntry` struct (`id`, `path`)
  - [x] `discover_repos(&MgitConfig)` with `walkdir`
  - [x] Directory pruning (`.git`, `node_modules`, `target`, `.cargo`, `dist`, `build`, `vendor`, `.venv`)
  - [x] Git worktree & submodule `.git` file pointer support
  - [x] Lexicographical path sorting & deterministic ID assignment (`0..N`)
- [x] Hermetic Unit Tests for `src/discovery.rs` (using `tempfile`):
  - [x] `test_discover_empty_directory`
  - [x] `test_discover_single_standard_git_repo`
  - [x] `test_discover_worktree_submodule_git_file`
  - [x] `test_prune_ignored_directories`
  - [x] `test_nested_repos_handling`
  - [x] `test_deterministic_path_sorting`
  - [x] `test_max_depth_limiting`

---

### Phase 3: In-Order Output Streaming & Formatters
- [x] Implement `src/output.rs`:
  - [x] `RepoStatus` and `RepoResult` structs
  - [x] `SummaryStats` metric accumulator
  - [x] `OutputFormatter` trait
  - [x] `TerminalFormatter` (using `anstream` & `anstyle` with `NO_COLOR` support)
  - [x] `JsonFormatter` (full AI-friendly JSON schema serialization)
  - [x] `stream_in_order` buffered sequential emitting algorithm
- [x] Unit Tests for `src/output.rs`:
  - [x] `test_in_order_streaming_out_of_order_arrival`
  - [x] `test_terminal_formatter_quiet_mode`
  - [x] `test_terminal_formatter_only_changes_mode`
  - [x] `test_terminal_formatter_failure_and_error`
  - [x] `test_json_formatter_valid_schema`
  - [x] `test_summary_only_mode`

---

### Phase 4: Async Process Execution Engine (`tokio`)
- [x] Implement `src/engine.rs`:
  - [x] `execute_repo` async function using `tokio::process::Command`
  - [x] Elapsed execution time measurement per repo
  - [x] Concurrency control with `tokio::sync::Semaphore`
  - [x] Cancellation token propagation for `--fail-fast`
  - [x] `run_engine` coordinator task spawning
- [x] Unit & Mock Tests for `src/engine.rs`:
  - [x] `test_execute_git_status_success`
  - [x] `test_execute_git_failure_status`
  - [x] `test_execute_invalid_directory_error`
  - [x] `test_semaphore_concurrency_bounding`
  - [x] `test_fail_fast_cancellation`

---

### Phase 5: CLI Integration & End-to-End Test Suite
- [x] Refactor `src/main.rs`:
  - [x] Wire together `args`, `discovery`, `engine`, and `output`
  - [x] Implement help text renderer
  - [x] Deterministic exit code logic (`0`, `1`, `2`, `127`)
- [x] Create `tests/e2e_tests.rs`:
  - [x] `test_e2e_normal_git_status`
  - [x] `test_e2e_separator_syntax`
  - [x] `test_e2e_json_output_machine_readable`
  - [x] `test_e2e_failure_exit_code`
  - [x] `test_e2e_only_changes_filtering`
  - [x] `test_e2e_no_color_environment`
- [x] Verify 100% test pass rate with `cargo test`

---

## Dated Follow-up: 2026-10-17

> This section is append-only. The historical branch label and task records above are preserved;
> the active work for this follow-up is on `develop`.

- [x] Compare `v0.1.0`, `v0.2.0`, `feature/enhancements-2026`, and `feature/tui`; record what is
  retained or rejected in [`branch-retirement.md`](./branch-retirement.md).
- [x] Close `feature/tui` with commit `b9b5778` and annotated tag
  `archive/feature/tui`.
- [x] Close `feature/enhancements-2026` with commit `9a6d6eb` and annotated tag
  `archive/feature/enhancements-2026`.
- [x] Remove `indicatif` progress/status updates from the `develop` code while retaining
  `--timer`, `--no-banner`, formatter styling, and ordered output.
- [x] Update current product/architecture docs, restore a documentation index, and record the
  simpler terminal-output policy.
- [x] Validate the change: `cargo test` passed (30 unit tests and 6 end-to-end tests);
  `cargo tree -i indicatif` found no such dependency; source searches found no progress-bar
  constructor or `indicatif` references; `git diff --check` reported no whitespace errors.
- [x] Run `cargo fmt --all -- --check`; it reports pre-existing formatting differences across
  source and test files, so unrelated formatting was left unchanged.

### Follow-up: Terminal polish and discovery feedback (2026-10-17)

> Implementation is complete; verification details are recorded below.

- [x] Remove the leading blank line before the ASCII banner, and remove bold styling from all
  terminal formatter styles so text uses normal font weight.
- [x] Replace the successful-repository glyph with a compact green check/mark and explicitly
  retain one separating space before the repository path, matching the warning marker layout.
- [x] Add visible repository-discovery feedback for interactive human runs:
  - [x] Show a lightweight dotted spinner and the number of repositories found while scanning.
  - [x] Leave a final discovery-completion status visible and put a separator before repository
    output, as preferred after considering the terminal flow.
  - [x] Gate the indicator on stderr terminal capability and disable it for JSON output; status
    text remains off stdout.
  - [x] Keep the scan lightweight: no TUI, temporary files, sockets, or heavyweight progress
    dependency unless implementation evidence shows one is needed.
- [x] Preserve the current global path sort and in-order result output for this change:
  discovery currently returns a complete sorted list before commands start, and the output
  streamer already emits results in that order as each expected result becomes available.
  Starting commands before discovery completes would require a bounded discovery-to-worker
  pipeline and a decision between preserving global sort (which requires knowing all paths)
  and switching to traversal order; this remains a separate design choice, not an implicit part
  of the progress indicator.
- [x] Add focused tests for banner spacing, absence of bold style, success-marker spacing, and
  discovery progress lifecycle/counting.
- [x] Update maintained UX/design documentation to match the implemented behavior.
- [x] Run the full `cargo test` suite on Windows: 33 unit tests and 6 end-to-end tests passed.
- [x] Existing end-to-end coverage confirms JSON remains parseable, stdout preserves ordered
  output, and the non-interactive JSON run has no discovery progress on stderr.
- [x] Preview the retained discovery status, banner, separator, and repository-output flow in a
  Windows VS Code task terminal.
- [ ] Manually assess spinner animation/layout on macOS and Linux terminals.

### Follow-up: Colored terminal status markers (2026-10-17)

- [x] Replace terminal emoji/status glyphs with colored-circle indicators: blue for the command
  header, green for success, orange for Git failures/warnings, and red for execution errors.
- [x] Update the terminal formatter regression tests and low-level design description to match.
- [x] Run full `cargo test` (34 unit tests and 6 end-to-end tests passed) and `git diff --check`.

### Follow-up: Configurable worker threads (2026-10-17)

- [x] Use `-j` / `--jobs` for both concurrent Git command limit and Tokio runtime worker threads.
- [x] Change the default to 2 while preserving a valid positive `MGIT_PARALLEL` override.
- [x] Update CLI help and maintained usage/architecture docs; remove the now-unused `num_cpus`
  dependency.
- [x] Run `cargo test`: 34 unit tests and 6 end-to-end tests passed; Cargo updated the lockfile
  after removing the unused `num_cpus` dependency.

### Follow-up: CLI help, configuration, exclusions, and terminal UX (2026-10-18)

- [x] Clarify `--help` with the standalone `-h`/`--help` and `-V`/`--version` behavior before
  the delimiter, the supported mgit-option forms after the last `--`, and the no-Git-arguments
  error; ensure the documented routing rule matches `parse_cli`.
- [x] Load current mgit options from `~/.config/mgit.toml`, with platform home resolution,
  defaults/config/environment/CLI precedence, explicit config errors, and parser tests.
- [x] Add `--save-config` to save mgit options explicitly supplied on the CLI into
  `~/.config/mgit.toml`; update only explicitly specified keys and preserve unspecified settings
  and unrelated sections (including repository exclusions). Define its interaction with the
  final `--` routing rule and test safe partial updates.
- [ ] Design and implement repository exclusions as a multiline pattern list in a section of
  `~/.config/mgit.toml` (no separate ignore file), supporting absolute and discovery-root-relative
  paths, glob wildcards including `**/*`-style patterns, and both `/` and `\` separators; define
  normalization/glob semantics, excluded-repository pruning, and discovery tests.
- [x] Replace the plain dotted discovery indicator with a colored rotating dotted marker while
  preserving TTY/JSON behavior.
- [ ] Manually validate Unicode/color rendering and terminal width on macOS and Linux.

---

## Code Review: Issues, Bugs, and Opportunities

### Bugs & Correctness
1. **`stream_in_order` drops pending results or produces mismatched summary counts on early termination / fail-fast**
   - *Problem*: In `src/output.rs`, `stream_in_order` terminates early when `rx.recv()` yields `None` if `run_engine` breaks on cancellation or error. Any buffered out-of-order items in `pending` with higher IDs are discarded, and `stats.total` will not equal the announced repository count.
   - *Solution*: Drain remaining buffered elements in `pending` into `stats` (or record cancellation errors) when `rx` closes, and ensure reported totals align.
2. **Missing exit code `127` when `git` executable is not found on `$PATH`**
   - *Problem*: The LLD (`low_level_design.md` §3) explicitly mandates exit code `127` when Git cannot be spawned, but `src/engine.rs` returns `RepoStatus::Error` which collapses to exit code `1` in `src/main.rs`.
   - *Solution*: Differentiate `std::io::ErrorKind::NotFound` when spawning `git` to return a dedicated error indicator or exit code `127`.
3. **`SummaryStats.elapsed_ms` sums per-repository execution durations rather than wall-clock elapsed time**
   - *Problem*: In `src/output.rs`, `stats.elapsed_ms += result.elapsed_ms` sums concurrent execution times, rendering "Total: X.XXs" in the summary misleadingly high compared to actual elapsed time.
   - *Solution*: Measure total wall-clock duration with `Instant::now()` from before execution begins through streaming completion.
4. **CI release workflow builds artifacts without running tests**
   - *Problem*: In `.github/workflows/release.yml`, the build job directly runs `cargo build --release` and publishes releases without invoking `cargo test`.
   - *Solution*: Add a test step (`cargo test --all`) prior to building release binaries in the workflow.

### Edge Cases & Robustness
5. **Silent suppression of discovery errors and permissions**
   - *Problem*: In `src/discovery.rs`, `Err(_) => continue` silently swallows directory read permission errors and corrupted paths without notifying the user.
   - *Solution*: Log or collect unreadable directory warnings to inform the user when repositories might have been skipped.
6. **Case-insensitive pruning can prune legitimate directories on case-sensitive filesystems**
   - *Problem*: In `src/discovery.rs`, `eq_ignore_ascii_case` prunes folders named `Target` or `Build` on Linux even when they are not build output folders.
   - *Solution*: Make directory pruning case-sensitive on Unix/Linux systems or restrict case-insensitivity to Windows/macOS.
7. **Unbounded stdout/stderr memory buffer accumulation per repository**
   - *Problem*: In `src/engine.rs`, `read_to_end` reads entire output into memory, which can lead to high memory consumption on verbose commands across many repositories.
   - *Solution*: Apply a sensible buffer size cap or stream large outputs to temporary storage if exceeded.
8. **Broken-pipe panics or swallowed write errors on piped terminal stdout**
   - *Problem*: In `src/output.rs`, multiple `let _ = writeln!(out, ...)` calls discard errors, continuing execution even if downstream consumers (e.g., `head`) disconnect.
   - *Solution*: Check for `io::ErrorKind::BrokenPipe` and shut down gracefully instead of running remaining jobs.

### UX & Interface Consistency
9. **Git failure results labeled as "Warnings" in summary**
   - *Problem*: In `src/output.rs`, non-zero Git exits (`RepoStatus::Failure`) are printed as `Warnings: N`, which is counter-intuitive for actual Git errors like checkout failures.
   - *Solution*: Label non-zero exit codes as `Failed: N` and reserve warnings for partial or non-fatal states.
10. **Missing trailing newline on repository outputs causing delimiter collisions**
    - *Problem*: In `src/output.rs`, if a Git command outputs text lacking a final newline, the separator dashed line is rendered on the same line.
    - *Solution*: Ensure repository output ends with a newline before printing the separator line.
11. **`--save-config` performs redundant file write if no explicit CLI options were provided**
    - *Problem*: In `src/main.rs`, running `mgit status -- --save-config` with no other options still reads and re-serializes the config file.
    - *Solution*: Skip saving and warn if `config.explicit_options` is empty.

### Code Quality & Architecture
12. **Circular domain modeling between `output.rs` and `engine.rs`**
    - *Problem*: `RepoStatus` and `RepoResult` are defined in `src/output.rs` despite being execution artifacts generated by `engine.rs`.
    - *Solution*: Extract execution models into a shared `types.rs` or keep execution models in `engine.rs`.
13. **Unnecessary `Arc<Mutex<W>>` synchronization in sequential output formatters**
    - *Problem*: In `src/output.rs`, `TerminalFormatter` wraps the output stream in an `Arc<Mutex<W>>` even though output is consumed sequentially by a single thread in `stream_in_order`.
    - *Solution*: Use plain mutable references or direct stream ownership without mutex synchronization.
14. **Unpinned `cross` dependency in CI builds**
    - *Problem*: In `.github/workflows/release.yml`, `cargo install cross --git ...` pulls `HEAD` of the master branch, creating a risk of unexpected build breakages.
    - *Solution*: Pin the `cross` installation to a known stable release tag or prebuilt action.

### Test Coverage Gaps
15. **Missing end-to-end coverage for `--fail-fast` and `--save-config`**
    - *Problem*: `tests/e2e_tests.rs` lacks integration tests validating abort-on-error behavior and configuration persistence across invocations.
    - *Solution*: Add automated tests running the compiled CLI against multi-repo fixtures verifying `--fail-fast` stopping conditions and config file updates.
16. **Build script (`build.rs`) SemVer resolution logic lacks unit tests**
    - *Problem*: `build.rs` contains complex version parsing and Conventional Commit analysis but has no standalone tests.
    - *Solution*: Add unit tests covering `parse_version`, `bumped_version`, and conventional commit bumping logic.
