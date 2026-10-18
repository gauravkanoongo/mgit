pub mod args;
pub mod discovery;
pub mod engine;
pub mod output;

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::args::{ArgsError, parse_cli, version_text};
use crate::discovery::{RepoEntry, discover_repos_with_progress};
use crate::engine::run_engine;
use crate::output::{JsonFormatter, OutputFormatter, TerminalFormatter, stream_in_order};

fn main() {
    let (git_args, config) = match parse_cli(std::env::args()) {
        Ok(res) => res,
        Err(ArgsError::HelpRequested) => {
            println!("{}", args::help_text());
            return;
        }
        Err(ArgsError::VersionRequested) => {
            println!("{}", version_text());
            return;
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.jobs)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("mgit: Error creating async runtime: {err}");
            std::process::exit(1);
        }
    };
    let exit_code = runtime.block_on(run(git_args, config));
    std::process::exit(exit_code);
}

async fn run(git_args: Vec<String>, config: crate::args::MgitConfig) -> i32 {
    let repos = match discover_repositories(&config).await {
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

    let cancel_token = CancellationToken::new();

    // Hook up Ctrl+C cancellation
    let cancel_token_ctrlc = cancel_token.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancel_token_ctrlc.cancel();
        }
    });

    let (tx, rx) = mpsc::channel(config.jobs.max(1) * 2);
    let git_args = Arc::new(git_args);
    let config = Arc::new(config);

    // Spawn async worker pool
    let worker_handle = tokio::spawn(run_engine(
        repos,
        git_args.clone(),
        config.clone(),
        cancel_token.clone(),
        tx,
    ));

    // Stream and render results in strict sequential order
    let stats = stream_in_order(total, rx, &config, formatter.as_ref()).await;
    let _ = worker_handle.await;

    formatter.render_summary(&stats, &config);

    if stats.failure > 0 || stats.error > 0 {
        1
    } else {
        0
    }
}

async fn discover_repositories(config: &crate::args::MgitConfig) -> Result<Vec<RepoEntry>, String> {
    let progress_enabled = !config.json && io::stderr().is_terminal();
    let discovered_count = Arc::new(AtomicUsize::new(0));
    let count_for_worker = discovered_count.clone();
    let config_for_worker = config.clone();
    let mut discovery = tokio::task::spawn_blocking(move || {
        discover_repos_with_progress(&config_for_worker, |count| {
            count_for_worker.store(count, Ordering::Relaxed);
        })
    });

    if !progress_enabled {
        return discovery
            .await
            .map_err(|err| format!("discovery worker failed: {err}"))?
            .map_err(|err| err.to_string());
    }

    let mut stderr = io::stderr();
    let mut progress_error =
        write_discovery_progress(&mut stderr, 0, discovered_count.load(Ordering::Relaxed)).err();

    let mut frame = 0;
    let mut ticker = tokio::time::interval(Duration::from_millis(120));
    ticker.tick().await;
    loop {
        tokio::select! {
            result = &mut discovery => {
                let count = discovered_count.load(Ordering::Relaxed);
                if let Some(write_error) = progress_error {
                    return Err(discovery_status_write_error(write_error, result));
                }
                match result {
                    Ok(Ok(repos)) => {
                        write_discovery_complete(&mut stderr, repos.len())
                            .map_err(|err| format!("could not write discovery status: {err}"))?;
                        return Ok(repos);
                    }
                    Ok(Err(err)) => {
                        write_discovery_failed(&mut stderr, count)
                            .map_err(|write_err| format!("discovery failed: {err}; could not write status: {write_err}"))?;
                        return Err(err.to_string());
                    }
                    Err(err) => {
                        write_discovery_failed(&mut stderr, count)
                            .map_err(|write_err| format!("discovery worker failed: {err}; could not write status: {write_err}"))?;
                        return Err(format!("discovery worker failed: {err}"));
                    }
                }
            }
            _ = ticker.tick(), if progress_error.is_none() => {
                frame += 1;
                progress_error = write_discovery_progress(
                    &mut stderr,
                    frame,
                    discovered_count.load(Ordering::Relaxed),
                )
                .err();
            }
        }
    }
}

fn discovery_status_write_error(
    write_error: io::Error,
    worker_result: Result<Result<Vec<RepoEntry>, io::Error>, tokio::task::JoinError>,
) -> String {
    match worker_result {
        Ok(Ok(_)) => format!("could not write discovery status: {write_error}"),
        Ok(Err(discovery_error)) => format!(
            "could not write discovery status: {write_error}; discovery also failed: {discovery_error}"
        ),
        Err(join_error) => format!(
            "could not write discovery status: {write_error}; discovery worker failed: {join_error}"
        ),
    }
}

fn write_discovery_progress(
    writer: &mut impl Write,
    frame: usize,
    discovered_count: usize,
) -> io::Result<()> {
    let dots = [".", "..", "...", "...."][frame % 4];
    write!(
        writer,
        "\rDiscovering repositories{dots:<4} {discovered_count} found"
    )?;
    writer.flush()
}

fn write_discovery_complete(writer: &mut impl Write, discovered_count: usize) -> io::Result<()> {
    write!(
        writer,
        "\rDiscovery complete: {discovered_count} repositories found.    \n"
    )?;
    writer.flush()
}

fn write_discovery_failed(writer: &mut impl Write, discovered_count: usize) -> io::Result<()> {
    write!(
        writer,
        "\rDiscovery failed after finding {discovered_count} repositories.    \n"
    )?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_status_shows_count_and_retains_completion_line() {
        let mut output = Vec::new();
        write_discovery_progress(&mut output, 0, 3).unwrap();
        write_discovery_complete(&mut output, 3).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Discovering repositories.    3 found"));
        assert!(output.ends_with("Discovery complete: 3 repositories found.    \n"));
    }
}
