//! `tasks resolve` (docs/specs/2026-10-06-project-resolution-design.md §3, §4).
use crate::error::Result;
use crate::locations::{Answer, Index, normalize};
use crate::model::{TaskId, is_valid_prefix};
use crate::output::{Output, ResolveOut, ResolveRow};
use crate::registry::Registry;
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub fn run(inputs: Vec<String>, stdin: bool, dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let base = super::start_dir(dir)?;
    let inputs = if stdin {
        std::io::stdin()
            .lock()
            .lines()
            .map(|line| line.map(|l| l.trim_end_matches('\r').to_string()))
            .collect::<std::io::Result<Vec<_>>>()?
    } else {
        inputs
    };
    let index = Index::build(&registry);
    let mut warnings = index.warnings.clone();
    let results = inputs
        .into_iter()
        .map(|input| row(&registry, &index, &base, input, &mut warnings))
        .collect();
    Ok(Output::Resolve(ResolveOut { results, warnings }))
}

/// §3.1: path, then id, then prefix; never the bare-hex shorthand.
fn row(
    registry: &Registry,
    index: &Index,
    base: &Path,
    input: String,
    warnings: &mut Vec<String>,
) -> ResolveRow {
    if input.contains('/') || input.starts_with('.') || input.starts_with('~') {
        return path_row(registry, index, base, input, warnings);
    }
    if let Ok(id) = TaskId::parse(&input) {
        let live = registry.canonical_prefix(&id.prefix).to_string();
        return match registry.project_root(&live) {
            Some(root) => ResolveRow {
                id: Some(registry.canonical_id(&id).to_string()),
                alias: (live != id.prefix).then(|| id.prefix.clone()),
                prefix: Some(live),
                root: Some(root.display().to_string()),
                ..row_of(input, "id", "resolved")
            },
            None => ResolveRow {
                prefix: Some(id.prefix.clone()),
                ..row_of(input, "id", "unregistered")
            },
        };
    }
    if is_valid_prefix(&input) {
        let live = registry.canonical_prefix(&input).to_string();
        return match registry.project_root(&live) {
            Some(root) => ResolveRow {
                alias: (live != input).then(|| input.clone()),
                prefix: Some(live),
                root: Some(root.display().to_string()),
                ..row_of(input, "prefix", "resolved")
            },
            None => ResolveRow {
                prefix: Some(input.clone()),
                ..row_of(input, "prefix", "unregistered")
            },
        };
    }
    ResolveRow {
        detail: Some("expected an id <prefix>-<hex6>, a prefix, or a path".into()),
        ..row_of(input, "invalid", "invalid")
    }
}

fn path_row(
    registry: &Registry,
    index: &Index,
    base: &Path,
    input: String,
    warnings: &mut Vec<String>,
) -> ResolveRow {
    let expanded = match expand_home(&input) {
        Ok(path) => path,
        Err(detail) => {
            return ResolveRow {
                detail: Some(detail),
                ..row_of(input, "path", "invalid")
            };
        }
    };
    let path = match normalize(&expanded, base) {
        Ok(path) => path,
        Err(detail) => {
            return ResolveRow {
                detail: Some(detail),
                ..row_of(input, "path", "invalid")
            };
        }
    };
    match index.resolve(&path, warnings) {
        Answer::Resolved(m) => ResolveRow {
            root: registry
                .project_root(&m.prefix)
                .map(|r| r.display().to_string()),
            prefix: Some(m.prefix),
            via: Some(m.via),
            location: Some(m.location.display().to_string()),
            worktree: m.worktree.map(|w| w.display().to_string()),
            relpath: Some(m.relpath.display().to_string()),
            ..row_of(input, "path", "resolved")
        },
        Answer::Ambiguous(candidates) => ResolveRow {
            candidates,
            ..row_of(input, "path", "ambiguous")
        },
        Answer::Unregistered => row_of(input, "path", "unregistered"),
    }
}

fn expand_home(input: &str) -> std::result::Result<PathBuf, String> {
    let Some(rest) = input.strip_prefix('~') else {
        return Ok(PathBuf::from(input));
    };
    if !(rest.is_empty() || rest.starts_with('/')) {
        return Err(format!("{input}: only ~ and ~/ are expanded"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| format!("{input}: HOME is not set"))?;
    Ok(PathBuf::from(home).join(rest.trim_start_matches('/')))
}

fn row_of(input: String, kind: &'static str, status: &'static str) -> ResolveRow {
    ResolveRow {
        input,
        kind,
        status,
        ..ResolveRow::default()
    }
}
