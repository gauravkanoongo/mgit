# Branch Retirement and Carry-Forward Decisions

**Decision date:** 2026-10-17  
**Active baseline:** `develop`, with the same file tree as `v0.2.0`  
**Stable behavior reference:** `v0.1.0`

## Branch and release comparison

### `v0.1.0` to `v0.2.0`

The release comparison showed changes confined to `Cargo.toml`, `Cargo.lock`, `src/args.rs`,
`src/main.rs`, and `src/output.rs`. The main regression was the new `indicatif` progress bar:
when stdout was a TTY, `main` enabled it automatically, and the ordered output consumer updated
its spinner/status line while rendering repository output. This tied normal output to terminal
cursor behavior and made the interface noisy and fragile.

Other independent terminal improvements were added alongside the progress indicator and are
retained:

- `--timer` displays per-repository and aggregate durations.
- `--no-banner` suppresses the ASCII startup banner.
- Existing terminal formatter styling and deterministic repository output remain.

`develop` currently has a different commit ID from the `v0.2.0` tag, but its tracked file tree
matches that tag exactly; this is a content baseline, not a claim that the refs point to the same
commit.

### `feature/enhancements-2026` and `feature/tui`

Both experimental lines accumulated design documents for the inline/TUI direction, including
`ui-deliberations.md`, and changes to the README, plan, low-level design, and task tracker. The
TUI proposal added a much larger interaction and output-capture model: terminal control, panes or
viewports, in-memory stream capture, scrolling, and replay. The progress-bar attempt in the
enhancement line also showed that even a much smaller status display can interfere with ordinary
command output.

These approaches are closed rather than being implementation roadmaps. Their branches and
historical docs are retained by local annotated archive tags; the design proposals are not
accepted requirements for future work.

## Carry-forward policy

Keep the parts that preserve mgit's simple, useful core:

1. Use `v0.1.0` as the known-good behavioral reference when judging regressions.
2. Keep the current `v0.2.0` core: repository discovery, worktree/submodule recognition,
   bounded asynchronous execution, deterministic ordered output, formatters, and failure
   reporting.
3. Retain `--timer`, `--no-banner`, and the established formatter styling.
4. Keep normal output as normal sequential terminal writes, regardless of whether stdout is a
   terminal or redirected.

Do not carry forward:

- Automatic or opt-in spinner, progress bar, or status-line redraws.
- A TUI, alternate screen, panes, live repository selection, or fzf integration.
- Output capture/replay, extra IPC, sockets, persistent servers, or disk-backed buffers.
- `indicatif`, Ratatui, or Crossterm solely for progress/status presentation.

Any future terminal UX proposal should first demonstrate that it preserves plain output,
stdout/stderr behavior, scrolling, and performance on supported platforms. It should be a
separately scoped opt-in feature with tests that exercise real TTY behavior; do not add it to the
default execution path.

## Closure and archive record

The branches were closed with empty commits that make their disposition explicit without changing
their source trees:

| Branch | Closure commit | Annotated archive tag | Disposition |
| --- | --- | --- | --- |
| `feature/tui` | `b9b5778` | `archive/feature/tui` | TUI rebuild effort abandoned |
| `feature/enhancements-2026` | `9a6d6eb` | `archive/feature/enhancements-2026` | Experimental branch closed; only deliberate milestones carry forward |

Both commits and tags were created locally. No remote branches or tags were pushed.

The requested `refs/tags/.archive/...` form is invalid under Git's ref-name rules because a
component cannot begin with a dot. The valid equivalent used here is
`refs/tags/archive/<branch-name>`.
