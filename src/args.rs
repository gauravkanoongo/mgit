use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use toml_edit::{DocumentMut, value};

/// Error encountered during CLI argument parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum ArgsError {
    NoArguments,
    HelpRequested,
    VersionRequested,
    UnknownOption(String),
    MissingValue(String),
    InvalidValue {
        option: String,
        value: String,
        details: String,
    },
    Custom(String),
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoArguments => {
                write!(f, "mgit: Please provide some arguments to pass on to git.")
            }
            Self::HelpRequested => write!(f, "{}", help_text()),
            Self::VersionRequested => write!(f, "{}", version_text()),
            Self::UnknownOption(opt) => write!(f, "mgit: Unknown option after last '--': {opt}"),
            Self::MissingValue(opt) => write!(f, "mgit: Option '{opt}' requires a value"),
            Self::InvalidValue {
                option,
                value,
                details,
            } => {
                write!(f, "mgit: Invalid value '{value}' for '{option}': {details}")
            }
            Self::Custom(msg) => write!(f, "mgit: {msg}"),
        }
    }
}

impl std::error::Error for ArgsError {}

pub const VERSION: &str = env!("MGIT_BUILD_VERSION");

/// Configuration for `mgit` execution parsed strictly after the final `--` delimiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MgitConfig {
    /// Root directory to search for git repositories (default: ".").
    pub dir: PathBuf,
    /// Maximum concurrent Git commands and Tokio worker threads (default: MGIT_PARALLEL or 2).
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
    /// Display execution duration timings.
    pub timer: bool,
    /// Suppress ASCII banner.
    pub no_banner: bool,
    /// Save mgit options explicitly supplied on this invocation.
    pub save_config: bool,
    pub(crate) explicit_options: BTreeSet<ConfigOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConfigOption {
    Dir,
    Jobs,
    Quiet,
    OnlyChanges,
    Json,
    SummaryOnly,
    FailFast,
    MaxDepth,
    Timer,
    NoBanner,
}

impl Default for MgitConfig {
    fn default() -> Self {
        let mut config = Self::defaults();
        apply_environment_overrides(&mut config, std::env::var("MGIT_PARALLEL").ok().as_deref());
        config
    }
}

impl MgitConfig {
    fn defaults() -> Self {
        Self {
            dir: PathBuf::from("."),
            jobs: 2,
            quiet: false,
            only_changes: false,
            json: false,
            summary_only: false,
            fail_fast: false,
            max_depth: None,
            timer: false,
            no_banner: false,
            save_config: false,
            explicit_options: BTreeSet::new(),
        }
    }
}

fn resolve_jobs(env_value: Option<&str>) -> usize {
    env_value
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&jobs| jobs > 0)
        .unwrap_or(2)
}

pub fn version_text() -> String {
    format!("mgit {VERSION}")
}

/// Generates human-readable help text.
pub fn help_text() -> String {
    format!(
        "{}\nRun git commands across multiple repositories with in-order deterministic output.\n\n\
         USAGE:\n\
             mgit <GIT_ARGS>... [-- <MGIT_OPTIONS>...]\n\n\
         RULE:\n\
             All arguments before the LAST '--' are forwarded directly and untouched to git.\n\
             All options after the LAST '--' configure mgit behavior.\n\n\
         EXCEPTIONS:\n\
             Standalone -h/--help and -V/--version are handled by mgit without a delimiter.\n\
             Running mgit without Git arguments is an error.\n\n\
         MGIT OPTIONS (after last '--'):\n\
             -C, --dir <PATH>      Root directory to search (default: config file or .)\n\
             -j, --jobs <N>        Git command and Tokio worker thread limit (default: config, MGIT_PARALLEL, or 2)\n\
             -q, --quiet           Only print repos with errors or non-empty output\n\
             --only-changes        Only print repos that have non-empty git stdout\n\
             --json                Output structured JSON (AI and machine-readable)\n\
             --summary-only        Print only the final summary statistics\n\
             --fail-fast           Cancel remaining running jobs on first failure\n\
             --timer               Display execution duration timings\n\
             --no-banner           Suppress ASCII banner\n\
             -d, --max-depth <N>   Limit directory traversal depth\n\
             --save-config         Save mgit options explicitly given on this invocation\n\
             -h, --help            Show this help information\n\
             -V, --version         Show version information\n\n\
         EXAMPLES:\n\
             mgit status\n\
             mgit checkout main -- file.txt\n\
             mgit status -- -q\n\
             mgit pull --rebase -- -j 16\n\
             mgit status -s -- --json --only-changes\n\
             mgit checkout main -- file.txt -- --quiet -j 4\n\
             mgit status -- --jobs 8 --save-config",
        version_text()
    )
}

/// Parses CLI arguments by splitting at the LAST `--` symbol.
///
/// Returns `(git_args, mgit_config)`.
pub fn parse_cli(
    argv: impl IntoIterator<Item = String>,
) -> Result<(Vec<String>, MgitConfig), ArgsError> {
    parse_cli_inner(argv, None, true)
}

#[cfg(test)]
fn parse_cli_with_config_path(
    argv: impl IntoIterator<Item = String>,
    config_path: Option<PathBuf>,
) -> Result<(Vec<String>, MgitConfig), ArgsError> {
    parse_cli_inner(argv, config_path, false)
}

fn parse_cli_inner(
    argv: impl IntoIterator<Item = String>,
    config_path: Option<PathBuf>,
    load_user_config: bool,
) -> Result<(Vec<String>, MgitConfig), ArgsError> {
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

    if mgit_raw_args.is_empty() && git_args.len() == 1 {
        match git_args[0].as_str() {
            "-h" | "--help" => return Err(ArgsError::HelpRequested),
            "-V" | "--version" => return Err(ArgsError::VersionRequested),
            _ => {}
        }
    }

    let mut cli_config = MgitConfig::defaults();
    if !mgit_raw_args.is_empty() {
        parse_mgit_options(&mgit_raw_args, &mut cli_config)?;
    }

    if git_args.is_empty() {
        return Err(ArgsError::NoArguments);
    }

    let mut config = MgitConfig::defaults();
    let config_path = if load_user_config {
        user_config_path()
    } else {
        config_path
    };
    if let Some(path) = config_path {
        load_config_file(&path, &mut config)?;
    }
    apply_environment_overrides(&mut config, std::env::var("MGIT_PARALLEL").ok().as_deref());
    apply_cli_overrides(&mut config, &cli_config);

    Ok((git_args, config))
}

fn apply_cli_overrides(config: &mut MgitConfig, cli: &MgitConfig) {
    for option in &cli.explicit_options {
        match option {
            ConfigOption::Dir => config.dir = cli.dir.clone(),
            ConfigOption::Jobs => config.jobs = cli.jobs,
            ConfigOption::Quiet => config.quiet = cli.quiet,
            ConfigOption::OnlyChanges => config.only_changes = cli.only_changes,
            ConfigOption::Json => config.json = cli.json,
            ConfigOption::SummaryOnly => config.summary_only = cli.summary_only,
            ConfigOption::FailFast => config.fail_fast = cli.fail_fast,
            ConfigOption::MaxDepth => config.max_depth = cli.max_depth,
            ConfigOption::Timer => config.timer = cli.timer,
            ConfigOption::NoBanner => config.no_banner = cli.no_banner,
        }
    }
    config.save_config = cli.save_config;
    config.explicit_options = cli.explicit_options.clone();
}

fn apply_environment_overrides(config: &mut MgitConfig, jobs: Option<&str>) {
    if let Some(jobs) = jobs {
        config.jobs = resolve_jobs(Some(jobs));
    }
}

fn user_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|path| !path.is_empty())?;
    Some(PathBuf::from(home).join(".config").join("mgit.toml"))
}

fn load_config_file(path: &std::path::Path, config: &mut MgitConfig) -> Result<(), ArgsError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(ArgsError::Custom(format!(
                "could not read config file '{}': {err}",
                path.display()
            )));
        }
    };
    let document = contents.parse::<DocumentMut>().map_err(|err| {
        ArgsError::Custom(format!(
            "could not parse config file '{}': {err}",
            path.display()
        ))
    })?;

    if let Some(dir) = document.get("dir") {
        config.dir = PathBuf::from(config_string(dir, "dir", path)?);
    }
    if let Some(jobs) = document.get("jobs") {
        let jobs = config_integer(jobs, "jobs", path)?;
        if jobs == 0 {
            return Err(config_value_error(path, "jobs", "must be greater than 0"));
        }
        config.jobs = jobs;
    }
    if let Some(max_depth) = document.get("max_depth") {
        let max_depth = max_depth.as_integer().ok_or_else(|| {
            config_value_error(path, "max_depth", "must be a non-negative integer")
        })?;
        config.max_depth = Some(usize::try_from(max_depth).map_err(|_| {
            config_value_error(path, "max_depth", "must be a non-negative integer")
        })?);
    }
    for (key, target) in [
        ("quiet", &mut config.quiet),
        ("only_changes", &mut config.only_changes),
        ("json", &mut config.json),
        ("summary_only", &mut config.summary_only),
        ("fail_fast", &mut config.fail_fast),
        ("timer", &mut config.timer),
        ("no_banner", &mut config.no_banner),
    ] {
        if let Some(item) = document.get(key) {
            *target = item
                .as_bool()
                .ok_or_else(|| config_value_error(path, key, "must be a boolean"))?;
        }
    }
    Ok(())
}

fn config_string<'a>(
    item: &'a toml_edit::Item,
    key: &str,
    path: &std::path::Path,
) -> Result<&'a str, ArgsError> {
    item.as_str()
        .ok_or_else(|| config_value_error(path, key, "must be a string"))
}

fn config_integer(
    item: &toml_edit::Item,
    key: &str,
    path: &std::path::Path,
) -> Result<usize, ArgsError> {
    let value = item
        .as_integer()
        .ok_or_else(|| config_value_error(path, key, "must be a positive integer"))?;
    usize::try_from(value).map_err(|_| config_value_error(path, key, "is out of range"))
}

fn config_value_error(path: &std::path::Path, key: &str, details: &str) -> ArgsError {
    ArgsError::Custom(format!(
        "invalid '{key}' in config file '{}': {details}",
        path.display()
    ))
}

pub fn save_config(config: &MgitConfig) -> Result<(), ArgsError> {
    let path = user_config_path().ok_or_else(|| {
        ArgsError::Custom("could not determine the user home directory to save config".into())
    })?;
    save_config_to(&path, config)
}

fn save_config_to(path: &std::path::Path, config: &MgitConfig) -> Result<(), ArgsError> {
    let mut document = match fs::read_to_string(path) {
        Ok(contents) => contents.parse::<DocumentMut>().map_err(|err| {
            ArgsError::Custom(format!(
                "could not parse config file '{}': {err}",
                path.display()
            ))
        })?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(err) => {
            return Err(ArgsError::Custom(format!(
                "could not read config file '{}': {err}",
                path.display()
            )));
        }
    };

    for option in &config.explicit_options {
        match option {
            ConfigOption::Dir => document["dir"] = value(config.dir.to_string_lossy().as_ref()),
            ConfigOption::Jobs => {
                let jobs = i64::try_from(config.jobs).map_err(|_| {
                    ArgsError::Custom("jobs value is too large to save in TOML".into())
                })?;
                document["jobs"] = value(jobs);
            }
            ConfigOption::Quiet => document["quiet"] = value(config.quiet),
            ConfigOption::OnlyChanges => document["only_changes"] = value(config.only_changes),
            ConfigOption::Json => document["json"] = value(config.json),
            ConfigOption::SummaryOnly => document["summary_only"] = value(config.summary_only),
            ConfigOption::FailFast => document["fail_fast"] = value(config.fail_fast),
            ConfigOption::MaxDepth => {
                if let Some(max_depth) = config.max_depth {
                    let max_depth = i64::try_from(max_depth).map_err(|_| {
                        ArgsError::Custom("max depth is too large to save in TOML".into())
                    })?;
                    document["max_depth"] = value(max_depth);
                }
            }
            ConfigOption::Timer => document["timer"] = value(config.timer),
            ConfigOption::NoBanner => document["no_banner"] = value(config.no_banner),
        }
    }

    let parent = path.parent().ok_or_else(|| {
        ArgsError::Custom(format!(
            "config path '{}' has no parent directory",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        ArgsError::Custom(format!(
            "could not create config directory '{}': {err}",
            parent.display()
        ))
    })?;
    fs::write(path, document.to_string()).map_err(|err| {
        ArgsError::Custom(format!(
            "could not write config file '{}': {err}",
            path.display()
        ))
    })
}

fn parse_mgit_options(raw_args: &[String], config: &mut MgitConfig) -> Result<(), ArgsError> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_args(raw_args);

    while let Some(arg) = parser
        .next()
        .map_err(|e| ArgsError::Custom(e.to_string()))?
    {
        match arg {
            Short('C') | Long("dir") => {
                let val = parser
                    .value()
                    .map_err(|_| ArgsError::MissingValue("--dir".to_string()))?;
                config.dir =
                    PathBuf::from(val.string().map_err(|e| ArgsError::Custom(e.to_string()))?);
                config.explicit_options.insert(ConfigOption::Dir);
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
                config.explicit_options.insert(ConfigOption::Jobs);
            }
            Short('q') | Long("quiet") => {
                config.quiet = true;
                config.explicit_options.insert(ConfigOption::Quiet);
            }
            Long("only-changes") => {
                config.only_changes = true;
                config.explicit_options.insert(ConfigOption::OnlyChanges);
            }
            Long("json") => {
                config.json = true;
                config.explicit_options.insert(ConfigOption::Json);
            }
            Long("summary-only") => {
                config.summary_only = true;
                config.explicit_options.insert(ConfigOption::SummaryOnly);
            }
            Long("fail-fast") => {
                config.fail_fast = true;
                config.explicit_options.insert(ConfigOption::FailFast);
            }
            Long("timer") => {
                config.timer = true;
                config.explicit_options.insert(ConfigOption::Timer);
            }
            Long("no-banner") => {
                config.no_banner = true;
                config.explicit_options.insert(ConfigOption::NoBanner);
            }
            Long("save-config") => config.save_config = true,
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
                config.explicit_options.insert(ConfigOption::MaxDepth);
            }
            Short('h') | Long("help") => {
                return Err(ArgsError::HelpRequested);
            }
            Short('V') | Long("version") => {
                return Err(ArgsError::VersionRequested);
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
    use std::fs;
    use tempfile::tempdir;

    fn parse_cli(argv: Vec<String>) -> Result<(Vec<String>, MgitConfig), ArgsError> {
        parse_cli_with_config_path(argv, None)
    }

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
            "--timer",
            "--no-banner",
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
        assert!(config.timer);
        assert!(config.no_banner);
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
    fn test_version_flag_triggers_version_signal() {
        let argv1 = to_argv(&["--version"]);
        assert_eq!(parse_cli(argv1), Err(ArgsError::VersionRequested));

        let argv2 = to_argv(&["status", "--", "-V"]);
        assert_eq!(parse_cli(argv2), Err(ArgsError::VersionRequested));
    }

    #[test]
    fn test_help_documents_informational_exceptions_and_empty_invocation() {
        let help = help_text();
        assert!(help.contains("Standalone -h/--help and -V/--version"));
        assert!(help.contains("Running mgit without Git arguments is an error"));
        assert!(help.contains("--save-config"));
    }

    #[test]
    fn test_config_file_loads_options_and_reports_invalid_values() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("mgit.toml");
        fs::write(
            &path,
            "dir = 'workspace'\njobs = 7\nquiet = true\nmax_depth = 4\n",
        )
        .unwrap();

        let mut config = MgitConfig::defaults();
        load_config_file(&path, &mut config).unwrap();
        assert_eq!(config.dir, PathBuf::from("workspace"));
        assert_eq!(config.jobs, 7);
        assert!(config.quiet);
        assert_eq!(config.max_depth, Some(4));

        fs::write(&path, "jobs = 0\n").unwrap();
        assert!(matches!(
            load_config_file(&path, &mut config),
            Err(ArgsError::Custom(_))
        ));

        fs::write(&path, "jobs = [\n").unwrap();
        assert!(matches!(
            load_config_file(&path, &mut config),
            Err(ArgsError::Custom(_))
        ));

        let missing = temp.path().join("missing.toml");
        assert!(load_config_file(&missing, &mut config).is_ok());
    }

    #[test]
    fn test_cli_options_override_config_and_save_only_explicit_options() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("mgit.toml");
        fs::write(
            &path,
            "jobs = 3\nquiet = true\ncustom_setting = 'preserve'\n\n[exclude]\npatterns = ['old/**']\n",
        )
        .unwrap();

        let argv = to_argv(&["status", "--", "--jobs", "9", "--save-config"]);
        let (_, config) = parse_cli_with_config_path(argv, Some(path.clone())).unwrap();
        assert_eq!(config.jobs, 9);
        assert!(config.quiet);
        assert!(config.save_config);
        save_config_to(&path, &config).unwrap();

        let saved = fs::read_to_string(path).unwrap();
        let document = saved.parse::<DocumentMut>().unwrap();
        assert_eq!(document["jobs"].as_integer(), Some(9));
        assert_eq!(document["quiet"].as_bool(), Some(true));
        assert_eq!(document["custom_setting"].as_str(), Some("preserve"));
        assert_eq!(document["exclude"]["patterns"][0].as_str(), Some("old/**"));
    }

    #[test]
    fn test_configuration_precedence_defaults_then_file_then_environment_then_cli() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("mgit.toml");
        fs::write(&path, "jobs = 3\n").unwrap();

        let mut config = MgitConfig::defaults();
        load_config_file(&path, &mut config).unwrap();
        assert_eq!(config.jobs, 3);

        apply_environment_overrides(&mut config, Some("5"));
        assert_eq!(config.jobs, 5);

        let mut cli = MgitConfig::defaults();
        parse_mgit_options(&["--jobs".into(), "9".into()], &mut cli).unwrap();
        apply_cli_overrides(&mut config, &cli);
        assert_eq!(config.jobs, 9);
    }

    #[test]
    fn test_save_config_with_no_existing_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join(".config").join("mgit.toml");
        let argv = to_argv(&["status", "--", "--json", "--save-config"]);
        let (_, config) = parse_cli(argv).unwrap();
        save_config_to(&path, &config).unwrap();

        let document = fs::read_to_string(path)
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(document["json"].as_bool(), Some(true));
    }

    #[test]
    fn test_mgit_parallel_env_fallback() {
        assert_eq!(resolve_jobs(Some("42")), 42);
        assert_eq!(resolve_jobs(None), 2);
        assert_eq!(resolve_jobs(Some("invalid")), 2);
        assert_eq!(resolve_jobs(Some("0")), 2);
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
        assert_eq!(format!("{}", ArgsError::VersionRequested), version_text());
    }
}
