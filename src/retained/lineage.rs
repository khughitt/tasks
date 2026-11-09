use super::{basis, model::*};
use crate::{
    error::{Error, Result},
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
pub fn validate_shape(prefix: &str, kind: crate::repo::Kind, edges: &[RenameEdge]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (i, e) in edges.iter().enumerate() {
        let id = TaskId::parse(&format!("{}-000001", e.from_prefix))?;
        source_basis(&id, e).validate()?;
        if e.version != 1
            || !is_valid_prefix(&e.to_prefix)
            || e.from_prefix == e.to_prefix
            || !(e.to_subdir == "." || member_path(&e.to_subdir))
            || !seen.insert(&e.from_prefix)
            || seen.contains(&e.to_prefix)
            || e.layout != kind.as_str()
        {
            return Err(invalid("conflicting or unsupported retention lineage"));
        }
        if let Some(previous) = i.checked_sub(1).map(|i| &edges[i])
            && (previous.to_prefix != e.from_prefix || previous.to_subdir != e.from_subdir)
        {
            return Err(invalid("noncontiguous retention lineage"));
        }
    }
    if edges.last().is_some_and(|e| e.to_prefix != prefix) {
        return Err(invalid(
            "retention lineage endpoint differs from project prefix",
        ));
    }
    Ok(())
}
pub fn validate(project: &Project) -> Result<()> {
    let edges = &project.retention_lineage;
    validate_shape(&project.prefix, project.kind, edges)?;
    if edges.is_empty() {
        return Ok(());
    }
    for (i, e) in edges.iter().enumerate() {
        if let Some(previous) = i.checked_sub(1).map(|i| &edges[i]) {
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
        let (prefix, kind, prior) =
            Project::retention_config(std::str::from_utf8(&raw).map_err(invalid)?)?;
        if prefix != e.from_prefix || prior != edges[..i] || kind != project.kind {
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
pub fn epoch_subdir(project: &Project, prefix: &str) -> Result<String> {
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
    for (index, (prefix, sub)) in epochs.into_iter().enumerate() {
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
        let (_, kind, prior) =
            Project::retention_config(std::str::from_utf8(&raw).map_err(invalid)?)?;
        if kind != project.kind || prior != project.retention_lineage[..index] {
            return Err(invalid(
                "historical configuration differs from the declared epoch",
            ));
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

fn check_protected_storage(project: &Project, task: &Task) -> Result<()> {
    if crate::attachments::check_storage(project, &task.id)? {
        let ledger = crate::attachments::ledger(task);
        for entry in std::fs::read_dir(crate::attachments::task_dir(project, &task.id))? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str()
                && super::publish::protected(name)
                && ledger.get(name) != Some(&crate::attachments::Ledger::Attached)
            {
                return Err(invalid(
                    "physical protected artifact lacks a live attachment ledger binding",
                ));
            }
        }
    }
    Ok(())
}
pub(crate) fn relevant(task: &Task) -> bool {
    task.notes.iter().any(|n| {
        ["retained:", "retention operation:", "retention checkpoint:"]
            .iter()
            .any(|p| n.text.starts_with(p))
    }) || crate::attachments::ledger(task)
        .keys()
        .any(|n| super::publish::protected(n))
}
fn source_basis(id: &TaskId, e: &RenameEdge) -> Basis {
    Basis {
        project: e.from_prefix.clone(),
        layout: e.layout.clone(),
        records_subdir: e.from_subdir.clone(),
        commit: e.commit.clone(),
        task_path: task_path(&e.from_subdir, id),
        retained_ref: e.retained_ref.clone(),
    }
}
fn protected_bytes(project: &Project, pinned: &basis::PinnedTask) -> Result<()> {
    use std::collections::BTreeMap;
    let mut expected = BTreeMap::new();
    for note in &pinned.task.notes {
        let Some(raw) = note.text.strip_prefix("retention operation: ") else {
            continue;
        };
        let operation = decode_operation(raw)?;
        for locator in super::publish::observation_artifacts(project, &pinned.task, &operation)? {
            let file = match &locator {
                ArtifactLocator::Directory { manifest, .. } => manifest.as_ref(),
                _ => &locator,
            };
            if let ArtifactLocator::Attachment {
                task, name, sha256, ..
            } = file
                && same(project, task, &pinned.task.id.to_string())?
                && expected
                    .insert(name.clone(), sha256.clone())
                    .is_some_and(|old| old != *sha256)
            {
                return Err(invalid("protected artifact has conflicting digests"));
            }
        }
    }
    for name in crate::attachments::ledger(&pinned.task)
        .keys()
        .filter(|n| super::publish::protected(n))
    {
        let path = format!(
            "{}/tasks/files/{}/{name}",
            pinned.basis.records_subdir, pinned.task.id
        )
        .trim_start_matches("./")
        .to_string();
        let bytes = basis::blob(
            project,
            &pinned.basis.commit,
            &path,
            Some(project.attachments_max_bytes),
            &mut 0,
        )?;
        let digest = super::verify::hash_bytes(&bytes, &mut super::verify::Metrics::default());
        let declared = expected.get(name).is_some_and(|v| v == &digest);
        // Interrupted-publication snapshots bind their own bytes and an operation already proved above.
        let backup = if name == &format!("retained-checkpoint-{digest}.json") {
            let v = strict(&bytes)?;
            let key = v
                .get("operation")
                .cloned()
                .map(serde_json::from_value::<OperationKey>)
                .transpose()
                .map_err(invalid)?;
            v.get("version").and_then(|v| v.as_u64()) == Some(1)
                && key.is_some_and(|key| {
                    pinned
                        .task
                        .notes
                        .iter()
                        .filter_map(|n| {
                            n.text
                                .strip_prefix("retention operation: ")
                                .and_then(|r| decode_operation(r).ok())
                        })
                        .any(|n| n.key == key)
                })
        } else {
            false
        };
        let manifest = pinned
            .task
            .notes
            .iter()
            .filter_map(|n| {
                n.text
                    .strip_prefix("retained: ")
                    .and_then(|r| decode_record(r).ok())
            })
            .any(|r| {
                r.sha256.value() == Some(digest.as_str())
                    && r.manifest.value().is_some_and(|p| {
                        Path::new(p).file_name().and_then(|n| n.to_str()) == Some(name.as_str())
                    })
                    && name == &format!("retained-manifest-{digest}.json")
            });
        if !(declared || backup || manifest) {
            return Err(invalid(
                "protected artifact lacks a verified retention binding",
            ));
        }
        let current = TaskId::parse(&format!("{}-{}", project.prefix, pinned.task.id.hex))?;
        crate::attachments::check_storage(project, &current)?;
        let live = super::verify::read_local(
            &crate::attachments::task_dir(project, &current).join(name),
            project.attachments_max_bytes,
        )?;
        if live != bytes {
            return Err(invalid(
                "live protected artifact differs from exact source bytes",
            ));
        }
    }
    Ok(())
}
/// Rename carries every note byte unchanged, so a malformed plain `retained:` item note
/// reads the same on both sides of it and does not block it. Operation and checkpoint
/// envelopes bind the evidence rename must preserve: any unresolved diagnostic on one
/// refuses. An operation citing a malformed `retained:` ordinal is itself such a
/// diagnostic. An unsupported item version also refuses: an older reader is renaming.
fn envelopes_complete(project: &Project, pinned: &basis::PinnedTask) -> Result<bool> {
    let state = super::state::replay(project, &pinned.task, &pinned.basis, None)?;
    Ok(state
        .diagnostics
        .iter()
        .filter(|d| d.resolved_by.is_none())
        .all(|d| {
            d.code == "malformed_record"
                && d.source
                    .as_ref()
                    .and_then(|s| usize::try_from(s.ordinal).ok()?.checked_sub(1))
                    .and_then(|i| pinned.task.notes.get(i))
                    .is_some_and(|n| n.text.starts_with("retained: "))
        }))
}
pub fn prepare(project: &Project, target: &str) -> Result<Option<RenameEdge>> {
    let tasks = project.scan()?;
    for task in &tasks {
        check_protected_storage(project, task)?;
    }
    if project.retention_lineage.is_empty() && !tasks.iter().any(relevant) {
        return Ok(None);
    }
    if project
        .uncommitted_task_files()?
        .is_some_and(|v| !v.is_empty())
    {
        return Err(invalid(
            "retention rename needs clean tasks/ including tasks/.config.toml at HEAD",
        ));
    }
    validate(project)?;
    let commit = basis::resolve(project, "HEAD")?;
    let reference = String::from_utf8(basis::git(project, &["symbolic-ref", "-q", "HEAD"])?)
        .map_err(invalid)?
        .trim()
        .to_string();
    ancestor(project, &commit, &reference)?;
    let sub = subdir(project)?;
    if project.kind == crate::repo::Kind::Detached && sub == "." {
        return Err(Error::RetainedPublicationUnsupported("This detached rename would relocate the records Git root; preserve its locators through an explicit relocation route first".into()));
    }
    let config_path = format!("{sub}/tasks/.config.toml")
        .trim_start_matches("./")
        .to_string();
    if basis::blob(project, &commit, &config_path, None, &mut 0)?
        != std::fs::read(project.records.join(crate::repo::CONFIG_REL))?
    {
        return Err(invalid("source config differs from its committed basis"));
    }
    for task in &tasks {
        super::publish::permission(project, task, super::publish::WriteAction::Retire)?;
        let pinned = basis::read(project, &task.id, None)?;
        if &pinned.task != task {
            return Err(invalid("source task differs from its committed basis"));
        }
        super::proof::validate_history(project, &pinned, None)?;
        super::reconcile::validate_terminal_history(project, &pinned)?;
        if !envelopes_complete(project, &pinned)? {
            return Err(invalid("incomplete retention history cannot be renamed"));
        }
        protected_bytes(project, &pinned)?;
    }
    let to_subdir = if project.kind == crate::repo::Kind::Detached {
        let path = Path::new(&sub).with_file_name(target);
        path.to_str()
            .ok_or_else(|| invalid("target subdir"))?
            .to_string()
    } else {
        sub.clone()
    };
    Ok(Some(RenameEdge {
        version: 1,
        from_prefix: project.prefix.clone(),
        to_prefix: target.into(),
        layout: project.kind.as_str().into(),
        from_subdir: sub,
        to_subdir,
        commit,
        retained_ref: reference,
    }))
}
pub fn check_live(project: &Project, tasks: &[Task]) -> Result<()> {
    validate(project)?;
    if project.retention_lineage.is_empty() {
        return Ok(());
    }
    for task in tasks {
        check_protected_storage(project, task)?;
        for edge in &project.retention_lineage {
            let old = TaskId::parse(&format!("{}-{}", edge.from_prefix, task.id.hex))?;
            let b = source_basis(&old, edge);
            if basis::git(project, &["ls-tree", "-z", &b.commit, "--", &b.task_path])?.is_empty() {
                continue;
            }
            let pinned = basis::read_exact(project, &task.id, &b)?;
            if !task.notes.starts_with(&pinned.task.notes) {
                return Err(invalid("live task changed its exact rename source history"));
            }
            super::proof::validate_history(project, &pinned, None)?;
            super::reconcile::validate_terminal_history(project, &pinned)?;
            if !envelopes_complete(project, &pinned)? {
                return Err(invalid("rename source retention history is incomplete"));
            }
            protected_bytes(project, &pinned)?;
        }
        let path = task_path(&subdir(project)?, &task.id);
        if !basis::git(project, &["ls-tree", "-z", "HEAD", "--", &path])?.is_empty() {
            let pinned = basis::read(project, &task.id, Some("HEAD"))?;
            super::proof::validate_history(project, &pinned, None)?;
            super::reconcile::validate_terminal_history(project, &pinned)?;
        }
    }
    Ok(())
}

pub fn check_staged(project: &Project, edge: &RenameEdge, ids: &[TaskId]) -> Result<()> {
    use crate::attachments::{DirState, dir_state};
    for old in ids {
        let b = source_basis(old, edge);
        let task = basis::task_at(project, old, &b, &mut 0)?;
        let protected = crate::attachments::ledger(&task)
            .keys()
            .filter(|n| super::publish::protected(n))
            .cloned()
            .collect::<Vec<_>>();
        if protected.is_empty() {
            continue;
        }
        let new = TaskId::parse(&format!("{}-{}", edge.to_prefix, old.hex))?;
        let source = crate::attachments::task_dir(project, old);
        let target = crate::attachments::task_dir(project, &new);
        let live = match (dir_state(&source)?, dir_state(&target)?) {
            (DirState::Directory, DirState::Absent) => source,
            (DirState::Absent, DirState::Directory) => target,
            _ => {
                return Err(invalid(
                    "staged protected attachment location is missing or ambiguous",
                ));
            }
        };
        for name in protected {
            let path = format!("{}/tasks/files/{old}/{name}", edge.from_subdir)
                .trim_start_matches("./")
                .to_string();
            let expected = basis::blob(
                project,
                &edge.commit,
                &path,
                Some(project.attachments_max_bytes),
                &mut 0,
            )?;
            if super::verify::read_local(&live.join(&name), project.attachments_max_bytes)?
                != expected
            {
                return Err(invalid(
                    "staged protected evidence differs from its frozen rename source",
                ));
            }
        }
    }
    Ok(())
}
