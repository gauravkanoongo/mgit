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

## Optional Inline Output UI

Design and constraints are documented in [`ui-deliberations.md`](./ui-deliberations.md) and the UI section of [`low_level_design.md`](./low_level_design.md).

- [x] Settle the UI contract: `--tui`, terminal/error behavior, compatible options, scrolling, stream labels, and summary placement.
- [x] Implement inline discovery status/progress and scrollable stdout/stderr rendering without changing the default, JSON, or redirected paths.
- [x] Capture output once in memory for the live view and complete replay; preserve repository ordering and avoid blocking output readers or reducing job concurrency.
- [x] Restore terminal state and replay captured bytes on completion, errors, Ctrl+C, and fail-fast cancellation.
- [x] Add focused unit and end-to-end tests, including non-interactive rejection.
- [x] Run `cargo test` (35 unit tests and 7 end-to-end tests passed).
- [ ] Perform interactive pseudo-terminal smoke tests on supported terminals.
