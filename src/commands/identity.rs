//! Refresh the records-entry identity from its registered root's committed source.
use crate::catalogue::snapshot::{Snapshot, SnapshotSource};
use crate::catalogue::source::{HeadMeta, PublicIdentity, parse_identity, read_head};
use crate::error::{Error, Result};
use crate::output::{EntrySnapshotRow, IdentityRefreshOut, Output, RefreshRow};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub struct RefreshReport {
    pub head: HeadMeta,
    pub result: Result<RefreshKind>,
}

pub enum RefreshKind {
    Refreshed { changed: Vec<&'static str> },
    Unchanged,
    NoIdentity,
}

/// Refresh one entry without committing it (init's configuration-write contract).
/// The command holds one records lock across a repository's entire batch instead.
#[allow(dead_code)] // Init adopts this API in the next plan task.
pub fn refresh(registry: &Registry, project: &Project, now: &str) -> Result<RefreshReport> {
    let _lock = crate::record_commit::try_lock(&project.records).map_err(Error::Io)?;
    refresh_locked(registry, project, now, None)
}

fn refresh_locked(
    registry: &Registry,
    project: &Project,
    now: &str,
    seed: Option<&Path>,
) -> Result<RefreshReport> {
    let root = registry
        .project_root(&project.prefix)
        .ok_or_else(|| Error::Config(format!("no project registered as {:?}", project.prefix)))?;
    let source = read_head(root)?;
    let result = (|| {
        let (identity, snapshot_source) = if let Some(path) = seed {
            // Invalid identity content still proves a committed file exists; it must
            // not let a seed replace the registered root's committed authority.
            match source.found {
                Ok(None) => {}
                Ok(Some(_)) | Err(Error::IdentityInvalid(_)) => {
                    return Err(Error::IdentityCommitted(format!(
                        "{} has identity.toml at {}; omit --from",
                        root.display(),
                        source.head.commit
                    )));
                }
                Err(error) => return Err(error),
            }
            let text = std::fs::read_to_string(path)?;
            (
                parse_identity(&text, &path.display().to_string())?,
                SnapshotSource::Seeded,
            )
        } else {
            let Some((blob, identity)) = source.found? else {
                return Ok(RefreshKind::NoIdentity);
            };
            (
                identity,
                SnapshotSource::Head {
                    commit: source.head.commit.clone(),
                    blob,
                },
            )
        };
        if let Some(prefix) = &identity.prefix
            && prefix != &project.prefix
        {
            return Err(Error::IdentityPrefixMismatch(format!(
                "{} at {} declares prefix {prefix:?}, expected {:?}",
                seed.map(Path::to_path_buf)
                    .unwrap_or_else(|| root.join("identity.toml"))
                    .display(),
                source.head.commit,
                project.prefix
            )));
        }
        write_snapshot(project, identity, snapshot_source, now)
    })();
    Ok(RefreshReport {
        head: source.head,
        result,
    })
}

fn write_snapshot(
    project: &Project,
    identity: PublicIdentity,
    source: SnapshotSource,
    now: &str,
) -> Result<RefreshKind> {
    let path = project.records.join(CONFIG_REL);
    let text = std::fs::read_to_string(&path)?;
    let current = Project::from_config(project.records.clone(), &text)?;
    let snapshot = Snapshot {
        name: identity.name,
        aliases: identity.aliases,
        purpose: identity.purpose,
        source,
        refreshed: now.into(),
    };
    let old = current.identity.as_ref();
    let mut changed = Vec::new();
    if old.is_none_or(|old| old.name != snapshot.name) {
        changed.push("name");
    }
    if old.is_none_or(|old| old.aliases != snapshot.aliases) {
        changed.push("aliases");
    }
    if old.is_none_or(|old| old.purpose != snapshot.purpose) {
        changed.push("purpose");
    }
    match (old.map(|old| &old.source), &snapshot.source) {
        (Some(SnapshotSource::Seeded), SnapshotSource::Seeded) => {}
        (Some(SnapshotSource::Head { blob: old, .. }), SnapshotSource::Head { blob, .. })
            if old == blob => {}
        (Some(SnapshotSource::Head { .. }), SnapshotSource::Head { .. }) => {
            changed.extend(["commit", "blob"])
        }
        _ => changed.push("source"),
    }
    // An unrelated HEAD change (including our own snapshot commit) does not
    // change identity bytes, so keep the original provenance and refresh time.
    if changed.is_empty() {
        return Ok(RefreshKind::Unchanged);
    }
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
    let mut table = snapshot.to_table();
    if let Some(previous) = doc.get("identity").and_then(toml_edit::Item::as_table) {
        *table.decor_mut() = previous.decor().clone();
        table.set_position(previous.position());
    }
    doc["identity"] = toml_edit::Item::Table(table);
    crate::repo::atomic_write(&path, doc.to_string().as_bytes())?;
    Ok(RefreshKind::Refreshed { changed })
}

fn error_row(row: &mut RefreshRow, error: Error) {
    row.outcome = "error";
    row.detail = Some(format!("{}: {error}", error.kind()));
}

fn report_row(row: &mut RefreshRow, project: &Project, report: RefreshReport, seeded: bool) {
    row.branch = Some(report.head.branch);
    row.commit = Some(report.head.commit);
    let result = report.result.and_then(|kind| {
        let project = Project::open(&project.records)?;
        row.snapshot = project.identity.as_ref().map(|snapshot| {
            let (source, commit, blob) = match &snapshot.source {
                SnapshotSource::Head { commit, blob } => {
                    ("head", Some(commit.clone()), Some(blob.clone()))
                }
                SnapshotSource::Seeded => ("seeded", None, None),
            };
            EntrySnapshotRow {
                source,
                commit,
                blob,
                refreshed: snapshot.refreshed.clone(),
            }
        });
        Ok(kind)
    });
    match result {
        Ok(RefreshKind::Refreshed { changed }) => {
            row.changed = changed;
            row.outcome = if seeded { "seeded" } else { "refreshed" };
            if seeded {
                row.branch = None;
                row.commit = None;
            }
        }
        Ok(RefreshKind::Unchanged) => row.outcome = "unchanged",
        Ok(RefreshKind::NoIdentity) => row.outcome = "no_identity",
        Err(error) => error_row(row, error),
    }
}

pub fn run(
    prefixes: Vec<String>,
    all: bool,
    seed: Option<&Path>,
    dir: Option<&Path>,
) -> Result<Output> {
    let registry = Registry::load()?;
    let seed = seed
        .map(|path| super::start_dir(dir).map(|start| start.join(path)))
        .transpose()?;
    let prefixes = if all {
        registry.projects.keys().cloned().collect()
    } else if prefixes.is_empty() {
        vec![Project::locate_with(&registry, &super::start_dir(dir)?)?.prefix]
    } else {
        prefixes
    };
    let mut seen = BTreeSet::new();
    let prefixes: Vec<_> = prefixes
        .iter()
        .map(|p| registry.canonical_prefix(p).to_string())
        .filter(|p| seen.insert(p.clone()))
        .collect();
    let mut results: Vec<_> = prefixes
        .iter()
        .map(|prefix| RefreshRow {
            prefix: prefix.clone(),
            ..RefreshRow::default()
        })
        .collect();
    let mut groups: BTreeMap<PathBuf, Vec<(usize, Project)>> = BTreeMap::new();
    for (index, row) in results.iter_mut().enumerate() {
        let prepared = (|| {
            let root = registry.project_root(&row.prefix).ok_or_else(|| {
                Error::Config(format!("no project registered as {:?}", row.prefix))
            })?;
            if !root.try_exists()? {
                row.outcome = "unreachable";
                return Ok(None);
            }
            let project = crate::scope::open_registered(
                &registry,
                &row.prefix,
                crate::scope::Origin::Prefix,
            )?;
            let top = crate::record_commit::working_tree(&project.records)
                .map_err(Error::Io)?
                .unwrap_or_else(|| project.records.clone());
            Ok(Some((top, project)))
        })();
        match prepared {
            Ok(Some((top, project))) => groups.entry(top).or_default().push((index, project)),
            Ok(None) => {}
            Err(error) => error_row(row, error),
        }
    }
    let now = crate::time::now();
    let mut warnings = Vec::new();
    for (top, entries) in groups {
        let lock = match crate::record_commit::try_lock(&top) {
            Ok(lock) => lock,
            Err(error) => {
                for (index, _) in entries {
                    error_row(&mut results[index], Error::Io(error.clone()));
                }
                continue;
            }
        };
        let mut paths = Vec::new();
        let mut committed = Vec::new();
        for (index, project) in entries {
            // Reopen under the repository lock; discovery was only for grouping.
            let project = match crate::scope::open_registered(
                &registry,
                &project.prefix,
                crate::scope::Origin::Prefix,
            ) {
                Ok(project) => project,
                Err(error) => {
                    error_row(&mut results[index], error);
                    continue;
                }
            };
            let report = refresh_locked(&registry, &project, &now, seed.as_deref());
            let row = &mut results[index];
            match report {
                Ok(report) => report_row(row, &project, report, seed.is_some()),
                Err(error) => error_row(row, error),
            }
            if project.commit_records && matches!(row.outcome, "refreshed" | "seeded") {
                paths.push(
                    project
                        .records
                        .join(CONFIG_REL)
                        .strip_prefix(&top)
                        .map_err(|error| Error::Io(error.to_string()))?
                        .to_path_buf(),
                );
                committed.push(project.prefix);
            }
        }
        if !paths.is_empty() {
            if let Some(lock) = &lock {
                let message = format!(
                    "chore(tasks): refresh identity snapshot for {}",
                    committed.join(", ")
                );
                let (refused, notices) = lock.commit_paths(&top, &paths, &message);
                warnings.extend(notices);
                warnings.extend(refused);
            } else {
                warnings.push(format!(
                    "commit_unavailable: {} is outside Git; identity snapshots left uncommitted",
                    top.display()
                ));
            }
        }
    }
    Ok(Output::IdentityRefresh(IdentityRefreshOut {
        results,
        warnings,
    }))
}
