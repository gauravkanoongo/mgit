use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use anstyle::{AnsiColor, Color, Style};

use crate::args::MgitConfig;

/// Status outcome of a Git execution in a repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    Success,
    Failure,
    Error,
}

/// Result of executing a command in a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoResult {
    pub id: usize,
    pub repo: String,
    pub status: RepoStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub has_changes: bool,
    pub elapsed_ms: u64,
}

/// Aggregated execution statistics.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SummaryStats {
    pub total: usize,
    pub success: usize,
    pub failure: usize,
    pub error: usize,
    pub elapsed_ms: u64,
}

impl SummaryStats {
    pub fn record(&mut self, result: &RepoResult) {
        self.total += 1;
        match result.status {
            RepoStatus::Success => self.success += 1,
            RepoStatus::Failure => self.failure += 1,
            RepoStatus::Error => self.error += 1,
        }
    }
}

/// Abstract formatter for rendering output.
pub trait OutputFormatter: Send + Sync {
    fn render_header(&self, command: &[String], count: usize);
    fn render_repo(&self, result: &RepoResult, config: &MgitConfig);
    fn render_summary(&self, stats: &SummaryStats, config: &MgitConfig);
}

/// Human-readable terminal output formatter with ANSI color support.
pub struct TerminalFormatter<W: Write + Send + Sync> {
    writer: Arc<Mutex<W>>,
}

impl<W: Write + Send + Sync> TerminalFormatter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

impl<W: Write + Send + Sync> OutputFormatter for TerminalFormatter<W> {
    fn render_header(&self, command: &[String], count: usize) {
        let mut out = self.writer.lock().unwrap();
        let _ = writeln!(out, "Running 'git {}' on {} projects.", command.join(" "), count);
    }

    fn render_repo(&self, result: &RepoResult, config: &MgitConfig) {
        if config.summary_only {
            return;
        }

        // Apply filtering
        if config.only_changes && !result.has_changes && result.status == RepoStatus::Success {
            return;
        }

        if config.quiet && result.status == RepoStatus::Success && result.stdout.trim().is_empty() && result.stderr.trim().is_empty() {
            return;
        }

        let mut out = self.writer.lock().unwrap();

        let header_style = match result.status {
            RepoStatus::Success => Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))).bold(),
            RepoStatus::Failure => Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow))).bold(),
            RepoStatus::Error => Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red))).bold(),
        };

        let _ = writeln!(out, "{header_style}{}{header_style:#}", result.repo);

        if !result.stdout.is_empty() {
            let _ = write!(out, "{}", result.stdout);
        }
        if !result.stderr.is_empty() {
            let _ = write!(out, "{}", result.stderr);
        }
    }

    fn render_summary(&self, stats: &SummaryStats, _config: &MgitConfig) {
        let mut out = self.writer.lock().unwrap();
        let green = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))).bold();
        let yellow = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow))).bold();
        let red = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red))).bold();

        let _ = writeln!(out);
        let _ = write!(out, "{green}Success: {}{green:#}", stats.success);

        if stats.failure > 0 {
            let _ = write!(out, ", {yellow}Warnings: {}{yellow:#}", stats.failure);
        }

        if stats.error > 0 {
            let _ = write!(out, ", {red}Errors: {}{red:#}", stats.error);
        }

        let _ = writeln!(out);
    }
}

/// Structured JSON output document for AI agents and automated workflows.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonReport {
    pub command: Vec<String>,
    pub total_repos: usize,
    pub summary: SummaryStats,
    pub results: Vec<RepoResult>,
}

/// Formatter that buffers all in-order results and emits a single valid JSON document.
pub struct JsonFormatter<W: Write + Send + Sync> {
    command: Vec<String>,
    results: Arc<Mutex<Vec<RepoResult>>>,
    writer: Arc<Mutex<W>>,
}

impl<W: Write + Send + Sync> JsonFormatter<W> {
    pub fn new(command: Vec<String>, writer: W) -> Self {
        Self {
            command,
            results: Arc::new(Mutex::new(Vec::new())),
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

impl<W: Write + Send + Sync> OutputFormatter for JsonFormatter<W> {
    fn render_header(&self, _command: &[String], _count: usize) {
        // Headers are omitted in JSON mode so stdout remains clean parsable JSON
    }

    fn render_repo(&self, result: &RepoResult, config: &MgitConfig) {
        if config.only_changes && !result.has_changes && result.status == RepoStatus::Success {
            return;
        }
        if config.quiet && result.status == RepoStatus::Success && result.stdout.trim().is_empty() && result.stderr.trim().is_empty() {
            return;
        }

        let mut res = self.results.lock().unwrap();
        res.push(result.clone());
    }

    fn render_summary(&self, stats: &SummaryStats, config: &MgitConfig) {
        let results = if config.summary_only {
            Vec::new()
        } else {
            self.results.lock().unwrap().clone()
        };

        let report = JsonReport {
            command: self.command.clone(),
            total_repos: stats.total,
            summary: stats.clone(),
            results,
        };

        let mut out = self.writer.lock().unwrap();
        let _ = serde_json::to_writer_pretty(&mut *out, &report);
        let _ = writeln!(out);
    }
}

/// Consumes completed repository results from workers and guarantees strict in-order sequential output.
pub async fn stream_in_order(
    total: usize,
    mut rx: mpsc::Receiver<RepoResult>,
    config: &MgitConfig,
    formatter: &dyn OutputFormatter,
) -> SummaryStats {
    let mut pending = HashMap::<usize, RepoResult>::new();
    let mut next_expected_id = 0;
    let mut stats = SummaryStats::default();

    while next_expected_id < total {
        // Check if next item is already in pending buffer
        if let Some(result) = pending.remove(&next_expected_id) {
            stats.record(&result);
            formatter.render_repo(&result, config);
            next_expected_id += 1;
            continue;
        }

        // Await next item from receiver
        match rx.recv().await {
            Some(result) => {
                if result.id == next_expected_id {
                    stats.record(&result);
                    formatter.render_repo(&result, config);
                    next_expected_id += 1;
                } else {
                    pending.insert(result.id, result);
                }
            }
            None => break, // Channel closed
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_result(id: usize, repo: &str, status: RepoStatus, stdout: &str) -> RepoResult {
        RepoResult {
            id,
            repo: repo.to_string(),
            status,
            exit_code: match status {
                RepoStatus::Success => Some(0),
                RepoStatus::Failure => Some(1),
                RepoStatus::Error => None,
            },
            stdout: stdout.to_string(),
            stderr: String::new(),
            has_changes: !stdout.trim().is_empty(),
            elapsed_ms: 50,
        }
    }

    #[tokio::test]
    async fn test_in_order_streaming_out_of_order_arrival() {
        let (tx, rx) = mpsc::channel(10);
        let config = MgitConfig::default();

        // Send out of order: 2, 0, 1
        tokio::spawn(async move {
            tx.send(dummy_result(2, "repo_c", RepoStatus::Success, "c")).await.unwrap();
            tx.send(dummy_result(0, "repo_a", RepoStatus::Success, "a")).await.unwrap();
            tx.send(dummy_result(1, "repo_b", RepoStatus::Success, "b")).await.unwrap();
        });

        struct BufferFormatter(Arc<Mutex<Vec<String>>>);
        impl OutputFormatter for BufferFormatter {
            fn render_header(&self, _: &[String], _: usize) {}
            fn render_repo(&self, result: &RepoResult, _: &MgitConfig) {
                self.0.lock().unwrap().push(result.repo.clone());
            }
            fn render_summary(&self, _: &SummaryStats, _: &MgitConfig) {}
        }

        let recorded = Arc::new(Mutex::new(Vec::new()));
        let formatter = BufferFormatter(recorded.clone());

        let stats = stream_in_order(3, rx, &config, &formatter).await;
        assert_eq!(stats.total, 3);
        assert_eq!(stats.success, 3);

        let final_order = recorded.lock().unwrap().clone();
        assert_eq!(final_order, vec!["repo_a", "repo_b", "repo_c"]);
    }

    #[test]
    fn test_terminal_formatter_quiet_mode() {
        let buffer = Vec::new();
        let formatter = TerminalFormatter::new(buffer);
        let mut config = MgitConfig::default();
        config.quiet = true;

        // Clean success repo -> should be suppressed
        let clean = dummy_result(0, "clean_repo", RepoStatus::Success, "");
        formatter.render_repo(&clean, &config);

        let out = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(out.is_empty());

        // Repo with error -> should be rendered
        let failed = dummy_result(1, "failed_repo", RepoStatus::Failure, "error: rejected");
        formatter.render_repo(&failed, &config);

        let out_after_fail = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(out_after_fail.contains("failed_repo"));
        assert!(out_after_fail.contains("error: rejected"));
    }

    #[test]
    fn test_terminal_formatter_only_changes_mode() {
        let buffer = Vec::new();
        let formatter = TerminalFormatter::new(buffer);
        let mut config = MgitConfig::default();
        config.only_changes = true;

        // Clean repo -> suppressed
        let clean = dummy_result(0, "clean_repo", RepoStatus::Success, "");
        formatter.render_repo(&clean, &config);

        let out = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(out.is_empty());

        // Changed repo -> rendered
        let changed = dummy_result(1, "changed_repo", RepoStatus::Success, " M file.rs\n");
        formatter.render_repo(&changed, &config);

        let out_after_change = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(out_after_change.contains("changed_repo"));
        assert!(out_after_change.contains("M file.rs"));
    }

    #[test]
    fn test_terminal_formatter_failure_and_error() {
        let buffer = Vec::new();
        let formatter = TerminalFormatter::new(buffer);
        let config = MgitConfig::default();

        let fail = dummy_result(0, "failed_repo", RepoStatus::Failure, "");
        let err = dummy_result(1, "error_repo", RepoStatus::Error, "command failed");

        formatter.render_repo(&fail, &config);
        formatter.render_repo(&err, &config);

        let out = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(out.contains("failed_repo"));
        assert!(out.contains("error_repo"));
        assert!(out.contains("command failed"));

        let stats = SummaryStats {
            total: 2,
            success: 0,
            failure: 1,
            error: 1,
            elapsed_ms: 100,
        };
        formatter.render_summary(&stats, &config);
        let final_out = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(final_out.contains("Warnings: 1"));
        assert!(final_out.contains("Errors: 1"));
    }

    #[test]
    fn test_json_formatter_valid_schema() {
        let buffer = Vec::new();
        let formatter = JsonFormatter::new(vec!["status".to_string(), "-s".to_string()], buffer);
        let config = MgitConfig::default();

        let res1 = dummy_result(0, "core", RepoStatus::Success, " M Cargo.toml\n");
        let res2 = dummy_result(1, "cli", RepoStatus::Success, "");

        formatter.render_repo(&res1, &config);
        formatter.render_repo(&res2, &config);

        let stats = SummaryStats {
            total: 2,
            success: 2,
            failure: 0,
            error: 0,
            elapsed_ms: 120,
        };
        formatter.render_summary(&stats, &config);

        let json_str = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        let report: JsonReport = serde_json::from_str(&json_str).unwrap();

        assert_eq!(report.command, vec!["status", "-s"]);
        assert_eq!(report.total_repos, 2);
        assert_eq!(report.summary.success, 2);
        assert_eq!(report.results.len(), 2);
        assert_eq!(report.results[0].repo, "core");
        assert!(report.results[0].has_changes);
    }

    #[test]
    fn test_summary_only_mode() {
        let buffer = Vec::new();
        let formatter = TerminalFormatter::new(buffer);
        let mut config = MgitConfig::default();
        config.summary_only = true;

        let res = dummy_result(0, "my_repo", RepoStatus::Success, "output");
        formatter.render_repo(&res, &config);

        let out = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(out.is_empty()); // Suppressed

        let stats = SummaryStats {
            total: 1,
            success: 1,
            failure: 0,
            error: 0,
            elapsed_ms: 50,
        };
        formatter.render_summary(&stats, &config);

        let summary_out = String::from_utf8(formatter.writer.lock().unwrap().clone()).unwrap();
        assert!(summary_out.contains("Success: 1"));
    }
}
