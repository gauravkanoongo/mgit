# mgit Interactive Output UI: Deliberations

## Purpose

Record the alternatives considered for showing status and progress alongside live output from
multiple repositories, and the direction selected for implementation. The inline UI is available
through `--tui`.

## User goal

When explicitly requested in an interactive terminal, mgit should provide a clearly bounded
terminal view with:

- a top status area for repository discovery and execution progress;
- a scrollable output area below it for repository command stdout and stderr;
- live updates while repository commands run; and
- normal console output available in the terminal's scrollback after mgit exits.

The default CLI behavior, JSON output, and redirected/non-interactive behavior must remain
unchanged unless the user opts into the UI.

## Alternatives considered

### Inline progress indicator (`indicatif`)

`indicatif` is a lightweight choice for a spinner or one-line progress bar, and can coordinate
progress redraws with log lines. It is appropriate when the requirement is simply to report
progress beside ordinary terminal output.

It does not provide the bounded, independently scrollable output region requested here. Writing
normal logs while maintaining a fixed header/output pane would require additional cursor and
screen management.

### fzf

fzf provides a fuzzy-select list with a preview pane and can be driven with list updates and
preview commands. It could support repository selection and per-repository previews.

It is an external optional executable, not a general-purpose multi-pane log viewer. A global
ordered output view plus selected-repository detail is awkward, and it brings process/IPC,
preview-refresh, and platform-shell integration concerns that are unnecessary for the selected
status-and-output view.

### Terminal multiplexer or persistent terminal server

Tools such as tmux, zmx, Herdr, and RMUX retain terminal sessions and scrollback in long-lived
processes, usually in memory, and expose them to clients through IPC. Their buffers are part of
the terminal/session engine. mgit does not need persistent sessions, detachable clients, or a
terminal server just to show one run's progress; adopting that architecture would be excessive.

### Fullscreen TUI / alternate screen

A conventional fullscreen TUI can provide multiple panes and rich navigation, but takes over the
terminal screen and generally separates the view from the user's ordinary scrollback. That is not
the desired interaction.

### Selected direction: opt-in inline TUI

Use a small Rust TUI built with **Ratatui and Crossterm** as the leading implementation choice.
Ratatui supports inline viewports; Crossterm provides cross-platform terminal control. The UI
should use a simple layout rather than fzf or a terminal multiplexer:

```text
+--------------------------------------------------+
| mgit  Scanning... / 12 of 47 complete ...        |  status/progress
+--------------------------------------------------+
|                                                  |
|  Scrollable combined command output              |
|                                                  |
+--------------------------------------------------+
```

The implemented option is `--tui`, parsed after mgit's final `--`. It does not activate
automatically. It requires interactive stdout and stderr and reports an error otherwise. It cannot
be combined with `--json`, `--summary-only`, `--quiet`, or `--only-changes`.

## Progress model

1. **Discovery:** show an indeterminate spinner and the number of repositories found so far.
   Do not perform a second traversal just to manufacture a percentage.
2. **Execution:** after discovery establishes the total, show completed/total and optionally the
   number currently running. Progress advances when a repository finishes, not on every output
   chunk.
3. **Completion:** show final success/failure/error counts and elapsed time using the existing
   summary data.

The TUI should redraw at a modest, bounded rate, coalescing output/status updates instead of
redrawing once per byte or line.

## Output and memory model

- Capture child stdout and stderr bytes once in memory, partitioned by repository and stream.
  Line-offset metadata renders visible output without a second persistent copy. No temporary
  output files, sockets, per-repository services, or external preview process are needed.
- Preserve mgit's deterministic repository ordering in the displayed output. Later repository
  output may need to wait in memory until earlier repository output is displayable, matching the
  current ordered-streaming contract.
- The whole captured output is replayed as ordinary terminal output when the UI exits, making it
  available in the terminal emulator's native scrollback. This is replay, not a transfer of
  Ratatui's buffer: the terminal emulator owns its scrollback.
- Complete replay necessarily retains all output until exit. Therefore memory use scales with
  total child output. A bounded/truncated buffer would reduce peak memory but would violate the
  requirement to replay the complete output; do not introduce truncation without an explicit
  product decision.

### Stream ordering caveat

stdout and stderr are separate child pipes. Their exact original cross-stream chronology cannot
be reconstructed from independent reads. Keep stream identity in captured records and define a
stable observed-event ordering for display/replay; do not claim byte-perfect original chronology
between stdout and stderr. Within each stream, preserve read order. The current user-facing
formatter already presents both streams in the same output flow.

## Exit and terminal behavior

- The UI remains active until all work is complete or cancellation has taken effect.
- On normal completion, restore terminal modes/cursor, leave the inline region cleanly, replay the
  captured output to normal terminal output, then show the final summary/exit result.
- On Ctrl+C, fail-fast cancellation, or an error, use the same terminal-restoration path before
  returning. Do not leave raw mode, hidden cursor, or a damaged display behind.
- Inline UI requires an interactive terminal. JSON and redirected output remain on existing paths.
- Output from a command that itself requires an interactive TTY is outside the initial scope:
  mgit's current subprocess model pipes stdout/stderr and does not allocate a PTY. Initial UI
  support is for ordinary non-interactive Git commands.

## Performance priorities

1. Do not slow down repository discovery or concurrent Git execution to render the interface.
2. Use the existing async execution and event flow; rendering must not block child-output readers.
3. Avoid per-chunk terminal redraws; use a bounded UI tick and coalesce notifications.
4. Store output once in memory, with repository/stream boundaries, and render visible portions
   only.
5. Keep the UI optional and avoid overhead/buffers when it is not requested.

The main unavoidable cost is retaining complete output for live scrolling and post-exit replay.
Other risks are backpressure if the renderer cannot consume events, memory growth for huge command
output, terminal resize/cleanup behavior, and tests that need a pseudo-terminal to exercise the
interactive path.

## Decisions and validation status

The initial product decisions are implemented:

- `--tui` is the explicit opt-in; it reports an error if stdout or stderr is not interactive.
- `--json`, `--summary-only`, `--quiet`, and `--only-changes` are rejected with `--tui`.
- Up/Down and PageUp/PageDown scroll; Home/End move to oldest/latest output. New output is
  followed until manual scrolling pauses it; returning to the bottom resumes following.
- The output pane labels stdout as `out` and stderr as `err`.
- After terminal restoration, captured stdout/stderr bytes are replayed before the final summary.

Automated tests cover option conflicts, in-memory output and line indexing, and non-interactive
rejection. Manual pseudo-terminal validation remains needed across Windows Terminal/ConPTY, Unix
terminals, SSH, and IDE terminals.

## References

- [Ratatui viewport documentation](https://docs.rs/ratatui/latest/ratatui/enum.Viewport.html)
- [Ratatui terminal documentation, including inline output insertion](https://docs.rs/ratatui/latest/ratatui/struct.Terminal.html)
- [Ratatui scrollable Paragraph widget](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Paragraph.html)
- [Crossterm documentation](https://docs.rs/crossterm/latest/crossterm/)
- [Indicatif ProgressBar](https://docs.rs/indicatif/latest/indicatif/struct.ProgressBar.html)
- [fzf](https://github.com/junegunn/fzf)
- [Herdr session state](https://herdr.dev/docs/session-state/)
- [zmx](https://github.com/neurosnap/zmx)
- [RMUX architecture](https://rmux.io/docs/architecture/)
