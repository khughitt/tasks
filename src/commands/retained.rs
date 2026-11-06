use super::{Ctx, open_id_ctx, parse_id};
use crate::cli::RetainedAction;
use crate::error::Result;
use crate::output::Output;
use crate::retained::{basis, model::*, state as history};
use std::path::{Path, PathBuf};

pub fn state(ctx: Ctx, id: String, at: Option<String>, host: Option<String>) -> Result<Output> {
    if host.as_deref().is_some_and(|h| !line(h)) {
        return Err(invalid("host must be a nonempty line"));
    }
    let id = parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let pinned = basis::read(&ctx.project, &id, at.as_deref())?;
    crate::retained::proof::validate_history(&ctx.project, &pinned, host.as_deref())?;
    crate::retained::reconcile::validate_terminal_history(&ctx.project, &pinned)?;
    let mut out = history::replay(&ctx.project, &pinned.task, &pinned.basis, host.as_deref())?;
    out.warnings.extend(ctx.warnings);
    out.warnings.extend(pinned.warnings);
    Ok(Output::RetainedState(out))
}
pub fn request_bytes(ctx: &Ctx, path: &Path) -> Result<Vec<u8>> {
    match crate::attachments::entry_state(path)? {
        crate::attachments::EntryState::File(_) => {}
        _ => return Err(invalid("request must be a regular file")),
    }
    crate::retained::verify::read_local(path, ctx.project.attachments_max_bytes)
}
pub fn dispatch(dir: Option<&Path>, action: RetainedAction) -> Result<Output> {
    match action {
        RetainedAction::State { id, at, host } => state(open_id_ctx(dir, &id)?, id, at, host),
        RetainedAction::Checkpoint { id, request } => checkpoint(dir, id, request),
        RetainedAction::Transfer { from, to, request } => transfer(dir, from, to, request),
        RetainedAction::Reconcile {
            coordinator,
            request,
        } => reconcile(dir, coordinator, request),
    }
}
pub fn checkpoint(dir: Option<&Path>, id: String, request: PathBuf) -> Result<Output> {
    let ctx = open_id_ctx(dir, &id)?;
    let id = parse_id(&ctx.registry, &ctx.shorthand, &id)?.to_string();
    let bytes = request_bytes(&ctx, &request)?;
    let r: CheckpointRequest = decode_request(&bytes)?;
    r.validate()?;
    if r.inputs[0].task != id {
        return Err(invalid(
            "checkpoint argument differs from its request input",
        ));
    }
    let key = OperationKey {
        coordinator: id.clone(),
        operation_id: r.operation_id.clone(),
    };
    let operation = crate::retained::publish::open(dir, &[id], &key)?;
    Ok(Output::RetainedMutation(crate::retained::checkpoint::run(
        operation, r, &bytes,
    )?))
}
pub fn transfer(dir: Option<&Path>, from: String, to: String, request: PathBuf) -> Result<Output> {
    let ctx = open_id_ctx(dir, &from)?;
    let from = parse_id(&ctx.registry, &ctx.shorthand, &from)?.to_string();
    let target = open_id_ctx(dir, &to)?;
    let to = parse_id(&target.registry, &target.shorthand, &to)?.to_string();
    let bytes = request_bytes(&ctx, &request)?;
    let r: TransferRequest = decode_request(&bytes)?;
    r.validate()?;
    if r.inputs[0].task != from || r.inputs[1].task != to {
        return Err(invalid("transfer arguments differ from request inputs"));
    }
    let key = OperationKey {
        coordinator: from.clone(),
        operation_id: r.operation_id.clone(),
    };
    let operation = crate::retained::publish::open(dir, &[from, to], &key)?;
    Ok(Output::RetainedMutation(crate::retained::transfer::run(
        operation, r, &bytes,
    )?))
}
pub fn reconcile(dir: Option<&Path>, coordinator: String, request: PathBuf) -> Result<Output> {
    let ctx = open_id_ctx(dir, &coordinator)?;
    let coordinator = parse_id(&ctx.registry, &ctx.shorthand, &coordinator)?.to_string();
    let bytes = request_bytes(&ctx, &request)?;
    let r: ReconcileRequest = decode_request(&bytes)?;
    r.validate()?;
    if r.operation.coordinator != coordinator {
        return Err(invalid(
            "coordinator argument differs from reconciliation request",
        ));
    }
    let ids = r.inputs.iter().map(|i| i.task.clone()).collect::<Vec<_>>();
    let operation = crate::retained::publish::open(dir, &ids, &r.operation)?;
    Ok(Output::RetainedMutation(crate::retained::reconcile::run(
        operation, r, &bytes,
    )?))
}
