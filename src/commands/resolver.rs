//! `tasks resolve` (docs/project-operations.md).
use crate::catalogue::discovery::{Catalogue, EntryState};
use crate::catalogue::snapshot::SnapshotSource;
use crate::error::Result;
use crate::locations::{Answer, Index, normalize};
use crate::model::{TaskId, is_valid_prefix};
use crate::output::{EntryRow, EntrySnapshotRow, Output, ResolveOut, ResolveRow};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub fn run(
    inputs: Vec<String>,
    stdin: bool,
    all: bool,
    freshness: bool,
    dir: Option<&Path>,
) -> Result<Output> {
    let registry = Registry::load()?;
    let catalogue = Catalogue::build(&registry);
    let inputs = if all {
        catalogue.entries.keys().cloned().map(Ok).collect()
    } else if stdin {
        read_inputs(std::io::stdin().lock())
    } else {
        inputs.into_iter().map(Ok).collect()
    };
    let index = Index::build(&registry);
    let mut warnings = index.warnings.clone();
    warnings.extend(catalogue.warnings.iter().cloned());
    for (prefix, entry) in &catalogue.entries {
        if let EntryState::Error(detail) = entry {
            warnings.push(format!("{prefix}: {detail}"));
        }
    }
    let results = inputs
        .into_iter()
        .map(|input| match input {
            Ok(input) => {
                let mut result = if all {
                    prefix_row(&registry, &index, input.clone(), input)
                } else {
                    row(&registry, &index, &catalogue, dir, input, &mut warnings)
                };
                if let Some(prefix) = &result.prefix {
                    match catalogue.entries.get(prefix) {
                        Some(EntryState::Read(project)) => {
                            result.entry = Some(entry_row(project, freshness, &mut warnings));
                        }
                        Some(EntryState::Error(detail)) => {
                            result.entry_error = Some(detail.clone())
                        }
                        None => {}
                    }
                }
                result
            }
            Err((input, detail)) => invalid_row(input, "invalid", detail),
        })
        .collect();
    Ok(Output::Resolve(ResolveOut { results, warnings }))
}

fn entry_row(project: &Project, with_freshness: bool, warnings: &mut Vec<String>) -> EntryRow {
    let snapshot = project.identity.as_ref();
    let freshness = with_freshness.then(|| {
        if !project.root.is_dir() {
            return crate::catalogue::source::Freshness::Unknown;
        }
        let (freshness, warning) = crate::catalogue::source::freshness(&project.root, snapshot);
        warnings.extend(warning);
        freshness
    });
    EntryRow {
        config: project.records.join(CONFIG_REL).display().to_string(),
        records: project.records.display().to_string(),
        checkout: project.checkout.display().to_string(),
        kind: project.kind,
        name: snapshot.map(|s| s.name.clone()),
        aliases: snapshot.map(|s| s.aliases.clone()),
        purpose: snapshot.map(|s| s.purpose.clone()),
        snapshot: snapshot.map(|s| {
            let (source, commit, blob) = match &s.source {
                SnapshotSource::Head { commit, blob } => {
                    ("head", Some(commit.clone()), Some(blob.clone()))
                }
                SnapshotSource::Seeded => ("seeded", None, None),
            };
            EntrySnapshotRow {
                source,
                commit,
                blob,
                refreshed: s.refreshed.clone(),
            }
        }),
        shorthand: project.shorthand.clone(),
        profile: project.profile.clone(),
        color: project.color,
        retired: project.retired.clone(),
        feedback: project.feedback.clone(),
        freshness,
    }
}

/// : path, then id, then prefix; never the bare-hex shorthand.
fn row(
    registry: &Registry,
    index: &Index,
    catalogue: &Catalogue,
    dir: Option<&Path>,
    input: String,
    warnings: &mut Vec<String>,
) -> ResolveRow {
    if input.contains('/') || input.starts_with('.') || input.starts_with('~') {
        return path_row(registry, index, catalogue, dir, input, warnings);
    }
    if let Ok(id) = TaskId::parse(&input) {
        let live = registry.canonical_id(&id).prefix;
        return match registry.project_root(&live) {
            Some(root) => ResolveRow {
                id: Some(registry.canonical_id(&id).to_string()),
                alias: (live != id.prefix).then(|| id.prefix.clone()),
                color: index.color(&live),
                root: Some(root.display().to_string()),
                records: Some(records_of(registry, &live, root)),
                prefix: Some(live),
                ..row_of(input, "id", "resolved")
            },
            None => ResolveRow {
                prefix: Some(id.prefix.clone()),
                ..row_of(input, "id", "unregistered")
            },
        };
    }
    if is_valid_prefix(&input) {
        let prefix = registry.canonical_prefix(&input).to_string();
        return prefix_row(registry, index, input, prefix);
    }
    ResolveRow {
        detail: Some("expected an id <prefix>-<hex6>, a prefix, or a path".into()),
        ..row_of(input, "invalid", "invalid")
    }
}

fn prefix_row(registry: &Registry, index: &Index, input: String, prefix: String) -> ResolveRow {
    match registry.project_root(&prefix) {
        Some(root) => ResolveRow {
            alias: (prefix != input).then(|| input.clone()),
            color: index.color(&prefix),
            root: Some(root.display().to_string()),
            records: Some(records_of(registry, &prefix, root)),
            prefix: Some(prefix),
            ..row_of(input, "prefix", "resolved")
        },
        None => ResolveRow {
            prefix: Some(input.clone()),
            ..row_of(input, "prefix", "unregistered")
        },
    }
}

fn path_row(
    registry: &Registry,
    index: &Index,
    catalogue: &Catalogue,
    dir: Option<&Path>,
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
    let base = if expanded.is_absolute() {
        Ok(PathBuf::from("/"))
    } else if let Some(base) = dir.filter(|base| base.is_absolute()) {
        Ok(base.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| dir.map_or(cwd.clone(), |base| cwd.join(base)))
            .map_err(|error| format!("cannot read working directory: {error}"))
    };
    let base = match base {
        Ok(base) => base,
        Err(detail) => return invalid_row(input, "path", detail),
    };
    let path = match normalize(&expanded, &base) {
        Ok(path) => path,
        Err(detail) => {
            return ResolveRow {
                detail: Some(detail),
                ..row_of(input, "path", "invalid")
            };
        }
    };
    let mut result = match index.resolve(&path, warnings) {
        Answer::Resolved(m) => ResolveRow {
            root: registry
                .project_root(&m.prefix)
                .map(|r| r.display().to_string()),
            records: registry
                .project_root(&m.prefix)
                .map(|root| records_of(registry, &m.prefix, root)),
            color: index.color(&m.prefix),
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
    };
    result.records_store = catalogue
        .store_of(&path)
        .map(|store| store.display().to_string());
    result
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

/// The records directory the registry pairs with `prefix`, else its root.
fn records_of(registry: &Registry, prefix: &str, root: &Path) -> String {
    registry
        .records_dir(prefix)
        .unwrap_or(root)
        .display()
        .to_string()
}

fn row_of(input: String, kind: &'static str, status: &'static str) -> ResolveRow {
    ResolveRow {
        input,
        kind,
        status,
        ..ResolveRow::default()
    }
}

/// Decode each line independently; a read failure ends the stream, not the command.
fn read_inputs(mut reader: impl BufRead) -> Vec<std::result::Result<String, (String, String)>> {
    let mut inputs = Vec::new();
    loop {
        let mut bytes = Vec::new();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) => break,
            Ok(_) => {
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                    if bytes.last() == Some(&b'\r') {
                        bytes.pop();
                    }
                }
                inputs.push(String::from_utf8(bytes).map_err(|error| {
                    (
                        String::from_utf8_lossy(error.as_bytes()).into_owned(),
                        format!("stdin line is not valid UTF-8: {}", error.utf8_error()),
                    )
                }));
            }
            Err(error) => {
                inputs.push(Err((
                    String::from_utf8_lossy(&bytes).into_owned(),
                    format!("cannot read stdin: {error}"),
                )));
                break;
            }
        }
    }
    inputs
}

fn invalid_row(input: String, kind: &'static str, detail: String) -> ResolveRow {
    ResolveRow {
        detail: Some(detail),
        ..row_of(input, kind, "invalid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdin_read_error_preserves_prior_lines_and_stops() {
        struct Failing {
            first: bool,
        }
        impl std::io::Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                unreachable!()
            }
        }
        impl BufRead for Failing {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                if self.first {
                    Ok(b"sci\n")
                } else {
                    Err(std::io::Error::other("reader failed"))
                }
            }
            fn consume(&mut self, _: usize) {
                self.first = false;
            }
        }
        let inputs = read_inputs(Failing { first: true });
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0], Ok("sci".into()));
        assert!(
            inputs[1]
                .as_ref()
                .unwrap_err()
                .1
                .contains("cannot read stdin: reader failed")
        );
    }
}
