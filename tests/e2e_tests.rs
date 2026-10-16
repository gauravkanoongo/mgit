use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn get_bin_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // pop test binary name
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("mgit")
}

fn init_git_repo(path: &Path) {
    fs::create_dir_all(path).unwrap();
    let status = Command::new("git")
        .arg("init")
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn test_e2e_normal_git_status() {
    let temp = tempdir().unwrap();
    let repo1 = temp.path().join("repo1");
    let repo2 = temp.path().join("repo2");
    init_git_repo(&repo1);
    init_git_repo(&repo2);

    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("status")
        .arg("--")
        .arg("-C")
        .arg(temp.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running 'git status' on 2 projects."));
    assert!(stdout.contains("Success: 2"));
}

#[test]
fn test_e2e_separator_syntax_and_quiet() {
    let temp = tempdir().unwrap();
    let repo1 = temp.path().join("repo1");
    init_git_repo(&repo1);

    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("status")
        .arg("-s")
        .arg("--")
        .arg("-C")
        .arg(temp.path())
        .arg("-q")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // In quiet mode with a clean repo, repo path should not be printed
    assert!(!stdout.contains("repo1"));
    assert!(stdout.contains("Success: 1"));
}

#[test]
fn test_e2e_json_output_machine_readable() {
    let temp = tempdir().unwrap();
    let repo1 = temp.path().join("repo_alpha");
    let repo2 = temp.path().join("repo_beta");
    init_git_repo(&repo1);
    init_git_repo(&repo2);

    // Create a modified file in repo_alpha
    let mut file = File::create(repo1.join("test.txt")).unwrap();
    writeln!(file, "hello world").unwrap();

    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("status")
        .arg("-s")
        .arg("--")
        .arg("-C")
        .arg(temp.path())
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Stdout should be valid JSON document");

    assert_eq!(parsed["total_repos"], 2);
    assert_eq!(parsed["command"], serde_json::json!(["status", "-s"]));
    assert_eq!(parsed["summary"]["success"], 2);

    let results = parsed["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);

    let alpha_res = &results[0];
    assert!(alpha_res["has_changes"].as_bool().unwrap());
    assert!(alpha_res["stdout"].as_str().unwrap().contains("test.txt"));
}

#[test]
fn test_e2e_failure_exit_code() {
    let temp = tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_git_repo(&repo);

    let bin = get_bin_path();
    // Run invalid git command
    let output = Command::new(&bin)
        .arg("checkout")
        .arg("non_existent_branch_xyz")
        .arg("--")
        .arg("-C")
        .arg(temp.path())
        .output()
        .unwrap();

    // Must return exit code 1 on failure
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Warnings: 1") || stdout.contains("Errors: 1"));
}

#[test]
fn test_e2e_only_changes_filtering() {
    let temp = tempdir().unwrap();
    let repo_clean = temp.path().join("clean_repo");
    let repo_dirty = temp.path().join("dirty_repo");
    init_git_repo(&repo_clean);
    init_git_repo(&repo_dirty);

    // Create untracked file in dirty_repo
    let mut file = File::create(repo_dirty.join("dirty.txt")).unwrap();
    writeln!(file, "dirty").unwrap();

    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("status")
        .arg("-s")
        .arg("--")
        .arg("-C")
        .arg(temp.path())
        .arg("--only-changes")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("clean_repo"));
    assert!(stdout.contains("dirty_repo"));
    assert!(stdout.contains("dirty.txt"));
}

#[test]
fn test_e2e_no_color_environment() {
    let temp = tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_git_repo(&repo);

    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("status")
        .arg("--")
        .arg("-C")
        .arg(temp.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verify no ANSI escape codes (e.g. \x1b[)
    assert!(!stdout.contains("\x1b["));
}

#[test]
fn test_e2e_tui_requires_interactive_terminals() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("status")
        .arg("--")
        .arg("--tui")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--tui requires interactive stdout and stderr terminals"));
}
