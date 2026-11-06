use super::{basis, model::*, publish, reconcile, state, verify};
use crate::{error::Result, model::TaskId, repo::Project};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn project_for(project: &Project, task: &str) -> Result<Project> {
    let id = TaskId::parse(task)?;
    if super::lineage::owns_prefix(project, &id.prefix) {
        Ok(project.clone())
    } else {
        crate::scope::open_registered(
            &crate::registry::Registry::load()?,
            &id.prefix,
            crate::scope::Origin::Id(&id),
        )
    }
}

// Validate immutable metadata only; current payloads and working attachment files are irrelevant.
pub fn validate_history(
    project: &Project,
    pinned: &basis::PinnedTask,
    host: Option<&str>,
) -> Result<()> {
    let mut pending = vec![(project.clone(), pinned.clone())];
    let mut seen = BTreeSet::new();
    let mut metrics = verify::Metrics::default();
    while let Some((project, pinned)) = pending.pop() {
        if !seen.insert((
            pinned.basis.project.clone(),
            pinned.basis.commit.clone(),
            pinned.basis.task_path.clone(),
            pinned.basis.retained_ref.clone(),
        )) {
            continue;
        }
        let mut requests = BTreeMap::new();
        for (index, entry) in pinned.task.notes.iter().enumerate() {
            let Some(raw) = entry.text.strip_prefix("retention operation: ") else {
                continue;
            };
            // Malformed envelopes remain scoped diagnostics in replay.
            let Ok(note) = decode_operation(raw) else {
                continue;
            };
            if host.is_some_and(|h| h != note.host) {
                continue;
            }
            let input = super::lineage::participant(&project, &note, &pinned.task.id)?;
            let original_id = TaskId::parse(&input.task)?;
            let publication = basis::publication_basis(
                &project,
                &pinned.task.id,
                &input.basis,
                &pinned.task.notes[..=index],
                &mut metrics.history_bytes_read,
            )?;
            let request = verify::artifact(
                &project,
                &note
                    .request
                    .pinned(&original_id.to_string(), &publication)?,
                &mut metrics,
            )?;
            reconcile::validate_original(&request, &note)?;
            let key = (note.key.coordinator.clone(), note.key.operation_id.clone());
            if requests
                .insert(key, request.clone())
                .is_some_and(|old| old != request)
            {
                return Err(invalid(
                    "operation phases name different immutable requests",
                ));
            }
            for artifact in publish::observation_artifacts(&project, &pinned.task, &note)?
                .into_iter()
                .skip(1)
            {
                let artifact = artifact.pinned(&original_id.to_string(), &publication)?;
                let file = match &artifact {
                    ArtifactLocator::Directory { manifest, .. } => manifest.as_ref(),
                    _ => &artifact,
                };
                verify::artifact(&project, file, &mut metrics)?;
            }
            for input in &note.input_bases {
                let p = project_for(&project, &input.task)?;
                let id = TaskId::parse(&input.task)?;
                let original = basis::read_exact(&p, &id, &input.basis)?;
                if super::lineage::same(&project, &input.task, &pinned.task.id.to_string())? {
                    if !pinned.task.notes[..index].starts_with(&original.task.notes) {
                        return Err(invalid(
                            "operation does not preserve its exact input history",
                        ));
                    }
                    basis::git(
                        &p,
                        &[
                            "merge-base",
                            "--is-ancestor",
                            &input.basis.commit,
                            &publication.commit,
                        ],
                    )?;
                }
            }
            let publications = match &note.outputs {
                PhaseOutputs::Completed(out) => out.publications.clone(),
                PhaseOutputs::Reconciled(out) => out.ready_bases.clone(),
                PhaseOutputs::Sender(out) => vec![Publication {
                    task: note
                        .input_bases
                        .get(1)
                        .ok_or_else(|| invalid("release lacks receiver"))?
                        .task
                        .clone(),
                    phase: "receiver_published".into(),
                    basis: out.receiver_basis.clone(),
                }],
                _ => Vec::new(),
            };
            for publication in publications {
                if !note.input_bases.iter().any(|i| i.task == publication.task) {
                    return Err(invalid("publication names a foreign operation participant"));
                }
                let p = project_for(&project, &publication.task)?;
                let id = TaskId::parse(&publication.task)?;
                let target = basis::read_exact(&p, &id, &publication.basis)?;
                let replay = state::replay(&p, &target.task, &target.basis, host)?;
                let source = replay
                    .operations
                    .iter()
                    .rev()
                    .find(|o| o.note.key == note.key && o.note.phase == publication.phase)
                    .ok_or_else(|| invalid("publication lacks its exact valid operation phase"))?;
                let actual = basis::publication_basis(
                    &p,
                    &id,
                    &publication.basis,
                    &target.task.notes[..source.source.ordinal as usize],
                    &mut metrics.history_bytes_read,
                )?;
                if actual != publication.basis
                    || source.note.host != note.host
                    || source.note.input_bases != note.input_bases
                    || verify::artifact(
                        &p,
                        &source.note.request.pinned(&publication.task, &actual)?,
                        &mut metrics,
                    )? != request
                {
                    return Err(invalid(
                        "publication conflicts with its immutable operation binding",
                    ));
                }
                pending.push((p, target));
            }
        }
    }
    Ok(())
}
