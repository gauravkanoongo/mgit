use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

use crate::args::MgitConfig;

/// A discovered repository target with a unique, deterministically assigned index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepoEntry {
    /// Deterministic 0-indexed sequence number (used to preserve in-order output).
    pub id: usize,
    /// Absolute or relative canonical path to the repository working directory.
    pub path: PathBuf,
}

/// Directory names to completely prune during recursive traversal.
const PRUNED_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".cargo",
    "dist",
    "build",
    "vendor",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
];

/// Determines if a directory entry should be pruned during traversal.
fn should_prune(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }

    if let Some(file_name) = entry.file_name().to_str() {
        PRUNED_DIR_NAMES
            .iter()
            .any(|&pruned| file_name.eq_ignore_ascii_case(pruned))
    } else {
        false
    }
}

/// Checks whether a given directory is the root of a Git repository.
///
/// Supports:
/// 1. Standard Git repositories: `.git` is a directory.
/// 2. Git worktrees and submodules: `.git` is a regular file containing `gitdir: <path>`.
fn is_git_repo(dir: &Path) -> bool {
    let git_path = dir.join(".git");
    if git_path.is_dir() {
        return true;
    }
    if git_path.is_file() {
        // Confirm it looks like a git file pointer (gitdir: ...)
        if let Ok(content) = std::fs::read_to_string(&git_path) {
            return content.trim().starts_with("gitdir:");
        }
    }
    false
}

/// Scans the filesystem according to `MgitConfig` and returns a deterministically sorted
/// list of discovered repositories with assigned sequential IDs.
pub fn discover_repos(config: &MgitConfig) -> Result<Vec<RepoEntry>, std::io::Error> {
    discover_repos_with_progress(config, |_| {})
}

/// Scans repositories and reports the cumulative discovery count after each match.
pub fn discover_repos_with_progress(
    config: &MgitConfig,
    mut on_repo_discovered: impl FnMut(usize),
) -> Result<Vec<RepoEntry>, std::io::Error> {
    let root = &config.dir;
    let mut discovered_paths = Vec::new();

    // Check if the root directory itself is a git repository
    if is_git_repo(root) {
        discovered_paths.push(root.clone());
        on_repo_discovered(discovered_paths.len());
    }

    let mut walker = WalkDir::new(root).min_depth(1).follow_links(false);

    if let Some(max_depth) = config.max_depth {
        walker = walker.max_depth(max_depth);
    }

    let mut it = walker.into_iter();
    while let Some(entry_res) = it.next() {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue, // Ignore unreadable directories / permission errors
        };

        if entry.file_type().is_dir() {
            // If this is a git repo, record it and prune descending into it
            if is_git_repo(entry.path()) {
                discovered_paths.push(entry.path().to_path_buf());
                on_repo_discovered(discovered_paths.len());
                it.skip_current_dir();
                continue;
            }

            // Prune blacklisted build / dependency directories
            if should_prune(&entry) {
                it.skip_current_dir();
            }
        }
    }

    // Sort paths alphabetically to guarantee 100% deterministic ordering across runs
    discovered_paths.sort();
    discovered_paths.dedup();

    let entries = discovered_paths
        .into_iter()
        .enumerate()
        .map(|(id, path)| RepoEntry { id, path })
        .collect();

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_discover_empty_directory() {
        let temp = tempdir().unwrap();
        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert!(repos.is_empty());
    }

    #[test]
    fn test_discover_root_is_git_repo() {
        let temp = tempdir().unwrap();
        fs::create_dir(temp.path().join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, 0);
        assert_eq!(repos[0].path, temp.path());
    }

    #[test]
    fn test_discover_single_standard_git_repo() {
        let temp = tempdir().unwrap();
        let repo_dir = temp.path().join("my-repo");
        fs::create_dir_all(repo_dir.join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, 0);
        assert_eq!(repos[0].path, repo_dir);
    }

    #[test]
    fn test_discover_worktree_submodule_git_file() {
        let temp = tempdir().unwrap();
        let worktree_dir = temp.path().join("worktree-repo");
        fs::create_dir_all(&worktree_dir).unwrap();

        let mut git_file = File::create(worktree_dir.join(".git")).unwrap();
        writeln!(
            git_file,
            "gitdir: /path/to/parent/.git/worktrees/worktree-repo"
        )
        .unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, 0);
        assert_eq!(repos[0].path, worktree_dir);
    }

    #[test]
    fn test_prune_ignored_directories() {
        let temp = tempdir().unwrap();

        // Legitimate repo
        let good_repo = temp.path().join("legit_repo");
        fs::create_dir_all(good_repo.join(".git")).unwrap();

        // Repos nested inside pruned folders (e.g. node_modules, target, .cargo)
        let node_modules_repo = temp.path().join("node_modules").join("dummy_pkg");
        fs::create_dir_all(node_modules_repo.join(".git")).unwrap();

        let target_repo = temp.path().join("target").join("debug").join("nested");
        fs::create_dir_all(target_repo.join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].path, good_repo);
    }

    #[test]
    fn test_nested_repos_handling() {
        let temp = tempdir().unwrap();
        let parent_repo = temp.path().join("parent_repo");
        fs::create_dir_all(parent_repo.join(".git")).unwrap();

        // Internal sub-repo inside parent
        let child_repo = parent_repo.join("nested_repo");
        fs::create_dir_all(child_repo.join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        // Since we skip descending into discovered git repos, only parent_repo is discovered
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].path, parent_repo);
    }

    #[test]
    fn test_deterministic_path_sorting() {
        let temp = tempdir().unwrap();

        let repo_c = temp.path().join("repo_c");
        let repo_a = temp.path().join("repo_a");
        let repo_b = temp.path().join("repo_b");

        fs::create_dir_all(repo_c.join(".git")).unwrap();
        fs::create_dir_all(repo_a.join(".git")).unwrap();
        fs::create_dir_all(repo_b.join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert_eq!(repos.len(), 3);
        assert_eq!(repos[0].id, 0);
        assert_eq!(repos[0].path, repo_a);

        assert_eq!(repos[1].id, 1);
        assert_eq!(repos[1].path, repo_b);

        assert_eq!(repos[2].id, 2);
        assert_eq!(repos[2].path, repo_c);
    }

    #[test]
    fn test_discovery_progress_reports_repository_count() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("repo_a").join(".git")).unwrap();
        fs::create_dir_all(temp.path().join("repo_b").join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            ..MgitConfig::default()
        };
        let mut reported_counts = Vec::new();

        let repos =
            discover_repos_with_progress(&config, |count| reported_counts.push(count)).unwrap();

        assert_eq!(repos.len(), 2);
        assert_eq!(reported_counts, vec![1, 2]);
    }

    #[test]
    fn test_max_depth_limiting() {
        let temp = tempdir().unwrap();

        // Level 1: <temp>/shallow
        let shallow = temp.path().join("shallow");
        fs::create_dir_all(shallow.join(".git")).unwrap();

        // Level 3: <temp>/a/b/deep
        let deep = temp.path().join("a").join("b").join("deep");
        fs::create_dir_all(deep.join(".git")).unwrap();

        let config = MgitConfig {
            dir: temp.path().to_path_buf(),
            max_depth: Some(1),
            ..MgitConfig::default()
        };

        let repos = discover_repos(&config).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].path, shallow);
    }
}
