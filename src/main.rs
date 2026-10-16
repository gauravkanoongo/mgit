pub mod args;
pub mod discovery;
pub mod engine;
pub mod output;

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

    // Use the coordinated live status line only for interactive human-readable output.
    use std::io::IsTerminal;
    let is_interactive = std::io::stdout().is_terminal() && !config.json && !config.quiet && !config.summary_only;
    let progress = if is_interactive {
        Some(output::create_progress_bar(total))
    } else {
        None
    };

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

    let stats = if let Some(progress) = progress.as_ref() {
        let (tx, rx) = mpsc::unbounded_channel();
        let worker_handle = tokio::spawn(run_engine_streaming(
            repos,
            git_args.clone(),
            config.clone(),
            cancel_token.clone(),
            tx,
        ));
        let stats = stream_events_in_order(rx, &config, formatter.as_ref(), progress).await;
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
