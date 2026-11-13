use super::{Ctx, open_id_ctx, parse_id};
use crate::cli::RetainedAction;
use crate::config::HostConfig;
use crate::error::Result;
use crate::output::Output;
use crate::retained::{basis, model::*, state as history};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(test)]
thread_local! {
    pub(crate) static BEFORE_STATE_RETURN: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

pub fn state(ctx: Ctx, id: String, at: Option<String>, host: Option<String>) -> Result<Output> {
    if host.as_deref().is_some_and(|h| !line(h)) {
        return Err(invalid("host must be a nonempty line"));
    }
    let id = parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let pinned = basis::read(&ctx.project, &id, at.as_deref())?;
    crate::retained::proof::validate_history(&ctx.project, &pinned, host.as_deref())?;
    crate::retained::reconcile::validate_terminal_history(&ctx.project, &pinned)?;
    let mut out = history::replay(&ctx.project, &pinned.task, &pinned.basis, host.as_deref())?;
    #[cfg(test)]
    BEFORE_STATE_RETURN.with(|hook| {
        if let Some(action) = hook.borrow_mut().take() {
            action();
        }
    });
    if at.is_none() {
        crate::retained::freshness::current(&ctx.project, &id, &pinned, false)?;
    }
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
pub fn dispatch(
    dir: Option<&Path>,
    action: RetainedAction,
    config: Arc<HostConfig>,
) -> Result<Output> {
    match action {
        RetainedAction::State { id, at, host } => {
            state(open_id_ctx(dir, &id, config.clone())?, id, at, host)
        }
        RetainedAction::Checkpoint { id, request } => checkpoint(dir, id, request, config.clone()),
        RetainedAction::Transfer { from, to, request } => {
            transfer(dir, from, to, request, config.clone())
        }
        RetainedAction::Reconcile {
            coordinator,
            request,
        } => reconcile(dir, coordinator, request, config.clone()),
    }
}
pub fn checkpoint(
    dir: Option<&Path>,
    id: String,
    request: PathBuf,
    config: Arc<HostConfig>,
) -> Result<Output> {
    let ctx = open_id_ctx(dir, &id, config.clone())?;
    let id = parse_id(&ctx.registry, &ctx.shorthand, &id)?.to_string();
    let bytes = request_bytes(&ctx, &request)?;
    let r: CheckpointRequest = decode_request(&bytes)?;
    r.validate()?;
    if r.inputs[0].task != id
        && crate::retained::lineage::same(&ctx.project, &r.inputs[0].task, &id)?
    {
        let key = OperationKey {
            coordinator: r.inputs[0].task.clone(),
            operation_id: r.operation_id.clone(),
        };
        let operation = crate::retained::publish::open(dir, &[id], &key, config.clone())?;
        return Ok(Output::RetainedMutation(crate::retained::publish::replay(
            operation, &bytes, false,
        )?));
    }
    if r.inputs[0].task != id {
        return Err(invalid(
            "checkpoint argument differs from its request input",
        ));
    }
    let key = OperationKey {
        coordinator: id.clone(),
        operation_id: r.operation_id.clone(),
    };
    let operation = crate::retained::publish::open(dir, &[id], &key, config.clone())?;
    Ok(Output::RetainedMutation(crate::retained::checkpoint::run(
        operation, r, &bytes,
    )?))
}
pub fn transfer(
    dir: Option<&Path>,
    from: String,
    to: String,
    request: PathBuf,
    config: Arc<HostConfig>,
) -> Result<Output> {
    let ctx = open_id_ctx(dir, &from, config.clone())?;
    let from = parse_id(&ctx.registry, &ctx.shorthand, &from)?.to_string();
    let target = open_id_ctx(dir, &to, config.clone())?;
    let to = parse_id(&target.registry, &target.shorthand, &to)?.to_string();
    let bytes = request_bytes(&ctx, &request)?;
    let r: TransferRequest = decode_request(&bytes)?;
    r.validate()?;
    if (r.inputs[0].task != from || r.inputs[1].task != to)
        && crate::retained::lineage::same(&ctx.project, &r.inputs[0].task, &from)?
        && crate::retained::lineage::same(&target.project, &r.inputs[1].task, &to)?
    {
        let key = OperationKey {
            coordinator: r.inputs[0].task.clone(),
            operation_id: r.operation_id.clone(),
        };
        let operation = crate::retained::publish::open(dir, &[from, to], &key, config.clone())?;
        return Ok(Output::RetainedMutation(crate::retained::publish::replay(
            operation, &bytes, false,
        )?));
    }
    if r.inputs[0].task != from || r.inputs[1].task != to {
        return Err(invalid("transfer arguments differ from request inputs"));
    }
    let key = OperationKey {
        coordinator: from.clone(),
        operation_id: r.operation_id.clone(),
    };
    let operation = crate::retained::publish::open(dir, &[from, to], &key, config.clone())?;
    Ok(Output::RetainedMutation(crate::retained::transfer::run(
        operation, r, &bytes,
    )?))
}
pub fn reconcile(
    dir: Option<&Path>,
    coordinator: String,
    request: PathBuf,
    config: Arc<HostConfig>,
) -> Result<Output> {
    let ctx = open_id_ctx(dir, &coordinator, config.clone())?;
    let coordinator = parse_id(&ctx.registry, &ctx.shorthand, &coordinator)?.to_string();
    let bytes = request_bytes(&ctx, &request)?;
    let r: ReconcileRequest = decode_request(&bytes)?;
    r.validate()?;
    if !crate::retained::lineage::same(&ctx.project, &r.operation.coordinator, &coordinator)? {
        return Err(invalid(
            "coordinator argument differs from reconciliation request",
        ));
    }
    let mut ids = Vec::new();
    for input in &r.inputs {
        let p = crate::retained::proof::project_for(&ctx.project, &input.task)?;
        let old = crate::model::TaskId::parse(&input.task)?;
        let current = format!("{}-{}", p.prefix, old.hex);
        if !crate::retained::lineage::same(&p, &input.task, &current)? {
            return Err(invalid("unproved historical participant"));
        }
        ids.push(current);
    }
    if r.inputs
        .iter()
        .zip(&ids)
        .any(|(input, id)| input.task != *id)
    {
        let operation = crate::retained::publish::open(dir, &ids, &r.operation, config.clone())?;
        return Ok(Output::RetainedMutation(crate::retained::publish::replay(
            operation, &bytes, true,
        )?));
    }
    let operation = crate::retained::publish::open(dir, &ids, &r.operation, config.clone())?;
    Ok(Output::RetainedMutation(crate::retained::reconcile::run(
        operation, r, &bytes,
    )?))
}
