use std::fmt;
use std::path::PathBuf;

/// Error encountered during CLI argument parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum ArgsError {
    NoArguments,
    HelpRequested,
    UnknownOption(String),
    MissingValue(String),
    InvalidValue { option: String, value: String, details: String },
    Custom(String),
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoArguments => write!(f, "mgit: Please provide some arguments to pass on to git."),
            Self::HelpRequested => write!(f, "{}", help_text()),
            Self::UnknownOption(opt) => write!(f, "mgit: Unknown option after last '--': {opt}"),
            Self::MissingValue(opt) => write!(f, "mgit: Option '{opt}' requires a value"),
            Self::InvalidValue { option, value, details } => {
                write!(f, "mgit: Invalid value '{value}' for '{option}': {details}")
            }
            Self::Custom(msg) => write!(f, "mgit: {msg}"),
        }
    }
}

impl std::error::Error for ArgsError {}

/// Configuration for `mgit` execution parsed strictly after the final `--` delimiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MgitConfig {
    /// Root directory to search for git repositories (default: ".").
    pub dir: PathBuf,
    /// Maximum concurrent async jobs (default: MGIT_PARALLEL or num_cpus * 8).
    pub jobs: usize,
    /// Only display repositories with warnings, errors, or output.
    pub quiet: bool,
    /// Only display repositories with non-empty git stdout.
    pub only_changes: bool,
    /// Emit structured JSON output.
    pub json: bool,
    /// Only print the final execution summary.
    pub summary_only: bool,
    /// Abort remaining operations immediately on first error.
    pub fail_fast: bool,
    /// Maximum directory traversal depth.
    pub max_depth: Option<usize>,
}

impl Default for MgitConfig {
    fn default() -> Self {
        let default_jobs = std::env::var("MGIT_PARALLEL")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(|| num_cpus::get() * 8);

        Self {
            dir: PathBuf::from("."),
            jobs: default_jobs,
            quiet: false,
            only_changes: false,
            json: false,
            summary_only: false,
            fail_fast: false,
            max_depth: None,
        }
    }
}

/// Generates human-readable help text.
pub fn help_text() -> &'static str {
    concat!(
        "mgit ",
        env!("CARGO_PKG_VERSION"),
        "\nRun git commands across multiple repositories with in-order deterministic output.\n\n",
        "USAGE:\n",
        "    mgit <GIT_ARGS>... [-- <MGIT_OPTIONS>...]\n\n",
        "RULE:\n",
        "    All arguments before the LAST '--' are forwarded directly and untouched to git.\n",
        "    All options after the LAST '--' configure mgit behavior.\n\n",
        "MGIT OPTIONS (after last '--'):\n",
        "    -C, --dir <PATH>      Root directory to search (default: .)\n",
        "    -j, --jobs <N>        Maximum concurrent worker jobs (default: cpus * 8)\n",
        "    -q, --quiet           Only print repos with errors or non-empty output\n",
        "    --only-changes        Only print repos that have non-empty git stdout\n",
        "    --json                Output structured JSON (AI and machine-readable)\n",
        "    --summary-only        Print only the final summary statistics\n",
        "    --fail-fast           Cancel remaining running jobs on first failure\n",
        "    -d, --max-depth <N>   Limit directory traversal depth\n",
        "    -h, --help            Show this help information\n\n",
        "EXAMPLES:\n",
        "    mgit status\n",
        "    mgit checkout main -- file.txt\n",
        "    mgit status -- -q\n",
        "    mgit pull --rebase -- -j 16\n",
        "    mgit status -s -- --json --only-changes\n",
        "    mgit checkout main -- file.txt -- --quiet -j 4\n"
    )
}

/// Parses CLI arguments by splitting at the LAST `--` symbol.
///
/// Returns `(git_args, mgit_config)`.
pub fn parse_cli(argv: impl IntoIterator<Item = String>) -> Result<(Vec<String>, MgitConfig), ArgsError> {
    let args: Vec<String> = argv.into_iter().skip(1).collect();

    if args.is_empty() {
        return Err(ArgsError::NoArguments);
    }

    // Split at the LAST `--` symbol
    let (git_args, mgit_raw_args) = match args.iter().rposition(|s| s == "--") {
        Some(idx) => {
            let (git, mgit) = args.split_at(idx);
            (git.to_vec(), mgit[1..].to_vec())
        }
        None => (args, Vec::new()),
    };

    // If git_args is empty (e.g. user ran `mgit -- --help`), verify if help was requested
    let mut config = MgitConfig::default();

    if !mgit_raw_args.is_empty() {
        parse_mgit_options(&mgit_raw_args, &mut config)?;
    } else if git_args.len() == 1 && (git_args[0] == "-h" || git_args[0] == "--help") {
        return Err(ArgsError::HelpRequested);
    }

    if git_args.is_empty() {
        return Err(ArgsError::NoArguments);
    }

    Ok((git_args, config))
}

fn parse_mgit_options(raw_args: &[String], config: &mut MgitConfig) -> Result<(), ArgsError> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_args(raw_args);

    while let Some(arg) = parser.next().map_err(|e| ArgsError::Custom(e.to_string()))? {
        match arg {
            Short('C') | Long("dir") => {
                let val = parser
                    .value()
                    .map_err(|_| ArgsError::MissingValue("--dir".to_string()))?;
                config.dir = PathBuf::from(val.string().map_err(|e| ArgsError::Custom(e.to_string()))?);
            }
            Short('j') | Long("jobs") => {
                let val = parser
                    .value()
                    .map_err(|_| ArgsError::MissingValue("--jobs".to_string()))?;
                let val_str = val.string().map_err(|e| ArgsError::Custom(e.to_string()))?;
                match val_str.parse::<usize>() {
                    Ok(n) if n > 0 => config.jobs = n,
                    Ok(_) => {
                        return Err(ArgsError::InvalidValue {
                            option: "--jobs".to_string(),
                            value: val_str,
                            details: "Must be greater than 0".to_string(),
                        });
                    }
                    Err(e) => {
                        return Err(ArgsError::InvalidValue {
                            option: "--jobs".to_string(),
                            value: val_str,
                            details: e.to_string(),
                        });
                    }
                }
            }
            Short('q') | Long("quiet") => {
                config.quiet = true;
            }
            Long("only-changes") => {
                config.only_changes = true;
            }
            Long("json") => {
                config.json = true;
            }
            Long("summary-only") => {
                config.summary_only = true;
            }
            Long("fail-fast") => {
                config.fail_fast = true;
            }
            Short('d') | Long("max-depth") => {
                let val = parser
                    .value()
                    .map_err(|_| ArgsError::MissingValue("--max-depth".to_string()))?;
                let val_str = val.string().map_err(|e| ArgsError::Custom(e.to_string()))?;
                match val_str.parse::<usize>() {
                    Ok(n) => config.max_depth = Some(n),
                    Err(e) => {
                        return Err(ArgsError::InvalidValue {
                            option: "--max-depth".to_string(),
                            value: val_str,
                            details: e.to_string(),
                        });
                    }
                }
            }
            Short('h') | Long("help") => {
                return Err(ArgsError::HelpRequested);
            }
            Long(opt) => return Err(ArgsError::UnknownOption(format!("--{opt}"))),
            Short(opt) => return Err(ArgsError::UnknownOption(format!("-{opt}"))),
            Value(val) => {
                let s = val.to_string_lossy();
                return Err(ArgsError::UnknownOption(s.to_string()));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_argv(slice: &[&str]) -> Vec<String> {
        let mut v = vec!["mgit".to_string()];
        v.extend(slice.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn test_no_arguments_returns_error() {
        let argv = to_argv(&[]);
        assert_eq!(parse_cli(argv), Err(ArgsError::NoArguments));
    }

    #[test]
    fn test_pure_git_command_no_separator() {
        let argv = to_argv(&["status"]);
        let (git_args, config) = parse_cli(argv).unwrap();
        assert_eq!(git_args, vec!["status"]);
        assert_eq!(config, MgitConfig::default());
    }

    #[test]
    fn test_git_command_with_internal_separator() {
        // Under the last '--' rule: to run `git checkout main -- file.txt`, an ending `--` delimits the git payload:
        let argv = to_argv(&["checkout", "main", "--", "file.txt", "--"]);
        let (git_args, config) = parse_cli(argv).unwrap();
        assert_eq!(git_args, vec!["checkout", "main", "--", "file.txt"]);
        assert_eq!(config, MgitConfig::default());
    }

    #[test]
    fn test_separator_with_all_mgit_flags() {
        let argv = to_argv(&[
            "status",
            "--",
            "-C",
            "/custom/path",
            "-j",
            "12",
            "-q",
            "--only-changes",
            "--json",
            "--summary-only",
            "--fail-fast",
            "-d",
            "3",
        ]);
        let (git_args, config) = parse_cli(argv).unwrap();
        assert_eq!(git_args, vec!["status"]);
        assert_eq!(config.dir, PathBuf::from("/custom/path"));
        assert_eq!(config.jobs, 12);
        assert!(config.quiet);
        assert!(config.only_changes);
        assert!(config.json);
        assert!(config.summary_only);
        assert!(config.fail_fast);
        assert_eq!(config.max_depth, Some(3));
    }

    #[test]
    fn test_multiple_separators_last_takes_precedence() {
        let argv = to_argv(&["log", "--oneline", "--", "file.txt", "--", "-j", "4", "-q"]);
        let (git_args, config) = parse_cli(argv).unwrap();
        assert_eq!(git_args, vec!["log", "--oneline", "--", "file.txt"]);
        assert_eq!(config.jobs, 4);
        assert!(config.quiet);
    }

    #[test]
    fn test_invalid_mgit_flag_returns_error() {
        let argv = to_argv(&["status", "--", "--unrecognized-opt"]);
        match parse_cli(argv) {
            Err(ArgsError::UnknownOption(opt)) => assert_eq!(opt, "--unrecognized-opt"),
            other => panic!("Expected UnknownOption, got {other:?}"),
        }
    }

    #[test]
    fn test_invalid_jobs_parameter_returns_error() {
        let argv = to_argv(&["status", "--", "-j", "not-a-number"]);
        match parse_cli(argv) {
            Err(ArgsError::InvalidValue { option, value, .. }) => {
                assert_eq!(option, "--jobs");
                assert_eq!(value, "not-a-number");
            }
            other => panic!("Expected InvalidValue, got {other:?}"),
        }

        let argv_zero = to_argv(&["status", "--", "-j", "0"]);
        match parse_cli(argv_zero) {
            Err(ArgsError::InvalidValue { option, value, .. }) => {
                assert_eq!(option, "--jobs");
                assert_eq!(value, "0");
            }
            other => panic!("Expected InvalidValue for 0 jobs, got {other:?}"),
        }
    }

    #[test]
    fn test_help_flag_triggers_help_signal() {
        let argv1 = to_argv(&["--help"]);
        assert_eq!(parse_cli(argv1), Err(ArgsError::HelpRequested));

        let argv2 = to_argv(&["status", "--", "-h"]);
        assert_eq!(parse_cli(argv2), Err(ArgsError::HelpRequested));
    }

    #[test]
    fn test_mgit_parallel_env_fallback() {
        // Test parsing with an explicit environment-based config loader
        let resolve_jobs = |env_val: Option<&str>| -> usize {
            env_val
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or_else(|| num_cpus::get() * 8)
        };

        assert_eq!(resolve_jobs(Some("42")), 42);
        assert_eq!(resolve_jobs(None), num_cpus::get() * 8);
        assert_eq!(resolve_jobs(Some("invalid")), num_cpus::get() * 8);
    }

    #[test]
    fn test_display_args_error_formatting() {
        assert_eq!(
            format!("{}", ArgsError::NoArguments),
            "mgit: Please provide some arguments to pass on to git."
        );
        assert_eq!(
            format!("{}", ArgsError::UnknownOption("--foo".to_string())),
            "mgit: Unknown option after last '--': --foo"
        );
        assert_eq!(
            format!("{}", ArgsError::MissingValue("-C".to_string())),
            "mgit: Option '-C' requires a value"
        );
        assert!(format!("{}", ArgsError::HelpRequested).contains("USAGE:"));
    }
}

