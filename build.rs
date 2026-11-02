//! Records the commit the binary is built from, for `--version`: `tasks 0.4.0 (1a2b3c4)`.
//! A build outside a git checkout of this source (a source archive, a manifest nested in
//! someone else's repository) prints the bare version, which consumers read as unknown.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let source = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let version = match build_commit(&source) {
        Some(sha) => format!("{version} ({sha})"),
        None => version,
    };
    println!("cargo:rustc-env=TASKS_VERSION={version}");
}

/// The short commit of `source`'s checkout, or `None` when `source` is not the top of one.
/// Once it is, every git failure is a build error rather than a silently bare version.
fn build_commit(source: &Path) -> Option<String> {
    let top = git(source, &["rev-parse", "--show-toplevel"]).ok()?;
    if Path::new(&top).canonicalize().ok()? != source.canonicalize().unwrap() {
        return None;
    }
    let required = |args: &[&str]| {
        git(source, args).unwrap_or_else(|error| panic!("git {args:?} in {source:?}: {error}"))
    };
    // Rebuild when HEAD moves to another branch or commit, or the branch it names moves.
    // A watched path that does not exist (a packed ref) makes cargo rerun this script
    // on every build, which costs two git calls and keeps the commit current.
    let mut watched = vec!["HEAD".to_string(), "packed-refs".to_string()];
    if let Ok(branch) = git(source, &["symbolic-ref", "-q", "HEAD"]) {
        watched.push(branch);
    }
    for name in watched {
        let path = required(&["rev-parse", "--path-format=absolute", "--git-path", &name]);
        println!("cargo:rerun-if-changed={path}");
    }
    Some(required(&["rev-parse", "--short", "HEAD"]))
}

/// Runs git in `dir` without inherited `GIT_*` selectors, which a hook running the build
/// can export and which would otherwise describe a different repository.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let out = command.output().map_err(|error| error.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim_end().to_string());
    }
    String::from_utf8(out.stdout)
        .map(|text| text.trim_end().to_string())
        .map_err(|error| error.to_string())
}
