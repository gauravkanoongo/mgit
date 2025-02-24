extern crate num_cpus;
extern crate threadpool;
extern crate walkdir;

use std::env;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{stderr, BufRead, BufReader, Error, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use threadpool::ThreadPool;
use walkdir::WalkDir;

enum GitStatus {
    SUCCESS,
    FAILURE,
    ERROR,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 || args.contains(&"--help".to_string()) {
        print_help();
        return;
    }

    // Allow customizing pool size
    let pool_size: usize = match env::var("MGIT_PARALLEL") {
        Ok(val) => val.parse().unwrap_or(4),
        Err(_) => num_cpus::get() * 8,
    };

    let (tx, rx) = channel();
    fork(pool_size, &tx).unwrap();
    drop(tx);
    join(rx).unwrap()
}

fn print_help() {
    println!("Usage: mgit [OPTIONS] --exec <cmd> [ARGS]...");
    println!();
    println!("Run a git command over multiple git repositories in a folder tree.");
    println!();
    println!("Options:");
    println!("  --help          Show this help message and exit.");
    println!("  --exec <cmd>    Execute a command for each repository.");
    println!();
    println!("Placeholders:");
    println!("  {{}}    path (of the current search result)");
    println!("  {{/}}   basename");
    println!("  {{//}}  parent directory");
    println!("  {{.}}   path without file extension");
    println!("  {{/.}}  basename without file extension");
    println!("  {{{{}}  literal '{{' (for escaping)");
    println!("  }}}}    literal '}}' (for escaping)");
    println!();
    println!("Examples:");
    println!("  mgit --exec git status");
    println!("  mgit --exec git add {{}}");
    println!("  mgit --exec echo {{/}}");
}

fn fork(pool_size: usize, tx: &Sender<(PathBuf, GitStatus, String)>) -> Result<(), Error> {
    let mut count = 0;
    let pool = ThreadPool::new(pool_size);

    let args: Vec<String> = env::args().skip(1).collect();
    let (exec_cmd, exec_args) = parse_exec_args(&args);

    for entry in WalkDir::new(env::current_dir()?)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir() && e.file_name().eq(".git"))
    {
        count += 1;
        let path = entry.path().parent().unwrap().to_owned();
        let tx = tx.clone();
        let exec_cmd = exec_cmd.clone();
        let exec_args = exec_args.clone();
        pool.execute(move || {
            let args = replace_placeholders(&exec_args, &path);
            let result = Command::new(exec_cmd)
                .current_dir(path.clone())
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            let mut output = String::new();
            let status: GitStatus;
            match result {
                Ok(child) => {
                    let child_output = child.wait_with_output().unwrap();

                    status = match child_output.status.code() {
                        Some(0) => GitStatus::SUCCESS,
                        Some(_) => GitStatus::FAILURE,
                        None => GitStatus::ERROR,
                    };
                    if !&child_output.stdout.is_empty() {
                        writeln!(
                            &mut output,
                            "{}",
                            String::from_utf8_lossy(&child_output.stdout)
                        )
                        .unwrap();
                    }
                    if !&child_output.stderr.is_empty() {
                        writeln!(
                            &mut output,
                            "{}",
                            String::from_utf8_lossy(&child_output.stderr)
                        )
                        .unwrap();
                    }
                }
                Err(err) => {
                    status = GitStatus::ERROR;
                    writeln!(&mut output, "{}", err).unwrap();
                }
            }

            tx.send((path, status, output))
                .expect("Could not send data!");
        });
    }

    println!(
        "Running '{}' on {} projects.",
        exec_cmd,
        count
    );
    Ok(())
}

fn parse_exec_args(args: &[String]) -> (String, Vec<String>) {
    let mut exec_cmd = "git".to_string();
    let mut exec_args = Vec::new();
    let mut exec_found = false;

    for arg in args {
        if exec_found {
            exec_args.push(arg.clone());
        } else if arg == "--exec" || arg == "-x" {
            exec_found = true;
        } else {
            exec_cmd = arg.clone();
        }
    }

    if !exec_found {
        exec_args.push("{}".to_string());
    }

    (exec_cmd, exec_args)
}

fn replace_placeholders(args: &[String], path: &PathBuf) -> Vec<String> {
    let mut replaced_args = Vec::new();
    let path_str = path.to_str().unwrap();
    let basename = path.file_name().unwrap().to_str().unwrap();
    let parent_dir = path.parent().unwrap().to_str().unwrap();
    let path_without_ext = path.with_extension("").to_str().unwrap();
    let basename_without_ext = path.file_stem().unwrap().to_str().unwrap();

    for arg in args {
        let replaced_arg = arg
            .replace("{}", path_str)
            .replace("{/}", basename)
            .replace("{//}", parent_dir)
            .replace("{.}", path_without_ext)
            .replace("{/.}", basename_without_ext)
            .replace("{{", "{")
            .replace("}}", "}");
        replaced_args.push(replaced_arg);
    }

    replaced_args
}

fn join(rx: Receiver<(PathBuf, GitStatus, String)>) -> Result<(), Error> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let mut success = 0;
    let mut failed = 0;
    let mut errors = 0;
    for (path, status, output) in rx.iter() {
        let title_color = match status {
            GitStatus::SUCCESS => {
                success += 1;
                Some(Color::Green)
            }
            GitStatus::FAILURE => {
                failed += 1;
                Some(Color::Yellow)
            }
            GitStatus::ERROR => {
                errors += 1;
                Some(Color::Red)
            }
        };

        stdout.set_color(ColorSpec::new().set_fg(title_color).set_bold(true))?;
        writeln!(&mut stdout, "{}", path.display())?;
        stdout.reset()?;

        match status {
            GitStatus::SUCCESS => {
                write!(&mut stdout, "{}", output)?;
            }
            GitStatus::FAILURE | GitStatus::ERROR => {
                write!(&mut stderr(), "{}", output)?;
            }
        };
    }

    // Status line:
    writeln!(&mut stdout)?;
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)).set_bold(true))?;
    write!(&mut stdout, "Success: ")?;
    stdout.reset()?;
    write!(&mut stdout, "{}", success)?;

    if failed > 0 {
        write!(&mut stdout, ", ")?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)).set_bold(true))?;
        write!(&mut stdout, "Warnings: ")?;
        stdout.reset()?;
        write!(&mut stdout, "{}", failed)?;
    }

    if errors > 0 {
        write!(&mut stdout, ", ")?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true))?;
        write!(&mut stdout, "Errors: ")?;
        stdout.reset()?;
        write!(&mut stdout, "{}", errors)?;
    }
    writeln!(&mut stdout)?;
    Ok(())
}
