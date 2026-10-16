use std::collections::BTreeMap;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Terminal, TerminalOptions, Viewport};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::args::MgitConfig;
use crate::discovery::discover_repos_with_progress;
use crate::engine::run_engine_streaming;
use crate::output::{OutputStream, RepoEvent, RepoResult, SummaryStats};

const VIEWPORT_HEIGHT: u16 = 12;
const UI_TICK: Duration = Duration::from_millis(100);

#[derive(Debug, Default)]
struct RepoLog {
    repo: String,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    lines: Vec<(OutputStream, usize, usize)>,
    line_start: [usize; 2],
    last_was_cr: [bool; 2],
    result: Option<RepoResult>,
}

impl RepoLog {
    fn append_chunk(&mut self, stream: OutputStream, bytes: &[u8]) {
        let index = stream.index();
        let buffer = match stream {
            OutputStream::Stdout => &mut self.stdout,
            OutputStream::Stderr => &mut self.stderr,
        };
        let offset = buffer.len();
        buffer.extend_from_slice(bytes);

        for (relative, byte) in bytes.iter().enumerate() {
            let position = offset + relative;
            if *byte == b'\n' && self.last_was_cr[index] {
                self.last_was_cr[index] = false;
                self.line_start[index] = position + 1;
                continue;
            }
            if *byte == b'\r' || *byte == b'\n' {
                self.lines.push((stream, self.line_start[index], position));
                self.line_start[index] = position + 1;
                self.last_was_cr[index] = *byte == b'\r';
            } else {
                self.last_was_cr[index] = false;
            }
        }
    }

    fn finish_stream(&mut self, stream: OutputStream) {
        let index = stream.index();
        let length = match stream {
            OutputStream::Stdout => self.stdout.len(),
            OutputStream::Stderr => self.stderr.len(),
        };
        if self.line_start[index] < length {
            self.lines.push((stream, self.line_start[index], length));
            self.line_start[index] = length;
        }
        self.last_was_cr[index] = false;
    }

    fn line_text(&self, stream: OutputStream, start: usize, end: usize) -> String {
        let buffer = match stream {
            OutputStream::Stdout => &self.stdout,
            OutputStream::Stderr => &self.stderr,
        };
        String::from_utf8_lossy(&buffer[start..end]).into_owned()
    }
}

pub struct UiRepoCapture {
    pub result: RepoResult,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub struct UiOutcome {
    pub stats: SummaryStats,
    pub total: usize,
    pub cancelled: bool,
    pub results: Vec<UiRepoCapture>,
}

struct InlineTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl InlineTerminal {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = match Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(VIEWPORT_HEIGHT),
            },
        ) {
            Ok(terminal) => terminal,
            Err(err) => {
                let _ = disable_raw_mode();
                return Err(err);
            }
        };

        let mut inline = Self { terminal };
        inline.terminal.hide_cursor()?;
        inline.terminal.clear()?;
        Ok(inline)
    }

    fn draw(
        &mut self,
        status: &str,
        logs: &BTreeMap<usize, RepoLog>,
        scroll_from_bottom: usize,
    ) -> io::Result<u16> {
        let mut visible_rows = 1;
        self.terminal.draw(|frame| {
            let areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(frame.area());
            frame.render_widget(
                Paragraph::new(status.to_string())
                    .style(Style::default().add_modifier(Modifier::BOLD)),
                areas[0],
            );

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Command output ");
            let output_area = block.inner(areas[1]);
            visible_rows = output_area.height.max(1);
            let line_count = logs
                .values()
                .map(|repo| repo.lines.len() + 1)
                .sum::<usize>();
            let scroll = scroll_from_bottom.min(line_count.saturating_sub(visible_rows as usize));

            let mut visible = logs
                .values()
                .rev()
                .flat_map(|repo| {
                    repo.lines
                        .iter()
                        .rev()
                        .map(|(stream, start, end)| {
                            let label = match stream {
                                OutputStream::Stdout => "out",
                                OutputStream::Stderr => "err",
                            };
                            format!(" [{label}] {}", repo.line_text(*stream, *start, *end))
                        })
                        .chain(std::iter::once(format!("› {}", repo.repo)))
                })
                .skip(scroll)
                .take(visible_rows as usize)
                .map(Line::from)
                .collect::<Vec<_>>();
            visible.reverse();

            frame.render_widget(
                Paragraph::new(visible)
                    .block(block)
                    .wrap(Wrap { trim: false }),
                areas[1],
            );
        })?;
        Ok(visible_rows)
    }

    fn poll_keys(&mut self, scroll_from_bottom: &mut usize, page_size: usize) -> io::Result<bool> {
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
                    KeyCode::Up => *scroll_from_bottom = scroll_from_bottom.saturating_add(1),
                    KeyCode::Down => *scroll_from_bottom = scroll_from_bottom.saturating_sub(1),
                    KeyCode::PageUp => {
                        *scroll_from_bottom = scroll_from_bottom.saturating_add(page_size.max(1));
                    }
                    KeyCode::PageDown => {
                        *scroll_from_bottom = scroll_from_bottom.saturating_sub(page_size.max(1));
                    }
                    KeyCode::Home => *scroll_from_bottom = usize::MAX,
                    KeyCode::End => *scroll_from_bottom = 0,
                    _ => {}
                }
            }
        }
        Ok(false)
    }
}

impl Drop for InlineTerminal {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = disable_raw_mode();
    }
}

pub async fn run_ui(
    git_args: Arc<Vec<String>>,
    config: Arc<MgitConfig>,
    cancel_token: CancellationToken,
) -> Result<UiOutcome, String> {
    let mut terminal = InlineTerminal::new().map_err(|err| err.to_string())?;
    let discovered = Arc::new(AtomicUsize::new(0));
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
    let discovery_config = config.clone();
    let discovery_count = discovered.clone();
    let discovery_cancel = cancel_token.clone();
    let discovery_task = tokio::task::spawn_blocking(move || {
        let mut last_reported = usize::MAX;
        discover_repos_with_progress(&discovery_config, |count| {
            discovery_count.store(count, Ordering::Relaxed);
            if count != last_reported {
                let _ = progress_tx.send(count);
                last_reported = count;
            }
            !discovery_cancel.is_cancelled()
        })
    });

    let mut ticker = tokio::time::interval(UI_TICK);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut scroll_from_bottom = 0usize;
    let spinner = ['|', '/', '-', '\\'];
    let mut tick_count = 0usize;

    let mut discovery_task = discovery_task;
    let repos = loop {
        tokio::select! {
            result = &mut discovery_task => {
                let discovered_repos = result.map_err(|err| err.to_string())?;
                match discovered_repos {
                    Ok(repos) => break repos,
                    Err(err) if cancel_token.is_cancelled() && err.kind() == io::ErrorKind::Interrupted => {
                        let total = discovered.load(Ordering::Relaxed);
                        terminal
                            .draw(" mgit Cancelled during discovery", &BTreeMap::new(), 0)
                            .map_err(|draw_err| draw_err.to_string())?;
                        drop(terminal);
                        return Ok(UiOutcome {
                            stats: SummaryStats::default(),
                            total,
                            cancelled: true,
                            results: Vec::new(),
                        });
                    }
                    Err(err) => return Err(err.to_string()),
                }
            }
            _ = progress_rx.recv() => {}
            _ = ticker.tick() => {
                tick_count = tick_count.wrapping_add(1);
                let count = discovered.load(Ordering::Relaxed);
                let status = format!(" mgit {} Scanning... {count} repositories found", spinner[tick_count % spinner.len()]);
                let page_size = terminal.draw(&status, &BTreeMap::new(), scroll_from_bottom)
                    .map_err(|err| err.to_string())? as usize;
                if terminal.poll_keys(&mut scroll_from_bottom, page_size).map_err(|err| err.to_string())? {
                    cancel_token.cancel();
                }
            }
        }
    };

    let total = repos.len();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let worker = tokio::spawn(run_engine_streaming(
        repos,
        git_args,
        config.clone(),
        cancel_token.clone(),
        tx,
        true,
    ));

    let mut stats = SummaryStats::default();
    let mut logs = BTreeMap::<usize, RepoLog>::new();
    scroll_from_bottom = 0;
    let mut finished = 0usize;
    let mut saw_cancel = cancel_token.is_cancelled();

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(RepoEvent::Started { id, repo }) => {
                        logs.entry(id).or_default().repo = repo;
                    }
                    Some(RepoEvent::CapturedChunk { id, stream, bytes }) => {
                        logs.entry(id).or_default().append_chunk(stream, &bytes);
                    }
                    Some(RepoEvent::StreamFinished { id, stream }) => {
                        logs.entry(id).or_default().finish_stream(stream);
                    }
                    Some(RepoEvent::Output { .. }) => {}
                    Some(RepoEvent::Finished(mut result)) => {
                        let log = logs.entry(result.id).or_default();
                        if log.stdout.is_empty() && log.stderr.is_empty() && !result.stderr.is_empty() {
                            log.append_chunk(OutputStream::Stderr, result.stderr.as_bytes());
                        }
                        log.finish_stream(OutputStream::Stdout);
                        log.finish_stream(OutputStream::Stderr);
                        stats.record(&result);
                        finished += 1;
                        result.stdout.clear();
                        result.stderr.clear();
                        log.result = Some(result);
                    }
                    None => break,
                }
            }
            _ = ticker.tick() => {
                tick_count = tick_count.wrapping_add(1);
                saw_cancel |= cancel_token.is_cancelled();
                let status = format!(
                    " mgit {} {}/{} complete · {} remaining · {} failed · q/Ctrl+C cancels",
                    spinner[tick_count % spinner.len()],
                    finished,
                    total,
                    total.saturating_sub(finished),
                    stats.failure + stats.error,
                );
                let page_size = terminal.draw(&status, &logs, scroll_from_bottom)
                    .map_err(|err| err.to_string())? as usize;
                if terminal.poll_keys(&mut scroll_from_bottom, page_size).map_err(|err| err.to_string())? {
                    cancel_token.cancel();
                }
            }
        }
    }

    worker.await.map_err(|err| err.to_string())?;
    let status = if saw_cancel || cancel_token.is_cancelled() {
        format!(
            " mgit Cancelled · {} of {} finished · {} failed",
            finished,
            total,
            stats.failure + stats.error,
        )
    } else {
        format!(
            " mgit Complete · {} repositories · {} failed",
            total,
            stats.failure + stats.error,
        )
    };
    terminal
        .draw(&status, &logs, 0)
        .map_err(|err| err.to_string())?;
    drop(terminal);

    let mut results = logs
        .into_values()
        .filter_map(|mut log| {
            Some(UiRepoCapture {
                result: log.result.take()?,
                stdout: log.stdout,
                stderr: log.stderr,
            })
        })
        .collect::<Vec<_>>();
    results.sort_by_key(|result| result.result.id);

    Ok(UiOutcome {
        stats,
        total,
        cancelled: saw_cancel || cancel_token.is_cancelled(),
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_output_preserves_bytes_and_tracks_display_lines() {
        let mut log = RepoLog::default();
        log.append_chunk(OutputStream::Stdout, b"first\r");
        log.append_chunk(OutputStream::Stdout, b"\nsecond");
        log.finish_stream(OutputStream::Stdout);
        log.append_chunk(OutputStream::Stderr, b"warning\n");
        log.finish_stream(OutputStream::Stderr);

        assert_eq!(log.stdout, b"first\r\nsecond");
        assert_eq!(log.stderr, b"warning\n");
        assert_eq!(log.lines.len(), 3);
        assert_eq!(log.line_text(OutputStream::Stdout, 0, 5), "first");
        assert_eq!(log.line_text(OutputStream::Stdout, 7, 13), "second");
        assert_eq!(log.line_text(OutputStream::Stderr, 0, 7), "warning");
    }
}
