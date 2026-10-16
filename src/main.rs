pub mod args;
pub mod discovery;
pub mod engine;
pub mod output;
pub mod ui;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::args::{parse_cli, ArgsError};
use crate::discovery::discover_repos;
use crate::engine::{run_engine, run_engine_streaming};
use crate::output::{
    stream_events_in_order, stream_in_order, JsonFormatter, OutputFormatter, TerminalFormatter,
};

#[tokio::main]
async fn main() {
    let exit_code = run().await;
    std::process::exit(exit_code);
}

async fn run() -> i32 {
    let (git_args, config) = match parse_cli(std::env::args()) {
        Ok(res) => res,
        Err(ArgsError::HelpRequested) => {
            println!("{}", args::help_text());
            return 0;
        }
        Err(err) => {
            eprintln!("{err}");
            return 2;
        }
    };

    if config.tui {
        use std::io::IsTerminal;
        if !std::io::stdout().is_terminal() || !std::io::stderr().is_terminal() {
            eprintln!("mgit: --tui requires interactive stdout and stderr terminals");
            return 2;
        }

        let cancel_token = CancellationToken::new();
        let cancel_token_ctrlc = cancel_token.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancel_token_ctrlc.cancel();
            }
        });

        let config = Arc::new(config);
        let outcome = match ui::run_ui(
            Arc::new(git_args.clone()),
            config.clone(),
            cancel_token,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(err) => {
                eprintln!("mgit: inline UI failed: {err}");
                return 1;
            }
        };

        let formatter = TerminalFormatter::new(anstream::stdout());
        formatter.render_banner(&config);
        formatter.render_header(&git_args, outcome.total);
        for captured in &outcome.results {
            formatter.render_stream_started(&captured.result.repo);
            use std::io::Write;
            if let Err(err) = std::io::stdout().write_all(&captured.stdout) {
                eprintln!("mgit: failed to replay stdout: {err}");
                return 1;
            }
            if !captured.stdout.is_empty() && !captured.stdout.ends_with(b"\n") {
                if let Err(err) = std::io::stdout().write_all(b"\n") {
                    eprintln!("mgit: failed to separate stdout output: {err}");
                    return 1;
                }
            }
            if let Err(err) = std::io::stderr().write_all(&captured.stderr) {
                eprintln!("mgit: failed to replay stderr: {err}");
                return 1;
            }
            if !captured.stderr.is_empty() && !captured.stderr.ends_with(b"\n") {
                if let Err(err) = std::io::stderr().write_all(b"\n") {
                    eprintln!("mgit: failed to separate stderr output: {err}");
                    return 1;
                }
            }
            formatter.render_stream_finished(&captured.result, &config);
        }
        formatter.render_summary(&outcome.stats, &config);

        return if outcome.cancelled || outcome.stats.failure > 0 || outcome.stats.error > 0 {
            1
        } else {
            0
        };
    }

    let repos = match discover_repos(&config) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("mgit: Error discovering repositories: {err}");
            return 1;
        }
    };

    let total = repos.len();

    // Select output formatter
    let stdout = anstream::stdout();
    let formatter: Box<dyn OutputFormatter> = if config.json {
        Box::new(JsonFormatter::new(git_args.clone(), stdout))
    } else {
        Box::new(TerminalFormatter::new(stdout))
    };

    formatter.render_banner(&config);
    formatter.render_header(&git_args, total);

    if total == 0 {
        return 0;
    }

    // Stream output as it arrives only for interactive human-readable output.
    use std::io::IsTerminal;
    let is_interactive = std::io::stdout().is_terminal() && !config.json && !config.quiet && !config.summary_only;

    let cancel_token = CancellationToken::new();

    // Hook up Ctrl+C cancellation
    let cancel_token_ctrlc = cancel_token.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancel_token_ctrlc.cancel();
        }
    });

    let git_args = Arc::new(git_args);
    let config = Arc::new(config);

    let stats = if is_interactive {
        let (tx, rx) = mpsc::unbounded_channel();
        let worker_handle = tokio::spawn(run_engine_streaming(
            repos,
            git_args.clone(),
            config.clone(),
            cancel_token.clone(),
            tx,
            false,
        ));
        let stats = stream_events_in_order(rx, &config, formatter.as_ref()).await;
        let _ = worker_handle.await;
        stats
    } else {
        let (tx, rx) = mpsc::channel(config.jobs.max(1) * 2);
        let worker_handle = tokio::spawn(run_engine(
            repos,
            git_args.clone(),
            config.clone(),
            cancel_token.clone(),
            tx,
        ));
        let stats = stream_in_order(total, rx, &config, formatter.as_ref()).await;
        let _ = worker_handle.await;
        stats
    };

    formatter.render_summary(&stats, &config);

    if stats.failure > 0 || stats.error > 0 {
        1
    } else {
        0
    }
}
