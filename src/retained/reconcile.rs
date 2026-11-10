use super::{
    basis,
    model::*,
    publish::{self, Binding, GeneratedNote, NewArtifact, OperationCtx},
    state, verify,
};
use crate::{
    error::{Error, Result},
    model::{Task, TaskId},
    repo::Project,
};
use std::collections::{BTreeMap, BTreeSet};

fn phases(task: &Task, key: &OperationKey) -> Result<Vec<(usize, OperationNote)>> {
    let mut out = Vec::new();
    let mut previous = None;
    for (index, n) in task.notes.iter().enumerate() {
        if let Some(raw) = n.text.strip_prefix("retention operation: ") {
            let op = decode_operation(raw)?;
            if op.key == *key {
                validate_transition(previous.as_ref(), &op)?;
                previous = Some(op.clone());
                out.push((index, op));
            }
        }
    }
    Ok(out)
}
pub(super) fn validate_original(bytes: &[u8], note: &OperationNote) -> Result<()> {
    if note
        .input_bases
        .first()
        .is_none_or(|i| i.task != note.key.coordinator)
    {
        return Err(invalid("original request coordinator mismatch"));
    }
    if note.input_bases.len() == 1 {
        let request: CheckpointRequest = decode_request(bytes)?;
        request.validate()?;
        if request.inputs != note.input_bases
            || request.host != note.host
            || request.operation_id != note.key.operation_id
        {
            return Err(invalid("original checkpoint binding mismatch"));
        }
    } else {
        let request: TransferRequest = decode_request(bytes)?;
        request.validate()?;
        if request.inputs != note.input_bases
            || request.host != note.host
            || request.operation_id != note.key.operation_id
        {
            return Err(invalid("original transfer binding mismatch"));
        }
    }
    Ok(())
}
fn load_original(ctx: &mut OperationCtx, from: &TaskId) -> Result<Binding> {
    let project = ctx.project(from).clone();
    let notes = phases(&ctx.tasks[from].task, &ctx.key)?;
    let (index, intent) = notes
        .first()
        .filter(|(_, n)| n.phase == "intent")
        .ok_or_else(|| invalid("reconciliation requires a published original intent"))?;
    let context = intent
        .input_bases
        .iter()
        .find(|i| i.task == from.to_string())
        .ok_or_else(|| invalid("original coordinator input missing"))?
        .basis
        .clone();
    let publication = basis::publication_basis(
        &project,
        from,
        &context,
        &ctx.tasks[from].task.notes[..=*index],
        &mut ctx.metrics.history_bytes_read,
    )?;
    let loc = intent.request.pinned(&from.to_string(), &publication)?;
    let bytes = verify::artifact(&project, &loc, &mut ctx.metrics)?;
    let name = match &intent.request {
        ArtifactLocator::Attachment { task, name, .. } if *task == from.to_string() => name.clone(),
        _ => {
            return Err(invalid(
                "original request requires its coordinator attachment",
            ));
        }
    };
    validate_original(&bytes, intent)?;
    if intent.host != crate::claims::hostname() {
        return Err(invalid(
            "reconciliation is restricted to the original executing host",
        ));
    }
    publish::validate_bases(ctx, &intent.input_bases)?;
    for input in &intent.input_bases {
        let id = TaskId::parse(&input.task)?;
        let original = basis::read_exact(ctx.project(&id), &id, &input.basis)?;
        ctx.metrics.history_bytes_read += original.history_bytes_read;
        if !ctx.tasks[&id].task.notes.starts_with(&original.task.notes) {
            return Err(Error::RetainedHistoryBlocked("Original operation history was rewritten; recover its exact committed prefix before reconciliation".into()));
        }
    }
    if intent.input_bases.len() == 2 {
        let request: TransferRequest = decode_request(&bytes)?;
        let source = basis::read_exact(&project, from, &request.inputs[0].basis)?;
        ctx.metrics.history_bytes_read += source.history_bytes_read;
        let state = state::replay(&project, &source.task, &source.basis, None)?;
        for identity in &request.items {
            let record = state
                .items
                .iter()
                .find(|i| i.identity == *identity)
                .and_then(|i| i.record.as_ref())
                .ok_or_else(|| invalid("original transfer selection missing"))?;
            if record.state != "kept"
                || !super::lineage::owner_matches(&project, &record.next_owner, from)?
            {
                return Err(invalid(
                    "original transfer selection lacks sender custody in its pinned input",
                ));
            }
        }
        let to = TaskId::parse(&request.inputs[1].task)?;
        for (index, note) in phases(&ctx.tasks[&to].task, &ctx.key)? {
            if let PhaseOutputs::Receiver(output) = note.outputs {
                if output
                    .mapping
                    .iter()
                    .map(|m| &m.sender)
                    .ne(request.items.iter())
                    || output.items.len() != output.mapping.len()
                {
                    return Err(invalid(
                        "published receiver mapping differs from original selection",
                    ));
                }
                for (ordinal, mapping) in output.items.iter().zip(&output.mapping) {
                    let source = ordinal
                        .checked_sub(1)
                        .and_then(|o| usize::try_from(o).ok())
                        .filter(|o| *o < index)
                        .and_then(|o| ctx.tasks[&to].task.notes.get(o))
                        .and_then(|n| n.text.strip_prefix("retained: "))
                        .ok_or_else(|| invalid("published receiver source missing"))?;
                    let r = decode_record(source)?;
                    if r.version != Some(1)
                        || r.state != "kept"
                        || r.identity() != mapping.receiver
                        || !super::lineage::owner_matches(ctx.project(&to), &r.next_owner, &to)?
                        || r.host != mapping.sender.host
                        || r.kind != mapping.sender.kind
                        || (r.kind != "worktree" || from.prefix == to.prefix)
                            && r.path != mapping.sender.path
                    {
                        return Err(invalid(
                            "published receiver source does not bind its custody mapping",
                        ));
                    }
                }
            }
        }
    }
    Ok(Binding {
        bytes,
        request: intent.request.clone(),
        name,
        inputs: intent.input_bases.clone(),
        host: intent.host.clone(),
    })
}
fn directive_info(note: &OperationNote) -> Option<(&ArtifactLocator, &str)> {
    match &note.outputs {
        PhaseOutputs::ReconcileIntent(o) => Some((&o.directive, &o.reconciliation_id)),
        PhaseOutputs::Ready(o) => Some((&o.directive, &o.reconciliation_id)),
        PhaseOutputs::Reconciled(o) => Some((&o.directive, &o.reconciliation_id)),
        _ => None,
    }
}
fn ready_proofs(
    project: &Project,
    coordinator: &basis::PinnedTask,
    terminal: &OperationNote,
    metrics: &mut verify::Metrics,
) -> Result<Vec<basis::PinnedTask>> {
    ready_proofs_with(
        &super::proof::Resolver::new(project),
        project,
        coordinator,
        terminal,
        metrics,
    )
}
fn ready_proofs_with(
    resolver: &super::proof::Resolver<'_>,
    project: &Project,
    coordinator: &basis::PinnedTask,
    terminal: &OperationNote,
    metrics: &mut verify::Metrics,
) -> Result<Vec<basis::PinnedTask>> {
    if !super::lineage::same(
        project,
        &terminal.key.coordinator,
        &coordinator.task.id.to_string(),
    )? {
        return Err(invalid(
            "reconciled terminal must be published on its coordinator",
        ));
    }
    let PhaseOutputs::Reconciled(out) = &terminal.outputs else {
        return Err(invalid("terminal reconciliation shape"));
    };
    let expected = terminal
        .input_bases
        .iter()
        .map(|i| i.task.as_str())
        .collect::<BTreeSet<_>>();
    if out
        .ready_bases
        .iter()
        .map(|p| p.task.as_str())
        .collect::<BTreeSet<_>>()
        != expected
        || out.ready_bases.len() != expected.len()
    {
        return Err(invalid(
            "terminal reconciliation lacks exact participant ready publications",
        ));
    }
    let publication = if terminal.key.coordinator == coordinator.task.id.to_string() {
        coordinator.basis.clone()
    } else {
        let input = super::lineage::participant(project, terminal, &coordinator.task.id)?;
        let ordinal = coordinator
            .task
            .notes
            .iter()
            .position(|n| {
                n.text
                    .strip_prefix("retention operation: ")
                    .and_then(|r| decode_operation(r).ok())
                    .is_some_and(|n| n == *terminal)
            })
            .ok_or_else(|| invalid("terminal source missing"))?;
        basis::publication_basis(
            project,
            &coordinator.task.id,
            &input.basis,
            &coordinator.task.notes[..=ordinal],
            &mut metrics.history_bytes_read,
        )?
    };
    let expected_request = verify::artifact_with(
        resolver,
        project,
        &terminal
            .request
            .pinned(&terminal.key.coordinator, &publication)?,
        metrics,
    )?;
    let expected_directive = verify::artifact_with(
        resolver,
        project,
        &out.directive
            .pinned(&terminal.key.coordinator, &publication)?,
        metrics,
    )?;
    validate_original(&expected_request, terminal)?;
    if phases(&coordinator.task, &terminal.key)?
        .iter()
        .any(|(_, n)| n.phase == "sender_released")
    {
        return Err(invalid("a committed sender release forbids reconciliation"));
    }
    let directive: ReconcileRequest = decode_request(&expected_directive)?;
    directive.validate()?;
    if directive.operation != terminal.key
        || directive.reconciliation_id != out.reconciliation_id
        || directive
            .inputs
            .iter()
            .map(|i| i.task.as_str())
            .collect::<BTreeSet<_>>()
            != expected
    {
        return Err(invalid(
            "terminal directive does not bind the original operation and participants",
        ));
    }
    let mut unresolved = BTreeSet::new();
    let mut ready = Vec::new();
    for publication in &out.ready_bases {
        if publication.phase != "reconcile_ready" {
            return Err(invalid("terminal refers to a non-ready publication"));
        }
        let id = TaskId::parse(&publication.task)?;
        let p = resolver.task(project, &publication.task)?;
        let pinned = basis::read_exact(&p, &id, &publication.basis)?;
        metrics.history_bytes_read += pinned.history_bytes_read;
        for input in [
            directive
                .inputs
                .iter()
                .find(|i| i.task == publication.task)
                .ok_or_else(|| invalid("directive participant missing"))?,
            terminal
                .input_bases
                .iter()
                .find(|i| i.task == publication.task)
                .ok_or_else(|| invalid("original participant missing"))?,
        ] {
            let original = basis::read_exact(&p, &id, &input.basis)?;
            metrics.history_bytes_read += original.history_bytes_read;
            if !pinned.task.notes.starts_with(&original.task.notes)
                || input.basis.retained_ref != pinned.basis.retained_ref
            {
                return Err(invalid(
                    "ready publication does not preserve its exact input history",
                ));
            }
            basis::git(
                &p,
                &[
                    "merge-base",
                    "--is-ancestor",
                    &input.basis.commit,
                    &pinned.basis.commit,
                ],
            )?;
        }
        let (_, note) = phases(&pinned.task, &terminal.key)?
            .into_iter()
            .last()
            .ok_or_else(|| invalid("ready operation history missing"))?;
        let PhaseOutputs::Ready(output) = &note.outputs else {
            return Err(invalid("cited publication does not end at ready"));
        };
        if note.input_bases != terminal.input_bases
            || note.host != terminal.host
            || output.reconciliation_id != out.reconciliation_id
            || verify::artifact_with(
                resolver,
                project,
                &note.request.pinned(&id.to_string(), &pinned.basis)?,
                metrics,
            )? != expected_request
            || verify::artifact_with(
                resolver,
                project,
                &output.directive.pinned(&id.to_string(), &pinned.basis)?,
                metrics,
            )? != expected_directive
        {
            return Err(invalid(
                "ready publication conflicts with terminal directive or original request",
            ));
        }
        let state = state::replay(&p, &pinned.task, &pinned.basis, None)?;
        if !state.operations.iter().any(|o| o.note == note) {
            return Err(invalid(
                "ready observations do not bind to exact retained sources",
            ));
        }
        unresolved.extend(output.unresolved.iter().cloned());
        ready.push(pinned);
    }
    if out.unresolved.iter().cloned().collect::<BTreeSet<_>>() != unresolved
        || out.unresolved.len() != unresolved.len()
    {
        return Err(invalid(
            "terminal reconciliation omits or changes ready uncertainty actions",
        ));
    }
    Ok(ready)
}
pub fn validate_terminal_history(project: &Project, pinned: &basis::PinnedTask) -> Result<()> {
    for note in &pinned.task.notes {
        if let Some(raw) = note.text.strip_prefix("retention operation: ")
            && let Ok(operation) = decode_operation(raw)
            && operation.phase == "reconciled"
        {
            ready_proofs(project, pinned, &operation, &mut verify::Metrics::default())?;
        }
    }
    Ok(())
}
// A ready participant is free only after its coordinator's exact published terminal proof.
pub fn reservation_ended(project: &Project, task: &Task, note: &OperationNote) -> Result<bool> {
    if !matches!(note.phase.as_str(), "reconcile_ready" | "reconciled") {
        return Ok(false);
    }
    let from = TaskId::parse(&note.key.coordinator)?;
    if note.phase == "reconciled"
        && !super::lineage::same(project, &task.id.to_string(), &from.to_string())?
    {
        return Err(invalid(
            "participant cannot assert a coordinator terminal receipt",
        ));
    }
    let p = if super::lineage::owns_prefix(project, &from.prefix) {
        project.clone()
    } else {
        crate::scope::open_registered(
            &crate::registry::Registry::load()?,
            &from.prefix,
            crate::scope::Origin::Id(&from),
        )?
    };
    let p = super::read::inherit(project, p)?;
    let reference = note
        .input_bases
        .iter()
        .find(|i| i.task == from.to_string())
        .ok_or_else(|| invalid("coordinator input ref missing"))?
        .basis
        .retained_ref
        .clone();
    let coordinator = basis::read(&p, &from, Some(&reference))?;
    let terminal = phases(&coordinator.task, &note.key)?
        .into_iter()
        .find(|(_, n)| n.phase == "reconciled");
    let Some((_, terminal)) = terminal else {
        return Ok(false);
    };
    let mut metrics = verify::Metrics::default();
    let proofs = ready_proofs(&p, &coordinator, &terminal, &mut metrics)?;
    if note.phase == "reconciled" {
        return Ok(*note == terminal);
    }
    let mut own = None;
    for proof in &proofs {
        if super::lineage::same(project, &proof.task.id.to_string(), &task.id.to_string())? {
            own = Some(proof);
            break;
        }
    }
    let Some(own) = own else {
        return Err(invalid("participant absent from reconciliation proofs"));
    };
    Ok(phases(&own.task, &note.key)?
        .into_iter()
        .last()
        .is_some_and(|(_, n)| n == *note))
}
fn directive(
    ctx: &mut OperationCtx,
    from: &TaskId,
    request: &ReconcileRequest,
    provided: &[u8],
) -> Result<(ArtifactLocator, NewArtifact)> {
    let project = ctx.project(from).clone();
    let name = format!(
        "retained-op-{}-reconcile-{}.request.json",
        ctx.key.operation_id, request.reconciliation_id
    );
    let first = phases(&ctx.tasks[from].task, &ctx.key)?
        .into_iter()
        .find(|(_, n)| {
            matches!(
                n.phase.as_str(),
                "reconcile_intent" | "reconcile_ready" | "reconciled"
            )
        });
    let mut bytes = provided.to_vec();
    let mut locator = None;
    if let Some((index, note)) = first {
        let (loc, id) = directive_info(&note).ok_or_else(|| invalid("directive phase shape"))?;
        if id != request.reconciliation_id {
            return Err(invalid(
                "another reconciliation directive is already bound to this operation",
            ));
        }
        let exact = basis::read_exact(ctx.project(from), from, &ctx.tasks[from].basis)?;
        ctx.metrics.history_bytes_read += exact.history_bytes_read;
        if exact.task.notes.get(index) == ctx.tasks[from].task.notes.get(index) {
            let loc = loc.pinned(&from.to_string(), &exact.basis)?;
            bytes = verify::artifact(&project, &loc, &mut ctx.metrics)?;
        } else {
            bytes = verify::read_local(
                &crate::attachments::task_dir(ctx.project(from), from).join(&name),
                ctx.project(from).attachments_max_bytes,
            )?;
            if verify::version_key(loc)?.1 != verify::hash_bytes(&bytes, &mut ctx.metrics) {
                return Err(invalid("staged directive digest mismatch"));
            }
        }
        locator = Some(loc.clone());
    } else {
        for input in &request.inputs {
            let id = TaskId::parse(&input.task)?;
            if input.basis != ctx.tasks[&id].basis {
                return Err(Error::RetainedStaleBasis(
                    "A new reconciliation requires fresh current committed participant bases"
                        .into(),
                ));
            }
        }
        let path = crate::attachments::task_dir(ctx.project(from), from).join(&name);
        if crate::attachments::entry_state(&path)? != crate::attachments::EntryState::Absent {
            bytes = verify::read_local(&path, ctx.project(from).attachments_max_bytes)?;
        }
    }
    let saved: ReconcileRequest = decode_request(&bytes)?;
    saved.validate()?;
    if saved != *request {
        return Err(invalid(
            "same reconciliation id has different parsed directive bytes",
        ));
    }
    let loc = locator.unwrap_or_else(|| ArtifactLocator::Attachment {
        task: from.to_string(),
        name: name.clone(),
        basis: None,
        sha256: verify::hash_bytes(&bytes, &mut ctx.metrics),
    });
    Ok((loc, NewArtifact { name, bytes }))
}
fn scoped_directive(
    ctx: &OperationCtx,
    from: &TaskId,
    id: &TaskId,
    loc: &ArtifactLocator,
) -> Result<ArtifactLocator> {
    if id == from {
        Ok(loc.clone())
    } else {
        loc.pinned(&from.to_string(), &ctx.tasks[from].basis)
    }
}
fn intent(
    ctx: &mut OperationCtx,
    from: &TaskId,
    id: &TaskId,
    request: &ReconcileRequest,
    binding: &Binding,
    loc: &ArtifactLocator,
    artifact: &NewArtifact,
) -> Result<()> {
    if ctx
        .phase(id)?
        .is_some_and(|(_, n)| directive_info(&n).is_some())
    {
        return Ok(());
    }
    let at = crate::time::after(&ctx.tasks[id].task.updated)?;
    let directive = scoped_directive(ctx, from, id, loc)?;
    let note = publish::receipt(
        ctx,
        binding,
        id,
        PhaseOutputs::ReconcileIntent(ReconcileIntentOutput {
            directive,
            reconciliation_id: request.reconciliation_id.clone(),
        }),
        &at,
    )?;
    publish::batch(
        ctx,
        id,
        if id == from {
            std::slice::from_ref(artifact)
        } else {
            &[]
        },
        &[note],
    )?;
    publish::stop("reconcile_intent");
    Ok(())
}
fn candidates(
    ctx: &OperationCtx,
    from: &TaskId,
    binding: &Binding,
) -> Result<BTreeMap<TaskId, BTreeMap<Identity, Option<Declaration>>>> {
    let mut all = ctx
        .tasks
        .keys()
        .cloned()
        .map(|id| (id, BTreeMap::new()))
        .collect::<BTreeMap<_, _>>();
    if binding.inputs.len() == 1 {
        let request: CheckpointRequest = decode_request(&binding.bytes)?;
        let pinned = &ctx.tasks[from];
        let state = state::replay(
            ctx.project(from),
            &pinned.task,
            &pinned.basis,
            Some(&binding.host),
        )?;
        for item in state.items {
            if item.record.as_ref().is_none_or(|r| r.state == "kept") {
                all.get_mut(from).unwrap().insert(item.identity, None);
            }
        }
        for d in request
            .declarations
            .iter()
            .chain(request.resolutions.iter().flat_map(|r| &r.declarations))
        {
            let identity = d.record(&binding.host, &from.to_string())?.identity();
            all.get_mut(from)
                .unwrap()
                .entry(identity)
                .or_insert_with(|| Some(d.clone()));
        }
    } else {
        let request: TransferRequest = decode_request(&binding.bytes)?;
        for identity in request.items {
            all.get_mut(from).unwrap().insert(identity, None);
        }
        for (id, pinned) in &ctx.tasks {
            if id != from {
                for (_, n) in phases(&pinned.task, &ctx.key)? {
                    if let PhaseOutputs::Receiver(out) = n.outputs {
                        for mapping in out.mapping {
                            all.get_mut(id).unwrap().insert(mapping.receiver, None);
                        }
                    }
                }
            }
        }
    }
    Ok(all)
}
fn ready(
    ctx: &mut OperationCtx,
    from: &TaskId,
    id: &TaskId,
    request: &ReconcileRequest,
    binding: &Binding,
    loc: &ArtifactLocator,
    affected: &BTreeMap<Identity, Option<Declaration>>,
) -> Result<Vec<String>> {
    if let Some((_, note)) = ctx.phase(id)?
        && let PhaseOutputs::Ready(output) = note.outputs
    {
        return Ok(output.unresolved);
    }
    let project = ctx.project(id).clone();
    let pinned = &ctx.tasks[id];
    let before = state::replay(&project, &pinned.task, &pinned.basis, Some(&binding.host))?;
    let mut unresolved = before
        .diagnostics
        .iter()
        .filter(|d| d.resolved_by.is_none())
        .map(|d| format!("{}: {}; {}", id, d.detail, d.action))
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut artifacts = BTreeMap::new();
    for (identity, declaration) in affected {
        let blocked = before.diagnostics.iter().any(|d| {
            d.resolved_by.is_none()
                && d.identity.as_ref().map_or_else(
                    || d.host.as_ref().is_none_or(|h| *h == identity.host),
                    |i| i == identity,
                )
        });
        if blocked {
            continue;
        }
        let existing = before
            .items
            .iter()
            .find(|i| i.identity == *identity)
            .and_then(|i| i.record.clone());
        let record = if let Some(record) = &existing {
            if record.state != "kept" {
                return Err(Error::RetainedHistoryBlocked(format!(
                    "{id}: affected identity is already released outside the original operation; recover its exact custody history before reconciliation"
                )));
            }
            record.clone()
        } else if let Some(d) = declaration {
            d.record(&binding.host, &id.to_string())?
        } else {
            unresolved.push(format!(
                "{id}: selected identity is missing; recover its exact custody history"
            ));
            continue;
        };
        let mut observation = match verify::inspect_history(
            &project,
            &ctx.tasks[id],
            &record,
            None,
            &mut ctx.metrics,
        ) {
            Ok(o) => o,
            Err(e @ Error::RetainedOwnership(_)) => return Err(e),
            Err(e) => verify::unknown(record.clone(), e),
        };
        if let Some(manifest) = observation.manifest.take() {
            let name = format!("retained-manifest-{}.json", manifest.sha256);
            let target = crate::attachments::task_dir(&project, id).join(&name);
            let root = verify::current_path(&project, &record.path)?;
            if root
                .canonicalize()
                .is_ok_and(|root| target.starts_with(root))
            {
                observation = verify::unknown(
                    record.clone(),
                    "an off-container manifest destination is required; preserve raw evidence and repair by a fresh checkpoint",
                );
            } else {
                observation.record.manifest = FieldString::Text(
                    target
                        .to_str()
                        .ok_or_else(|| invalid("manifest path UTF-8"))?
                        .into(),
                );
                if existing.as_ref() != Some(&observation.record)
                    || crate::attachments::ledger(&ctx.tasks[id].task).get(&name)
                        != Some(&crate::attachments::Ledger::Attached)
                {
                    artifacts.insert(
                        name.clone(),
                        NewArtifact {
                            name,
                            bytes: manifest.bytes,
                        },
                    );
                }
            }
        }
        if let Some(action) = &observation.record.verification_action {
            unresolved.push(format!("{id}: {action}"));
        }
        if existing.as_ref() != Some(&observation.record) {
            records.push(observation.record);
        }
    }
    let artifacts = artifacts.into_values().collect::<Vec<_>>();
    let ledger = crate::attachments::ledger(&ctx.tasks[id].task);
    let added = artifacts
        .iter()
        .filter(|a| ledger.get(&a.name) != Some(&crate::attachments::Ledger::Attached))
        .count();
    let first = ctx.tasks[id].task.notes.len() + added + 1;
    let at = crate::time::after(&ctx.tasks[id].task.updated)?;
    let mut notes = records
        .iter()
        .map(|r| {
            r.validate()?;
            Ok(GeneratedNote {
                at: at.clone(),
                text: format!("retained: {}", serde_json::to_string(r).map_err(invalid)?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let directive = scoped_directive(ctx, from, id, loc)?;
    notes.push(publish::receipt(
        ctx,
        binding,
        id,
        PhaseOutputs::Ready(ReadyOutput {
            directive,
            reconciliation_id: request.reconciliation_id.clone(),
            items: (first..first + records.len()).map(|n| n as u64).collect(),
            unresolved: unresolved.clone(),
        }),
        &at,
    )?);
    publish::batch(ctx, id, &artifacts, &notes)?;
    publish::stop("reconcile_ready");
    Ok(unresolved)
}
pub(super) fn result(
    ctx: &mut OperationCtx,
    from: &TaskId,
    terminal: &OperationNote,
    replay: bool,
) -> Result<MutationResult> {
    let mut metrics = verify::Metrics::default();
    let proofs = ready_proofs(ctx.project(from), &ctx.tasks[from], terminal, &mut metrics)?;
    ctx.metrics.bytes_hashed += metrics.bytes_hashed;
    ctx.metrics.hash_ns += metrics.hash_ns;
    ctx.metrics.history_bytes_read += metrics.history_bytes_read;
    let mut counts = Counts {
        kept_verified: 0,
        kept_unknown: 0,
        released: 0,
    };
    for pinned in proofs {
        for item in state::replay(
            &super::proof::project_for(ctx.project(from), &pinned.task.id.to_string())?,
            &pinned.task,
            &pinned.basis,
            Some(&terminal.host),
        )?
        .items
        {
            if let Some(r) = item.record {
                if r.state == "released" {
                    counts.released += 1;
                } else if r.verification == "verified" {
                    counts.kept_verified += 1;
                } else {
                    counts.kept_unknown += 1;
                }
            }
        }
    }
    let PhaseOutputs::Reconciled(out) = &terminal.outputs else {
        return Err(invalid("reconciliation result phase"));
    };
    publish::result(
        ctx,
        "reconciled".into(),
        counts,
        out.unresolved.clone(),
        replay,
    )
}
pub fn run(
    mut ctx: OperationCtx,
    request: ReconcileRequest,
    provided: &[u8],
) -> Result<MutationResult> {
    request.validate()?;
    let from = TaskId::parse(&request.operation.coordinator)?;
    publish::validate_bases(&ctx, &request.inputs)?;
    let binding = load_original(&mut ctx, &from)?;
    for input in &binding.inputs {
        let id = TaskId::parse(&input.task)?;
        if ctx.tasks[&id].basis.retained_ref != input.basis.retained_ref {
            return Err(Error::RetainedStaleBasis("Reconcile from the original input's named retained branch; recover that exact publication context first".into()));
        }
    }
    let exact = basis::read_exact(ctx.project(&from), &from, &ctx.tasks[&from].basis)?;
    ctx.metrics.history_bytes_read += exact.history_bytes_read;
    if phases(&exact.task, &ctx.key)?
        .iter()
        .any(|(_, n)| n.phase == "sender_released")
    {
        return Err(invalid(
            "Sender release is committed; finish or replay the original operation, reconciliation cannot revoke it",
        ));
    }
    let (loc, artifact) = directive(&mut ctx, &from, &request, provided)?;
    if let Some((_, terminal)) = phases(&exact.task, &ctx.key)?
        .into_iter()
        .find(|(_, n)| n.phase == "reconciled")
    {
        return result(&mut ctx, &from, &terminal, true);
    }
    if ctx
        .phase(&from)?
        .is_some_and(|(_, n)| n.phase == "completed")
    {
        return Err(invalid(
            "original operation already completed; reconciliation requires a pending operation",
        ));
    }
    publish::validate_since(&mut ctx, &request.inputs, &binding.inputs, true)?;
    // A staged terminal is adopted only after its complete ready proof has been verified.
    if let Some((_, n)) = ctx.phase(&from)?
        && n.phase == "reconciled"
    {
        let mut metrics = verify::Metrics::default();
        ready_proofs(ctx.project(&from), &ctx.tasks[&from], &n, &mut metrics)?;
        ctx.metrics.bytes_hashed += metrics.bytes_hashed;
        ctx.metrics.hash_ns += metrics.hash_ns;
        ctx.metrics.history_bytes_read += metrics.history_bytes_read;
    }
    for id in ctx.tasks.keys().cloned().collect::<Vec<_>>() {
        publish::recover_saved(&mut ctx, &id, &binding)?;
    }
    if let Some((_, terminal)) = ctx.phase(&from)?
        && terminal.phase == "reconciled"
    {
        return result(&mut ctx, &from, &terminal, true);
    }
    intent(&mut ctx, &from, &from, &request, &binding, &loc, &artifact)?;
    for id in ctx
        .tasks
        .keys()
        .filter(|id| **id != from)
        .cloned()
        .collect::<Vec<_>>()
    {
        intent(&mut ctx, &from, &id, &request, &binding, &loc, &artifact)?;
    }
    let affected = candidates(&ctx, &from, &binding)?;
    let mut unresolved = BTreeSet::new();
    for (id, items) in affected {
        unresolved.extend(ready(
            &mut ctx, &from, &id, &request, &binding, &loc, &items,
        )?);
    }
    let mut publications = Vec::new();
    for id in ctx.tasks.keys().cloned().collect::<Vec<_>>() {
        let p = ctx
            .phase_publications(&id)?
            .into_iter()
            .rev()
            .find(|p| p.phase == "reconcile_ready")
            .ok_or_else(|| invalid("participant ready publication missing"))?;
        publications.push(p);
    }
    let at = crate::time::after(&ctx.tasks[&from].task.updated)?;
    let note = publish::receipt(
        &mut ctx,
        &binding,
        &from,
        PhaseOutputs::Reconciled(ReconciledOutput {
            directive: loc,
            reconciliation_id: request.reconciliation_id,
            ready_bases: publications,
            unresolved: unresolved.into_iter().collect(),
        }),
        &at,
    )?;
    let terminal = decode_operation(note.text.strip_prefix("retention operation: ").unwrap())?;
    let mut metrics = verify::Metrics::default();
    ready_proofs(
        ctx.project(&from),
        &ctx.tasks[&from],
        &terminal,
        &mut metrics,
    )?;
    ctx.metrics.bytes_hashed += metrics.bytes_hashed;
    ctx.metrics.hash_ns += metrics.hash_ns;
    ctx.metrics.history_bytes_read += metrics.history_bytes_read;
    ctx.revalidate()?;
    publish::batch(&mut ctx, &from, &[], &[note])?;
    publish::stop("reconciled");
    result(&mut ctx, &from, &terminal, false)
}
