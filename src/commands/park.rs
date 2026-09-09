use super::{ClaimIntent, Ctx, append_note, id_out, load, owner_name, save};
use crate::claims::{Liveness, Park, WaitingOn};
use crate::error::{Error, Result};
use crate::output::Output;

/// Set a task down (spec §3). Validate everything, refuse a foreign live claim, then let
/// `save` write the note and the entry in that order.
pub fn run(mut ctx: Ctx, id: String, next_step: String, waiting_on: String) -> Result<Output> {
    let mut task = load(&ctx, &id)?;
    if !task.status.is_open() {
        return Err(Error::InvalidTransition(
            task.status.as_str().into(),
            "parked".into(),
        ));
    }
    crate::format::validate_line("next_step", &next_step)?;
    let waiting_on = WaitingOn::parse(&waiting_on)?;
    let owner = owner_name(&ctx.project)?;
    let me = crate::claims::identity()?;

    // The claim rules of §3.1, as a guard: a live foreign claim refuses (no --force), a
    // stale one is taken over with the warning `start` gives, our own is simply replaced.
    let takeover = {
        let store = ctx.claims_mut()?;
        match store.get(&task.id) {
            Some(existing) if existing.session != me.session => {
                let live = crate::claims::liveness(existing);
                let description = Ctx::describe_claim(existing, &live);
                match live {
                    Liveness::Live => {
                        return Err(Error::Claimed(task.id.to_string(), description));
                    }
                    Liveness::Stale(_) => Some(format!("took over {description}")),
                }
            }
            _ => None,
        }
    };
    if let Some(warning) = takeover {
        ctx.warnings.push(warning);
    }

    append_note(
        &mut task,
        &owner,
        &format!("parked (waiting on {}): {next_step}", waiting_on.as_str()),
    )?;
    let park = Park {
        owner,
        session: me.tagged,
        host: crate::claims::hostname(),
        worktree: ctx.project.root.display().to_string(),
        at: crate::time::now(),
        next_step,
        waiting_on,
        title: task.title.clone(),
    };
    ctx.pending_claim = Some((task.id.clone(), ClaimIntent::Park(park)));
    save(&mut ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
