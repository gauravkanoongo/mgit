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
