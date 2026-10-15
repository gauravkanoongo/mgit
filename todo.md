# mgit Implementation Task Tracker (`todo.md`)

> Current Branch: `feature/enhancements-2026`  
> Target: Complete Phase 1 through Phase 5 with 100% test coverage.

---

## Overall Progress

- [ ] **Phase 1: Toolchain Modernization & Argument Parser Engine**
- [ ] **Phase 2: Repository Discovery Engine & Pruning**
- [ ] **Phase 3: In-Order Output Streaming & Formatters**
- [ ] **Phase 4: Async Process Execution Engine (`tokio`)**
- [ ] **Phase 5: CLI Integration & End-to-End Test Suite**

---

## Detailed Task Breakdown

### Phase 1: Toolchain Modernization & Argument Parser Engine
- [ ] Upgrade `Cargo.toml` to `edition = "2024"` and add required dependencies
- [ ] Implement `src/args.rs`:
  - [ ] `MgitConfig` struct with full defaults (`dir`, `jobs`, `quiet`, `only_changes`, `json`, `summary_only`, `fail_fast`, `max_depth`)
  - [ ] `parse_cli` function splitting at the **last `--`** delimiter
  - [ ] `lexopt` parser for tokens following the last `--`
- [ ] Unit Tests for `src/args.rs`:
  - [ ] `test_no_arguments_returns_error`
  - [ ] `test_pure_git_command_no_separator`
  - [ ] `test_git_command_with_internal_separator`
  - [ ] `test_separator_with_all_mgit_flags`
  - [ ] `test_multiple_separators_last_takes_precedence`
  - [ ] `test_invalid_mgit_flag_returns_error`
  - [ ] `test_invalid_jobs_parameter_returns_error`
  - [ ] `test_help_flag_triggers_help_signal`
  - [ ] `test_mgit_parallel_env_fallback`

---

### Phase 2: Repository Discovery Engine & Pruning
- [ ] Implement `src/discovery.rs`:
  - [ ] `RepoEntry` struct (`id`, `path`)
  - [ ] `discover_repos(&MgitConfig)` with `walkdir`
  - [ ] Directory pruning (`.git`, `node_modules`, `target`, `.cargo`, `dist`, `build`, `vendor`, `.venv`)
  - [ ] Git worktree & submodule `.git` file pointer support
  - [ ] Lexicographical path sorting & deterministic ID assignment (`0..N`)
- [ ] Hermetic Unit Tests for `src/discovery.rs` (using `tempfile`):
  - [ ] `test_discover_empty_directory`
  - [ ] `test_discover_single_standard_git_repo`
  - [ ] `test_discover_worktree_submodule_git_file`
  - [ ] `test_prune_ignored_directories`
  - [ ] `test_nested_repos_handling`
  - [ ] `test_deterministic_path_sorting`
  - [ ] `test_max_depth_limiting`
  - [ ] `test_symlink_and_junction_handling`

---

### Phase 3: In-Order Output Streaming & Formatters
- [ ] Implement `src/output.rs`:
  - [ ] `RepoStatus` and `RepoResult` structs
  - [ ] `SummaryStats` metric accumulator
  - [ ] `OutputFormatter` trait
  - [ ] `TerminalFormatter` (using `anstream` & `anstyle` with `NO_COLOR` support)
  - [ ] `JsonFormatter` (full AI-friendly JSON schema serialization)
  - [ ] `stream_in_order` buffered sequential emitting algorithm
- [ ] Unit Tests for `src/output.rs`:
  - [ ] `test_in_order_streaming_out_of_order_arrival`
  - [ ] `test_terminal_formatter_quiet_mode`
  - [ ] `test_terminal_formatter_only_changes_mode`
  - [ ] `test_terminal_formatter_failure_and_error`
  - [ ] `test_json_formatter_valid_schema`
  - [ ] `test_summary_only_mode`

---

### Phase 4: Async Process Execution Engine (`tokio`)
- [ ] Implement `src/engine.rs`:
  - [ ] `execute_repo` async function using `tokio::process::Command`
  - [ ] Elapsed execution time measurement per repo
  - [ ] Concurrency control with `tokio::sync::Semaphore`
  - [ ] Cancellation token propagation for `--fail-fast`
  - [ ] `run_engine` coordinator task spawning
- [ ] Unit & Mock Tests for `src/engine.rs`:
  - [ ] `test_execute_git_status_success`
  - [ ] `test_execute_git_failure_status`
  - [ ] `test_execute_nonexistent_binary_error`
  - [ ] `test_semaphore_concurrency_bounding`
  - [ ] `test_fail_fast_cancellation`

---

### Phase 5: CLI Integration & End-to-End Test Suite
- [ ] Refactor `src/main.rs`:
  - [ ] Wire together `args`, `discovery`, `engine`, and `output`
  - [ ] Implement help text renderer
  - [ ] Deterministic exit code logic (`0`, `1`, `2`, `127`)
- [ ] Create `tests/e2e_tests.rs`:
  - [ ] `test_e2e_normal_git_status`
  - [ ] `test_e2e_separator_syntax`
  - [ ] `test_e2e_json_output_machine_readable`
  - [ ] `test_e2e_failure_exit_code`
  - [ ] `test_e2e_only_changes_filtering`
  - [ ] `test_e2e_no_color_environment`
- [ ] Verify 100% test pass rate with `cargo test`

