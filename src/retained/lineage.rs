use super::{basis, model::*};
use crate::{
    error::Result,
    model::{Task, TaskId, is_valid_prefix},
    repo::Project,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameEdge {
    pub version: u8,
    pub from_prefix: String,
    pub to_prefix: String,
    pub layout: String,
    pub from_subdir: String,
    pub to_subdir: String,
    pub commit: String,
    pub retained_ref: String,
}
pub fn task_path(subdir: &str, id: &TaskId) -> String {
    format!("{subdir}/tasks/{id}.md")
        .trim_start_matches("./")
        .into()
}
pub fn subdir(project: &Project) -> Result<String> {
    let root = basis::root(project)?;
    let sub = project.records.strip_prefix(&root).map_err(invalid)?;
    Ok(if sub.as_os_str().is_empty() {
        ".".into()
    } else {
        sub.to_str()
            .ok_or_else(|| invalid("non UTF-8 records path"))?
            .into()
    })
}
fn ancestor(project: &Project, commit: &str, reference: &str) -> Result<()> {
    basis::git(project, &["show-ref", "--verify", reference])?;
    basis::git(project, &["merge-base", "--is-ancestor", commit, reference])?;
    Ok(())
}
pub fn validate(project: &Project) -> Result<()> {
    let edges = &project.retention_lineage;
    if edges.is_empty() {
        return Ok(());
    }
    let mut seen = BTreeSet::new();
    for (i, e) in edges.iter().enumerate() {
        let id = TaskId::parse(&format!("{}-000001", e.from_prefix))?;
        let b = Basis {
            project: e.from_prefix.clone(),
            layout: e.layout.clone(),
            records_subdir: e.from_subdir.clone(),
            commit: e.commit.clone(),
            task_path: task_path(&e.from_subdir, &id),
            retained_ref: e.retained_ref.clone(),
        };
        b.validate()?;
        if e.version != 1
            || !is_valid_prefix(&e.to_prefix)
            || e.from_prefix == e.to_prefix
            || !(e.to_subdir == "." || member_path(&e.to_subdir))
            || !seen.insert(&e.from_prefix)
            || seen.contains(&e.to_prefix)
            || e.layout != project.kind.as_str()
        {
            return Err(invalid("conflicting or unsupported retention lineage"));
        }
        if let Some(previous) = i.checked_sub(1).map(|i| &edges[i]) {
            if previous.to_prefix != e.from_prefix || previous.to_subdir != e.from_subdir {
                return Err(invalid("noncontiguous retention lineage"));
            }
            basis::git(
                project,
                &["merge-base", "--is-ancestor", &previous.commit, &e.commit],
            )?;
        }
        ancestor(project, &e.commit, &e.retained_ref)?;
        let mut bytes = 0;
        let path = format!("{}/tasks/.config.toml", e.from_subdir)
            .trim_start_matches("./")
            .to_string();
        let raw = basis::blob(project, &e.commit, &path, None, &mut bytes)?;
        let config: toml::Value =
            toml::from_str(std::str::from_utf8(&raw).map_err(invalid)?).map_err(invalid)?;
        let prior: Vec<RenameEdge> = config
            .get("retention_lineage")
            .map(|v| v.clone().try_into())
            .transpose()
            .map_err(invalid)?
            .unwrap_or_default();
        let layout = config
            .get("checkout")
            .map(|v| v.get("kind").and_then(|v| v.as_str()).unwrap_or("linked"))
            .unwrap_or("in-tree");
        if config.get("prefix").and_then(|v| v.as_str()) != Some(e.from_prefix.as_str())
            || prior != edges[..i]
            || layout != e.layout
        {
            return Err(invalid(
                "retention lineage differs from its anchored source configuration",
            ));
        }
    }
    let last = edges.last().expect("nonempty");
    if last.to_prefix != project.prefix || last.to_subdir != subdir(project)? {
        return Err(invalid(
            "retention lineage endpoint differs from live pairing",
        ));
    }
    Ok(())
}
pub fn epoch_subdir<'a>(project: &'a Project, prefix: &str) -> Result<String> {
    if prefix == project.prefix {
        return subdir(project);
    }
    project
        .retention_lineage
        .iter()
        .find(|e| e.from_prefix == prefix)
        .map(|e| e.from_subdir.clone())
        .ok_or_else(|| invalid("historical project lacks verified retention lineage"))
}
pub fn historical_id(basis: &Basis) -> Result<TaskId> {
    let id = TaskId::parse(
        Path::new(&basis.task_path)
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or_else(|| invalid("historical task path"))?,
    )?;
    if id.prefix != basis.project || task_path(&basis.records_subdir, &id) != basis.task_path {
        return Err(invalid("historical task path disagrees with its identity"));
    }
    Ok(id)
}
// No history reader recursion: compare exact task note prefixes at the declared cutovers.
pub fn equivalent(project: &Project, historical: &TaskId, current: &TaskId) -> Result<bool> {
    if historical == current {
        return Ok(true);
    }
    if historical.hex != current.hex {
        return Ok(false);
    }
    validate(project)?;
    let start = project
        .retention_lineage
        .iter()
        .position(|e| e.from_prefix == historical.prefix);
    let Some(start) = start else {
        return Ok(false);
    };
    let mut prefix = historical.prefix.clone();
    let mut previous: Option<Task> = None;
    for e in &project.retention_lineage[start..] {
        if prefix == current.prefix {
            return Ok(true);
        }
        let id = TaskId::parse(&format!("{}-{}", e.from_prefix, current.hex))?;
        let b = Basis {
            project: e.from_prefix.clone(),
            layout: e.layout.clone(),
            records_subdir: e.from_subdir.clone(),
            commit: e.commit.clone(),
            task_path: task_path(&e.from_subdir, &id),
            retained_ref: e.retained_ref.clone(),
        };
        let task = basis::task_at(project, &id, &b, &mut 0)?;
        if previous
            .as_ref()
            .is_some_and(|p| !task.notes.starts_with(&p.notes))
        {
            return Err(invalid("rename changed exact retained logical history"));
        }
        previous = Some(task);
        prefix = e.to_prefix.clone();
    }
    if prefix != current.prefix {
        return Ok(false);
    }
    let live = project.read_task(current)?;
    if previous.is_some_and(|p| !live.notes.starts_with(&p.notes)) {
        return Err(invalid("live history does not preserve rename source"));
    }
    Ok(true)
}
pub fn same(project: &Project, a: &str, b: &str) -> Result<bool> {
    let a = TaskId::parse(a)?;
    let b = TaskId::parse(b)?;
    Ok(equivalent(project, &a, &b)? || equivalent(project, &b, &a)?)
}
pub fn owns_prefix(project: &Project, prefix: &str) -> bool {
    prefix == project.prefix
        || project
            .retention_lineage
            .iter()
            .any(|e| e.from_prefix == prefix)
}
pub fn epoch_at(project: &Project, id: &TaskId, commit: &str) -> Result<Basis> {
    validate(project)?;
    let mut epochs = project
        .retention_lineage
        .iter()
        .map(|e| (e.from_prefix.clone(), e.from_subdir.clone()))
        .collect::<Vec<_>>();
    epochs.push((project.prefix.clone(), subdir(project)?));
    let mut candidates = Vec::new();
    for (prefix, sub) in epochs {
        let path = format!("{sub}/tasks/.config.toml")
            .trim_start_matches("./")
            .to_string();
        let tree = basis::git(project, &["ls-tree", "-z", commit, "--", &path])?;
        if tree.is_empty() {
            continue;
        }
        let raw = basis::blob(project, commit, &path, None, &mut 0)?;
        let config: toml::Value =
            toml::from_str(std::str::from_utf8(&raw).map_err(invalid)?).map_err(invalid)?;
        if config.get("prefix").and_then(|v| v.as_str()) != Some(prefix.as_str()) {
            continue;
        }
        let original = TaskId::parse(&format!("{prefix}-{}", id.hex))?;
        let refs = String::from_utf8(basis::git(
            project,
            &[
                "for-each-ref",
                "--contains",
                commit,
                "--format=%(refname)",
                "refs/heads/",
            ],
        )?)
        .map_err(invalid)?;
        let retained_ref = refs
            .lines()
            .next()
            .ok_or_else(|| invalid("historical revision lacks a retained branch"))?
            .into();
        candidates.push(Basis {
            project: prefix,
            layout: project.kind.as_str().into(),
            records_subdir: sub.clone(),
            commit: commit.into(),
            task_path: task_path(&sub, &original),
            retained_ref,
        });
    }
    if candidates.len() != 1 {
        return Err(invalid(
            "historical revision has no unique declared rename epoch",
        ));
    }
    Ok(candidates.remove(0))
}
pub fn validate_locator(project: &Project, id: &TaskId, b: &Basis) -> Result<()> {
    validate(project)?;
    let old = historical_id(b)?;
    if b.layout != project.kind.as_str()
        || epoch_subdir(project, &old.prefix)? != b.records_subdir
        || old.hex != id.hex
        || !owns_prefix(project, &id.prefix)
        || !equivalent(
            project,
            &old,
            &TaskId::parse(&format!("{}-{}", project.prefix, id.hex))?,
        )?
    {
        return Err(invalid("basis is outside verified retention lineage"));
    }
    ancestor(project, &b.commit, &b.retained_ref)?;
    if let Some(e) = project
        .retention_lineage
        .iter()
        .find(|e| e.from_prefix == old.prefix)
    {
        basis::git(
            project,
            &["merge-base", "--is-ancestor", &b.commit, &e.commit],
        )?;
    }
    Ok(())
}

pub fn participant<'a>(
    project: &Project,
    note: &'a OperationNote,
    id: &TaskId,
) -> Result<&'a Input> {
    let mut found = None;
    for input in &note.input_bases {
        if same(project, &input.task, &id.to_string())? {
            if found.is_some() {
                return Err(invalid("ambiguous historical participant"));
            }
            found = Some(input);
        }
    }
    found.ok_or_else(|| invalid("operation lacks its source participant"))
}

pub fn owner_matches(project: &Project, owner: &str, id: &TaskId) -> Result<bool> {
    match owner.strip_prefix("task:") {
        Some(owner) => same(project, owner, &id.to_string()),
        None => Ok(false),
    }
}
pub fn manifest_path(
    project: &Project,
    pinned: &basis::PinnedTask,
    record: &RetentionRecord,
) -> Result<std::path::PathBuf> {
    let raw = record
        .manifest
        .value()
        .ok_or_else(|| invalid("record has no manifest"))?;
    let original = super::verify::current_path(project, raw)?;
    if project.retention_lineage.is_empty() {
        return Ok(original);
    }
    let Some(name) = original.file_name().and_then(|v| v.to_str()) else {
        return Err(invalid("manifest filename"));
    };
    let Some(digest) = record.sha256.value() else {
        return Ok(original);
    };
    if name != format!("retained-manifest-{digest}.json") {
        return Ok(original);
    }
    let root = basis::root(project)?;
    for e in &project.retention_lineage {
        let old = TaskId::parse(&format!("{}-{}", e.from_prefix, pinned.task.id.hex))?;
        let source_dir = root.join(&e.from_subdir).join(format!("tasks/files/{old}"));
        if source_dir.join(name) != original {
            continue;
        }
        let b = Basis {
            project: e.from_prefix.clone(),
            layout: e.layout.clone(),
            records_subdir: e.from_subdir.clone(),
            commit: e.commit.clone(),
            task_path: task_path(&e.from_subdir, &old),
            retained_ref: e.retained_ref.clone(),
        };
        let source = basis::read_exact(project, &old, &b)?;
        let observed = source
            .task
            .notes
            .iter()
            .filter_map(|n| {
                n.text
                    .strip_prefix("retained: ")
                    .and_then(|r| decode_record(r).ok())
            })
            .any(|r| {
                r.identity() == record.identity()
                    && r.manifest.value() == Some(raw)
                    && r.sha256.value() == Some(digest)
            });
        if !observed {
            return Err(invalid(
                "generated manifest lacks its exact historical observation",
            ));
        }
        let loc = ArtifactLocator::Attachment {
            task: old.to_string(),
            name: name.into(),
            basis: Some(b),
            sha256: digest.into(),
        };
        super::verify::artifact(project, &loc, &mut super::verify::Metrics::default())?;
        let current = TaskId::parse(&format!("{}-{}", project.prefix, pinned.task.id.hex))?;
        crate::attachments::check_storage(project, &current)?;
        if crate::attachments::ledger(&pinned.task).get(name)
            != Some(&crate::attachments::Ledger::Attached)
        {
            return Err(invalid("moved manifest lacks a live attachment ledger"));
        }
        let path = crate::attachments::task_dir(project, &current).join(name);
        let bytes = super::verify::read_local(&path, project.attachments_max_bytes)?;
        if super::verify::hash_bytes(&bytes, &mut super::verify::Metrics::default()) != digest {
            return Err(invalid("moved manifest digest mismatch"));
        }
        return Ok(path);
    }
    Ok(original)
}
