use std::env;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn is_valid_version_base(version: &str) -> bool {
    if version.contains('+') {
        return false;
    }
    let (core, prerelease) = version
        .split_once('-')
        .map_or((version, None), |(core, prerelease)| {
            (core, Some(prerelease))
        });
    if parse_version(core).is_none() {
        return false;
    }
    prerelease.is_none_or(|identifiers| {
        !identifiers.is_empty()
            && identifiers.split('.').all(|identifier| {
                !identifier.is_empty()
                    && identifier
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    && !(identifier.len() > 1
                        && identifier.starts_with('0')
                        && identifier.bytes().all(|byte| byte.is_ascii_digit()))
            })
    })
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parse_component(parts.next()?)?;
    let minor = parse_component(parts.next()?)?;
    let patch = parse_component(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn parse_component(component: &str) -> Option<u64> {
    if component.is_empty()
        || (component.len() > 1 && component.starts_with('0'))
        || !component.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    component.parse().ok()
}

fn stable_tag() -> Option<(String, String)> {
    let tags =
        git(&["tag", "--merged", "HEAD", "--sort=-v:refname"]).expect("could not list Git tags");
    tags.lines().find_map(|tag| {
        let version = tag.strip_prefix('v')?;
        let core = version.split('+').next()?;
        if core.contains('-') || parse_version(core).is_none() {
            return None;
        }
        Some((tag.to_owned(), core.to_owned()))
    })
}

fn conventional_bump(range: &str) -> u8 {
    let messages =
        git(&["log", "--format=%B%x00", range]).expect("could not read Git commit messages");
    let mut bump = 0;
    for message in messages.split('\0') {
        let subject = message.lines().next().unwrap_or_default();
        if subject.contains("!:")
            || message.lines().any(|line| {
                line.starts_with("BREAKING CHANGE:") || line.starts_with("BREAKING-CHANGE:")
            })
        {
            bump = bump.max(3);
        } else if subject.starts_with("feat:") || subject.starts_with("feat(") {
            bump = bump.max(2);
        } else if subject.starts_with("fix:")
            || subject.starts_with("fix(")
            || subject.starts_with("perf:")
            || subject.starts_with("perf(")
        {
            bump = bump.max(1);
        }
    }
    bump
}

fn bumped_version(version: &str, bump: u8) -> String {
    let (mut major, mut minor, mut patch) = parse_version(version).expect("valid package version");
    match bump {
        3 => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        2 => {
            minor += 1;
            patch = 0;
        }
        1 => patch += 1,
        _ => {}
    }
    format!("{major}.{minor}.{patch}")
}

fn main() {
    println!("cargo:rerun-if-env-changed=MGIT_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_MGIT_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_MGIT_BUILD_SHA");
    println!("cargo:rerun-if-changed=build.rs");

    let package_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets package version");
    let git_sha = git(&["rev-parse", "--verify", "HEAD"]);
    let sha = env::var("CARGO_MGIT_BUILD_SHA")
        .ok()
        .or_else(|| git_sha.clone())
        .or_else(|| env::var("GITHUB_SHA").ok());
    let Some(sha) = sha else {
        println!("cargo:rustc-env=MGIT_BUILD_VERSION={package_version}");
        return;
    };
    if git_sha.is_some() {
        if let Some(git_dir) = git(&["rev-parse", "--git-dir"]) {
            println!("cargo:rerun-if-changed={git_dir}/HEAD");
            println!("cargo:rerun-if-changed={git_dir}/index");
        }
    }
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        panic!("git returned an invalid commit SHA");
    }

    let dirty = if git_sha.is_some() {
        !git(&["status", "--porcelain"])
            .expect("could not determine whether the Git working tree is dirty")
            .is_empty()
    } else {
        false
    };

    let supplied_version =
        env::var("MGIT_BUILD_VERSION").or_else(|_| env::var("CARGO_MGIT_BUILD_VERSION"));
    let version = if let Ok(version) = supplied_version {
        if !is_valid_version_base(&version) {
            panic!("MGIT_BUILD_VERSION must be a valid SemVer version without build metadata");
        }
        version
    } else if let Some((tag, stable_version)) = stable_tag() {
        let tag_ref = format!("{tag}^{{commit}}");
        let tagged_sha = git(&["rev-parse", "--verify", &tag_ref])
            .expect("could not resolve the latest stable Git tag");
        if tagged_sha == sha && !dirty {
            stable_version
        } else {
            let range = format!("{tag}..HEAD");
            let count = git(&["rev-list", "--count", &range])
                .expect("could not count commits since the latest stable tag")
                .parse::<u64>()
                .expect("Git returned an invalid commit count");
            let next_version = bumped_version(&stable_version, conventional_bump(&range));
            format!("{next_version}-dev.{count}")
        }
    } else {
        let count = git(&["rev-list", "--count", "HEAD"])
            .expect("could not count commits in the Git repository")
            .parse::<u64>()
            .expect("Git returned an invalid commit count");
        format!("{package_version}-dev.{count}")
    };

    let dirty_suffix = if dirty { ".dirty" } else { "" };
    println!("cargo:rustc-env=MGIT_BUILD_VERSION={version}+{sha}{dirty_suffix}");

    if let Some(head_ref) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(ref_path) = git(&["rev-parse", "--git-path", &head_ref]) {
            println!("cargo:rerun-if-changed={ref_path}");
        }
    }
}
