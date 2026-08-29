use super::{Ctx, append_note, id_out, load, owner_name, save, transition};
use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::Output;

pub fn note(ctx: Ctx, id: String, text: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    let owner = owner_name(&ctx.project)?;
    append_note(&mut task, &owner, &text)?;
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn start(ctx: Ctx, id: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    transition(&ctx, &mut task, Status::Doing, false)?;
    task.owner = Some(owner_name(&ctx.project)?);
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn close(
    ctx: Ctx,
    id: String,
    to: Status,
    message: Option<String>,
    force: bool,
) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    transition(&ctx, &mut task, to, force)?;
    if let Some(message) = message {
        let owner = owner_name(&ctx.project)?;
        append_note(&mut task, &owner, &message)?;
    }
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}

pub fn block(ctx: Ctx, id: String, message: Option<String>) -> Result<Output> {
    close(ctx, id, Status::Blocked, message, false)
}

pub fn unblock(ctx: Ctx, id: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    if task.status != Status::Blocked {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "todo (unblock requires blocked)".into(),
        ));
    }
    transition(&ctx, &mut task, Status::Todo, false)?;
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
