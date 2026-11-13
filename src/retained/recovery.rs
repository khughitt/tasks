//! Explicit declarations for an in-tree source imported into detached records.
use super::model::*;
use crate::{error::Result, repo::Kind};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryDeclaration {
    pub version: u64,
    pub source_cutover: String,
    pub destination_import: String,
    pub source_ref: String,
    pub tasks: Vec<String>,
    pub refs: Vec<RefEndpoint>,
    pub manifest_bindings: Vec<ManifestBinding>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefEndpoint {
    pub original_ref: String,
    pub endpoint: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestBinding {
    pub task: String,
    pub ordinal: u64,
    pub attachment: String,
}
pub(crate) fn validate_shape(prefix: &str, kind: Kind, d: &RecoveryDeclaration) -> Result<()> {
    if d.version != 1 || kind != Kind::Detached || d.tasks.is_empty() || d.refs.is_empty() {
        return Err(invalid("unsupported recovery declaration"));
    }
    let mut tasks = BTreeSet::new();
    for task in &d.tasks {
        let id = TaskId::parse(task)?;
        if id.prefix != prefix || !tasks.insert(task) {
            return Err(invalid("duplicate or foreign recovery task"));
        }
    }
    let id = d.tasks.first().unwrap();
    source_basis(prefix, id, &d.source_cutover, &d.source_ref).validate()?;
    source_basis(prefix, id, &d.destination_import, &d.source_ref).validate()?;
    let mut refs = BTreeSet::new();
    for r in &d.refs {
        source_basis(prefix, id, &r.endpoint, &r.original_ref).validate()?;
        if !refs.insert(&r.original_ref) {
            return Err(invalid("duplicate recovery ref"));
        }
    }
    let mut bindings = BTreeSet::new();
    for binding in &d.manifest_bindings {
        crate::attachments::validate_name(&binding.attachment)?;
        if !tasks.contains(&binding.task)
            || binding.ordinal == 0
            || !bindings.insert((&binding.task, binding.ordinal))
        {
            return Err(invalid("duplicate or invalid recovery manifest binding"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;

use super::{
    basis,
    read::{self, ReadContext},
};
use crate::{
    model::{Task, TaskId},
    repo::Project,
};
use std::collections::BTreeSet;

pub(crate) struct ValidatedBoundary {
    source: ReadContext,
    destination: ReadContext,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Epoch {
    Source,
    Destination,
}
pub(crate) struct Selection {
    pub context: ReadContext,
    pub epoch: Epoch,
    pub publication_tip: String,
}
fn source_basis(prefix: &str, id: &str, commit: &str, reference: &str) -> Basis {
    Basis {
        project: prefix.into(),
        layout: "in-tree".into(),
        records_subdir: ".".into(),
        commit: commit.into(),
        task_path: format!("tasks/{id}.md"),
        retained_ref: reference.into(),
    }
}
fn current_basis(
    project: &Project,
    id: &TaskId,
    commit: String,
    reference: String,
) -> Result<Basis> {
    let subdir = super::lineage::subdir(project)?;
    Ok(Basis {
        project: project.prefix.clone(),
        layout: "detached".into(),
        task_path: super::lineage::task_path(&subdir, id),
        records_subdir: subdir,
        commit,
        retained_ref: reference,
    })
}
fn text(context: &ReadContext, args: &[&str]) -> Result<String> {
    String::from_utf8(basis::git_context(context, args)?).map_err(invalid)
}
fn ancestor(context: &ReadContext, older: &str, newer: &str) -> Result<()> {
    if !context.ancestor(older, newer)? {
        return Err(invalid("recovery retention ancestry is unavailable"));
    }
    Ok(())
}
fn entry(context: &ReadContext, commit: &str, path: &str) -> Result<Vec<u8>> {
    let raw = basis::git_context(context, &["ls-tree", "-z", commit, "--", path])?;
    if raw.is_empty() {
        return Ok(raw);
    }
    if raw.split(|b| *b == 0).filter(|v| !v.is_empty()).count() != 1 {
        return Err(invalid("ambiguous recovery tree entry"));
    }
    Ok(raw.split(|b| *b == b'\t').next().unwrap().to_vec())
}
pub(crate) fn validate_boundary(
    project: &Project,
    current_tasks: Option<&[Task]>,
) -> Result<ValidatedBoundary> {
    let d = project
        .retention_recovery
        .as_ref()
        .ok_or_else(|| invalid("missing recovery declaration"))?;
    validate_shape(&project.prefix, project.kind, d)?;
    if !project.retention_lineage.is_empty() {
        return Err(invalid("recovery cannot coexist with rename lineage"));
    }
    let source = read::at(project, &project.checkout)?;
    let destination = read::at(project, &project.records)?;
    if source.root() != project.checkout.canonicalize()? || source.root() == destination.root() {
        return Err(invalid(
            "recovery source must be the actual paired code Git root",
        ));
    }
    let boundary = ValidatedBoundary {
        source,
        destination,
    };
    if read::recovery_cached(project)? {
        return Ok(boundary);
    }
    let s = &boundary.source;
    let t = &boundary.destination;
    for context in [s, t] {
        if text(context, &["rev-parse", "--show-object-format"])?.trim() != "sha1" {
            return Err(invalid("recovery requires SHA-1 repositories"));
        }
    }
    // Require commit objects, not merely full-looking object names.
    for (context, commit) in [(s, &d.source_cutover), (t, &d.destination_import)] {
        text(
            context,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{commit}^{{commit}}"),
            ],
        )?;
    }
    text(s, &["show-ref", "--verify", &d.source_ref])?;
    ancestor(s, &d.source_cutover, &d.source_ref)?;
    for reference in &d.refs {
        ancestor(s, &reference.endpoint, &d.source_cutover)?;
    }
    let parents = text(
        t,
        &["rev-list", "--parents", "-n", "1", &d.destination_import],
    )?;
    if parents.split_whitespace().count() < 3 {
        return Err(invalid("recovery import is not a merge"));
    }
    let trailers = text(
        t,
        &[
            "show",
            "-s",
            "--format=%(trailers:only,unfold)",
            &d.destination_import,
        ],
    )?;
    for (key, expected) in [
        ("Records-Migrate-Prefix", project.prefix.as_str()),
        ("Records-Migrate-Source", d.source_cutover.as_str()),
        (
            "Records-Migrate-Paths",
            "tasks docs/specs docs/plans docs/notes docs/superpowers docs/designs docs/reports",
        ),
        ("Records-Migrate-Citations", "2"),
    ] {
        let values = trailers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.trim())
            .collect::<Vec<_>>();
        if values.len() != 1
            || values[0].split_whitespace().collect::<Vec<_>>()
                != expected.split_whitespace().collect::<Vec<_>>()
        {
            return Err(invalid(format!("invalid recovery import trailer {key}")));
        }
    }
    let sub = super::lineage::subdir(project)?;
    if sub != project.prefix {
        return Err(invalid(
            "recovery import prefix differs from current pairing",
        ));
    }
    let config_path = "tasks/.config.toml";
    let old_config = basis::blob(s, &d.source_cutover, config_path, None, &mut 0)?;
    let config = std::str::from_utf8(&old_config).map_err(invalid)?;
    let (prefix, kind, lineage) = Project::retention_config(config)?;
    let parsed: toml::Value = toml::from_str(config).map_err(invalid)?;
    if prefix != project.prefix
        || kind != crate::repo::Kind::InTree
        || !lineage.is_empty()
        || parsed.get("retention_recovery").is_some()
    {
        return Err(invalid(
            "recovery source configuration is not an unchained in-tree epoch",
        ));
    }
    if entry(s, &d.source_cutover, config_path)?
        != entry(t, &d.destination_import, &format!("{sub}/{config_path}"))?
    {
        return Err(invalid("recovery imported config blob differs"));
    }
    let head = basis::resolve(project, "HEAD")?;
    let reference = text(t, &["symbolic-ref", "-q", "HEAD"])?.trim().to_string();
    ancestor(t, &d.destination_import, &reference)?;
    for task in &d.tasks {
        for path in [format!("tasks/{task}.md"), format!("tasks/files/{task}")] {
            let old = entry(s, &d.source_cutover, &path)?;
            let imported = entry(t, &d.destination_import, &format!("{sub}/{path}"))?;
            if old != imported || (path.ends_with(".md") && old.is_empty()) {
                return Err(invalid(format!("recovery imported tree differs: {path}")));
            }
        }
        let source = source_basis(&project.prefix, task, &d.source_cutover, &d.source_ref);
        super::test_observer::event("recovery_prefix", task)?;
        let old = basis::load_task(s, &source, &mut 0)?;
        let id = TaskId::parse(task)?;
        let loaded;
        let current = match current_tasks {
            Some(tasks) => tasks
                .iter()
                .find(|t| t.id == id)
                .ok_or_else(|| invalid("recovery task missing from live records"))?,
            None => {
                loaded = basis::load_task(
                    t,
                    &current_basis(project, &id, head.clone(), reference.clone())?,
                    &mut 0,
                )?;
                &loaded
            }
        };
        if !current.notes.starts_with(&old.notes) {
            return Err(invalid(format!("recovery logical prefix differs: {task}")));
        }
    }
    read::remember_recovery(project)?;
    Ok(boundary)
}
fn epoch(project: &Project, id: &TaskId, b: &Basis) -> Result<Epoch> {
    b.validate()?;
    let d = project
        .retention_recovery
        .as_ref()
        .ok_or_else(|| invalid("missing recovery declaration"))?;
    if id.prefix != project.prefix || b.project != project.prefix {
        return Err(invalid("recovery Basis has a foreign owner"));
    }
    if b.layout == "in-tree"
        && b.records_subdir == "."
        && b.task_path == format!("tasks/{id}.md")
        && d.tasks.contains(&id.to_string())
        && d.refs.iter().any(|r| r.original_ref == b.retained_ref)
    {
        return Ok(Epoch::Source);
    }
    if b.layout == "detached"
        && b.records_subdir == project.prefix
        && b.task_path == format!("{}/tasks/{id}.md", project.prefix)
    {
        return Ok(Epoch::Destination);
    }
    Err(invalid(
        "recovery Basis does not match a declared repository epoch",
    ))
}
pub(crate) fn select(project: &Project, id: &TaskId, b: &Basis) -> Result<Option<Selection>> {
    if project.retention_recovery.is_none() {
        return Ok(None);
    }
    let epoch = epoch(project, id, b)?;
    let boundary = validate_boundary(project, None)?;
    let d = project.retention_recovery.as_ref().unwrap();
    let (context, tip) = match epoch {
        Epoch::Source => {
            let endpoint = &d
                .refs
                .iter()
                .find(|r| r.original_ref == b.retained_ref)
                .unwrap()
                .endpoint;
            (boundary.source, endpoint.clone())
        }
        Epoch::Destination => {
            text(
                &boundary.destination,
                &["show-ref", "--verify", &b.retained_ref],
            )?;
            ancestor(&boundary.destination, &d.destination_import, &b.commit)?;
            (boundary.destination, b.retained_ref.clone())
        }
    };
    ancestor(&context, &b.commit, &tip)?;
    Ok(Some(Selection {
        context,
        epoch,
        publication_tip: tip,
    }))
}
pub(crate) fn same_epoch(
    project: &Project,
    id: &TaskId,
    older: &Basis,
    newer: &Basis,
) -> Result<()> {
    if project.retention_recovery.is_some() {
        // Classify both tuples before any Git read or ancestry comparison.
        if epoch(project, id, older)? != epoch(project, id, newer)? {
            return Err(invalid("operation spans recovery repository epochs"));
        }
        let old = select(project, id, older)?.expect("declared recovery");
        let new = select(project, id, newer)?.expect("declared recovery");
        if old.epoch != new.epoch || old.context.root() != new.context.root() {
            return Err(invalid("operation spans recovery repositories"));
        }
    }
    Ok(())
}

/// Check declared operands before immutable proof reads can issue ancestry queries.
pub(crate) fn operation_epochs(project: &Project, task: &Task) -> Result<()> {
    let resolver = super::proof::Resolver::new(project);
    let mut seen = std::collections::BTreeMap::new();
    for note in &task.notes {
        let Some(raw) = note.text.strip_prefix("retention operation: ") else {
            continue;
        };
        let Ok(note) = decode_operation(raw) else {
            continue;
        };
        let mut operands = note
            .input_bases
            .iter()
            .map(|i| (&i.task, &i.basis))
            .collect::<Vec<_>>();
        match &note.outputs {
            PhaseOutputs::Completed(out) => {
                operands.extend(out.publications.iter().map(|p| (&p.task, &p.basis)))
            }
            PhaseOutputs::Reconciled(out) => {
                operands.extend(out.ready_bases.iter().map(|p| (&p.task, &p.basis)))
            }
            PhaseOutputs::Sender(out) => {
                let receiver = note
                    .input_bases
                    .get(1)
                    .ok_or_else(|| invalid("release lacks receiver"))?;
                operands.push((&receiver.task, &out.receiver_basis));
            }
            _ => {}
        }
        for (id, basis) in operands {
            let id = TaskId::parse(id)?;
            let p = resolver.task(project, &id.to_string())?;
            if p.retention_recovery.is_none() {
                continue;
            }
            let epoch = epoch(&p, &id, basis)?;
            let key = (
                note.key.coordinator.clone(),
                note.key.operation_id.clone(),
                id,
            );
            if seen
                .insert(key, epoch)
                .is_some_and(|previous| previous != epoch)
            {
                return Err(invalid("operation spans recovery repository epochs"));
            }
        }
    }
    Ok(())
}
pub(crate) fn require_current(project: &Project, id: &TaskId, basis: &Basis) -> Result<()> {
    if project.retention_recovery.is_some() && epoch(project, id, basis)? != Epoch::Destination {
        return Err(invalid(
            "source recovery epoch is read-only; start new operations with current detached inputs",
        ));
    }
    Ok(())
}

pub(crate) fn manifest_path(
    project: &Project,
    pinned: &basis::PinnedTask,
    source: &Source,
) -> Result<Option<std::path::PathBuf>> {
    let Some(d) = &project.retention_recovery else {
        return Ok(None);
    };
    let id = &pinned.task.id;
    if source.basis != pinned.basis {
        return Err(invalid(
            "manifest Source differs from its pinned read context",
        ));
    }
    let ordinal = usize::try_from(source.ordinal).map_err(invalid)?;
    let note = ordinal
        .checked_sub(1)
        .and_then(|i| pinned.task.notes.get(i))
        .ok_or_else(|| invalid("manifest source ordinal unavailable"))?;
    let record = note
        .text
        .strip_prefix("retained: ")
        .ok_or_else(|| invalid("manifest source is not a retained note"))
        .and_then(decode_record)?;
    let raw = record
        .manifest
        .value()
        .ok_or_else(|| invalid("manifest source has no manifest"))?;
    let original = super::verify::current_path(project, raw)?;
    let name = original
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| invalid("manifest source filename"))?;
    let local = crate::attachments::task_dir(project, id).join(name);
    let binding = d
        .manifest_bindings
        .iter()
        .find(|b| b.task == id.to_string() && b.ordinal == source.ordinal);
    let Some(binding) = binding else {
        if original != local && original.ends_with(format!("tasks/files/{id}/{name}")) {
            return Err(invalid(
                "migrated manifest source has no explicit recovery binding",
            ));
        }
        return Ok(None);
    };
    if name != binding.attachment
        || !original.ends_with(format!("tasks/files/{id}/{}", binding.attachment))
    {
        return Err(invalid(
            "recovery binding does not name the original attached manifest string",
        ));
    }
    let boundary = validate_boundary(project, None)?;
    let b = source_basis(
        &project.prefix,
        &id.to_string(),
        &d.source_cutover,
        &d.source_ref,
    );
    let mut old = basis::load_task(&boundary.source, &b, &mut 0)?;
    if old.notes.get(ordinal - 1) != Some(note) {
        return Err(invalid(
            "manifest binding does not match its exact source-cutover note",
        ));
    }
    let digest = record
        .sha256
        .value()
        .ok_or_else(|| invalid("bound manifest lacks a digest"))?;
    let path = format!("tasks/files/{id}/{}", binding.attachment);
    let bytes = basis::blob(
        &boundary.source,
        &d.source_cutover,
        &path,
        Some(project.attachments_max_bytes),
        &mut 0,
    )?;
    if super::verify::hash_bytes(&bytes, &mut super::verify::Metrics::default()) != digest {
        return Err(invalid("source recovery manifest digest mismatch"));
    }
    // A generated name requires its original observation phase, unlike a manual attachment.
    if binding.attachment == format!("retained-manifest-{digest}.json") {
        let mut observed = false;
        for phase in &old.notes {
            let Some(raw) = phase.text.strip_prefix("retention operation: ") else {
                continue;
            };
            let Ok(phase) = decode_operation(raw) else {
                continue;
            };
            let ordinals = match &phase.outputs {
                PhaseOutputs::Items(out) => &out.items,
                PhaseOutputs::Receiver(out) => &out.items,
                PhaseOutputs::Ready(out) => &out.items,
                _ => continue,
            };
            if !ordinals.contains(&source.ordinal) {
                continue;
            }
            observed=super::publish::observation_artifacts(project,&old,&phase)?.iter().any(|a|matches!(a,ArtifactLocator::Attachment{task,name,sha256,..} if task==&id.to_string()&&name==&binding.attachment&&sha256==digest));
            if observed {
                break;
            }
        }
        if !observed {
            return Err(invalid(
                "generated recovery manifest lacks its source observation binding",
            ));
        }
    }
    old.notes.truncate(ordinal - 1);
    if crate::attachments::ledger(&old).get(&binding.attachment)
        != Some(&crate::attachments::Ledger::Attached)
    {
        return Err(invalid(
            "source manifest was not attached before its observation",
        ));
    }
    crate::attachments::check_storage(project, id)?;
    if crate::attachments::ledger(&pinned.task).get(&binding.attachment)
        != Some(&crate::attachments::Ledger::Attached)
    {
        return Err(invalid("bound manifest lacks its current attached ledger"));
    }
    let current = super::verify::read_local(&local, project.attachments_max_bytes)?;
    if current != bytes {
        return Err(invalid(
            "current recovery manifest differs from its original bytes",
        ));
    }
    Ok(Some(local))
}
