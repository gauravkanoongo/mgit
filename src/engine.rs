use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::args::MgitConfig;
use crate::discovery::RepoEntry;
use crate::output::{RepoEvent, RepoResult, RepoStatus};

async fn read_and_stream<R>(
    reader: R,
    id: usize,
    retain_output: bool,
    tx: mpsc::UnboundedSender<RepoEvent>,
) -> Vec<u8>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut reader = reader;
    let mut output = Vec::new();
    let mut line = Vec::new();
    let mut chunk = [0; 8192];
    let mut previous_was_cr = false;

    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(count) => {
                if retain_output {
                    output.extend_from_slice(&chunk[..count]);
                }
                for byte in &chunk[..count] {
                    if previous_was_cr {
                        previous_was_cr = false;
                        if *byte == b'\n' {
                            continue;
                        }
                    }

                    if *byte == b'\r' || *byte == b'\n' {
                        let text = String::from_utf8_lossy(&line).into_owned();
                        let _ = tx.send(RepoEvent::Output { id, text });
                        line.clear();
                        previous_was_cr = *byte == b'\r';
                    } else {
                        line.push(*byte);
                    }
                }
            }
            Err(_) => break,
        }
    }

    if !line.is_empty() {
        let text = String::from_utf8_lossy(&line).into_owned();
        let _ = tx.send(RepoEvent::Output { id, text });
    }

    output
}

/// Executes a repository command and streams each output line as it is read.
pub async fn execute_repo_streaming(
    entry: RepoEntry,
    git_args: &[String],
    cancel_token: CancellationToken,
    retain_output: bool,
    tx: mpsc::UnboundedSender<RepoEvent>,
) -> RepoResult {
    let start_time = Instant::now();
    let id = entry.id;
    let repo_display = entry.path.display().to_string();
    let _ = tx.send(RepoEvent::Started {
        id,
        repo: repo_display.clone(),
    });

    let make_error = |stderr: String| RepoResult {
        id,
        repo: repo_display.clone(),
        status: RepoStatus::Error,
        exit_code: None,
        stdout: String::new(),
        stderr,
        has_changes: false,
        elapsed_ms: start_time.elapsed().as_millis() as u64,
    };

    if cancel_token.is_cancelled() {
        let result = make_error("Cancelled due to fail-fast or abort signal".to_string());
        let _ = tx.send(RepoEvent::Output {
            id,
            text: result.stderr.clone(),
        });
        let _ = tx.send(RepoEvent::Finished(result.clone()));
        return result;
    }

    let spawn_res = Command::new("git")
        .current_dir(&entry.path)
        .args(git_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match spawn_res {
        Ok(child) => child,
        Err(err) => {
            let result = make_error(err.to_string());
            let _ = tx.send(RepoEvent::Output {
                id,
                text: result.stderr.clone(),
            });
            let _ = tx.send(RepoEvent::Finished(result.clone()));
            return result;
        }
    };

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_tx = tx.clone();
    let stderr_tx = tx.clone();
    let stdout_task = tokio::spawn(async move {
        read_and_stream(stdout, id, retain_output, stdout_tx).await
    });
    let stderr_task = tokio::spawn(async move {
        read_and_stream(stderr, id, retain_output, stderr_tx).await
    });

    let exit_status = tokio::select! {
        _ = cancel_token.cancelled() => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            let result = make_error("Killed due to fail-fast or abort signal".to_string());
            let _ = tx.send(RepoEvent::Output {
                id,
                text: result.stderr.clone(),
            });
            let _ = tx.send(RepoEvent::Finished(result.clone()));
            return result;
        }
        status_res = child.wait() => status_res,
    };

    let raw_stdout = stdout_task.await.unwrap_or_default();
    let raw_stderr = stderr_task.await.unwrap_or_default();
    let elapsed_ms = start_time.elapsed().as_millis() as u64;

    let result = match exit_status {
        Ok(status) => {
            let repo_status = match status.code() {
                Some(0) => RepoStatus::Success,
                Some(_) => RepoStatus::Failure,
                None => RepoStatus::Error,
            };
            let stdout = String::from_utf8_lossy(&raw_stdout).into_owned();
            let stderr = String::from_utf8_lossy(&raw_stderr).into_owned();
            let has_changes = !stdout.trim().is_empty();

            RepoResult {
                id,
                repo: repo_display,
                status: repo_status,
                exit_code: status.code(),
                stdout,
                stderr,
                has_changes,
                elapsed_ms,
            }
        }
        Err(err) => {
            let error_text = err.to_string();
            let _ = tx.send(RepoEvent::Output {
                id,
                text: error_text.clone(),
            });
            RepoResult {
                id,
                repo: repo_display,
                status: RepoStatus::Error,
                exit_code: None,
                stdout: String::from_utf8_lossy(&raw_stdout).into_owned(),
                stderr: format!("{}{}", String::from_utf8_lossy(&raw_stderr), error_text),
                has_changes: !raw_stdout.is_empty(),
                elapsed_ms,
            }
        }
    };

    let _ = tx.send(RepoEvent::Finished(result.clone()));
    result
}

/// Executes a single git command on a repository asynchronously.
///
/// Respects the `CancellationToken` for cooperative aborts (`--fail-fast`).
pub async fn execute_repo(
    entry: RepoEntry,
    git_args: &[String],
    cancel_token: CancellationToken,
) -> RepoResult {
    let start_time = Instant::now();
    let repo_display = entry.path.display().to_string();

    // Check cancellation before spawning
    if cancel_token.is_cancelled() {
        return RepoResult {
            id: entry.id,
            repo: repo_display,
            status: RepoStatus::Error,
            exit_code: None,
            stdout: String::new(),
            stderr: "Cancelled due to fail-fast or abort signal".to_string(),
            has_changes: false,
            elapsed_ms: start_time.elapsed().as_millis() as u64,
        };
    }

    // Spawn non-blocking subprocess
    let spawn_res = Command::new("git")
        .current_dir(&entry.path)
        .args(git_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match spawn_res {
        Ok(child) => child,
        Err(err) => {
            return RepoResult {
                id: entry.id,
                repo: repo_display,
                status: RepoStatus::Error,
                exit_code: None,
                stdout: String::new(),
                stderr: err.to_string(),
                has_changes: false,
                elapsed_ms: start_time.elapsed().as_millis() as u64,
            };
        }
    };

    let mut stdout_handle = child.stdout.take();
    let mut stderr_handle = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut stream) = stdout_handle.take() {
            use tokio::io::AsyncReadExt;
            let _ = stream.read_to_end(&mut buf).await;
        }
        buf
    });

    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut stream) = stderr_handle.take() {
            use tokio::io::AsyncReadExt;
            let _ = stream.read_to_end(&mut buf).await;
        }
        buf
    });

    // Await process exit cooperatively with cancellation
    let exit_status = tokio::select! {
        _ = cancel_token.cancelled() => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            return RepoResult {
                id: entry.id,
                repo: repo_display,
                status: RepoStatus::Error,
                exit_code: None,
                stdout: String::new(),
                stderr: "Killed due to fail-fast or abort signal".to_string(),
                has_changes: false,
                elapsed_ms: start_time.elapsed().as_millis() as u64,
            };
        }
        status_res = child.wait() => status_res,
    };

    let raw_stdout = stdout_task.await.unwrap_or_default();
    let raw_stderr = stderr_task.await.unwrap_or_default();

    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    match exit_status {
        Ok(status) => {
            let repo_status = match status.code() {
                Some(0) => RepoStatus::Success,
                Some(_) => RepoStatus::Failure,
                None => RepoStatus::Error,
            };
            let stdout = String::from_utf8_lossy(&raw_stdout).into_owned();
            let stderr = String::from_utf8_lossy(&raw_stderr).into_owned();
            let has_changes = !stdout.trim().is_empty();

            RepoResult {
                id: entry.id,
                repo: repo_display,
                status: repo_status,
                exit_code: status.code(),
                stdout,
                stderr,
                has_changes,
                elapsed_ms,
            }
        }
        Err(err) => RepoResult {
            id: entry.id,
            repo: repo_display,
            status: RepoStatus::Error,
            exit_code: None,
            stdout: String::new(),
            stderr: err.to_string(),
            has_changes: false,
            elapsed_ms,
        },
    }
}

/// Coordinates parallel execution across all discovered repositories using Tokio.
///
/// Limits concurrency with a semaphore and sends each completed result to `tx`.
pub async fn run_engine(
    repos: Vec<RepoEntry>,
    git_args: Arc<Vec<String>>,
    config: Arc<MgitConfig>,
    cancel_token: CancellationToken,
    tx: mpsc::Sender<RepoResult>,
) {
    let semaphore = Arc::new(Semaphore::new(config.jobs));

    for repo in repos {
        if cancel_token.is_cancelled() {
            break;
        }

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => break, // Semaphore closed
        };

        let git_args = git_args.clone();
        let config = config.clone();
        let cancel_token = cancel_token.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let result = execute_repo(repo, &git_args, cancel_token.clone()).await;

            if config.fail_fast && result.status != RepoStatus::Success {
                cancel_token.cancel();
            }

            let _ = tx.send(result).await;
            drop(permit);
        });
    }
}

/// Coordinates concurrent execution while streaming output events for interactive terminals.
pub async fn run_engine_streaming(
    repos: Vec<RepoEntry>,
    git_args: Arc<Vec<String>>,
    config: Arc<MgitConfig>,
    cancel_token: CancellationToken,
    tx: mpsc::UnboundedSender<RepoEvent>,
) {
    let semaphore = Arc::new(Semaphore::new(config.jobs));

    for repo in repos {
        if cancel_token.is_cancelled() {
            break;
        }

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => break,
        };
        if cancel_token.is_cancelled() {
            drop(permit);
            break;
        }

        let git_args = git_args.clone();
        let config = config.clone();
        let cancel_token = cancel_token.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let result =
                execute_repo_streaming(
                    repo,
                    &git_args,
                    cancel_token.clone(),
                    config.only_changes,
                    tx.clone(),
                )
                .await;
            if config.fail_fast && result.status != RepoStatus::Success {
                cancel_token.cancel();
            }
            drop(permit);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    fn init_temp_git_repo(path: &PathBuf) {
        fs::create_dir_all(path).unwrap();
        let status = std::process::Command::new("git")
            .arg("init")
            .current_dir(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_streaming_reader_emits_carriage_return_updates() {
        use tokio::io::AsyncWriteExt;

        let (mut writer, reader) = tokio::io::duplex(32);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let writer_task = tokio::spawn(async move {
            writer.write_all(b"one\rtwo\r\nthree").await.unwrap();
        });

        let output = read_and_stream(reader, 7, true, tx).await;
        writer_task.await.unwrap();

        let mut lines = Vec::new();
        while let Some(RepoEvent::Output { text, .. }) = rx.recv().await {
            lines.push(text);
        }

        assert_eq!(output, b"one\rtwo\r\nthree".to_vec());
        assert_eq!(lines, vec!["one", "two", "three"]);
    }

    #[tokio::test]
    async fn test_execute_git_status_success() {
        let temp = tempdir().unwrap();
        init_temp_git_repo(&temp.path().to_path_buf());

        let entry = RepoEntry {
            id: 0,
            path: temp.path().to_path_buf(),
        };
        let cancel_token = CancellationToken::new();
        let git_args = vec!["status".to_string(), "-s".to_string()];

        let result = execute_repo(entry, &git_args, cancel_token).await;
        assert_eq!(result.id, 0);
        assert_eq!(result.status, RepoStatus::Success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_execute_git_failure_status() {
        let temp = tempdir().unwrap();
        init_temp_git_repo(&temp.path().to_path_buf());

        let entry = RepoEntry {
            id: 1,
            path: temp.path().to_path_buf(),
        };
        let cancel_token = CancellationToken::new();
        let git_args = vec!["checkout".to_string(), "branch-does-not-exist-12345".to_string()];

        let result = execute_repo(entry, &git_args, cancel_token).await;
        assert_eq!(result.id, 1);
        assert_eq!(result.status, RepoStatus::Failure);
        assert!(result.exit_code.unwrap() != 0);
        assert!(!result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_execute_invalid_directory_error() {
        let entry = RepoEntry {
            id: 2,
            path: PathBuf::from("non_existent_dir_0987654321"),
        };
        let cancel_token = CancellationToken::new();
        let git_args = vec!["status".to_string()];

        let result = execute_repo(entry, &git_args, cancel_token).await;
        assert_eq!(result.id, 2);
        assert_eq!(result.status, RepoStatus::Error);
    }

    #[tokio::test]
    async fn test_semaphore_concurrency_bounding() {
        let max_jobs = 2;
        let semaphore = Arc::new(Semaphore::new(max_jobs));
        let active_count = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let sem = semaphore.clone();
            let active = active_count.clone();
            let max_obs = max_observed.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_obs.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        assert!(max_observed.load(Ordering::SeqCst) <= max_jobs);
    }

    #[tokio::test]
    async fn test_fail_fast_cancellation() {
        let temp1 = tempdir().unwrap();
        let temp2 = tempdir().unwrap();
        init_temp_git_repo(&temp1.path().to_path_buf());
        init_temp_git_repo(&temp2.path().to_path_buf());

        let repos = vec![
            RepoEntry { id: 0, path: temp1.path().to_path_buf() },
            RepoEntry { id: 1, path: temp2.path().to_path_buf() },
        ];

        let cancel_token = CancellationToken::new();
        let config = Arc::new(MgitConfig {
            jobs: 1, // Single worker to guarantee sequential trigger
            fail_fast: true,
            ..MgitConfig::default()
        });

        let (tx, mut rx) = mpsc::channel(10);
        // Command that will fail
        let git_args = Arc::new(vec!["checkout".to_string(), "invalid-branch-xyz".to_string()]);

        let cancel_token_clone = cancel_token.clone();
        tokio::spawn(async move {
            run_engine(repos, git_args, config, cancel_token_clone, tx).await;
        });

        // First task fails and triggers cancellation
        let first = rx.recv().await.unwrap();
        assert_eq!(first.status, RepoStatus::Failure);
        assert!(cancel_token.is_cancelled());
    }
}
