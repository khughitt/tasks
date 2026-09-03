use super::{Ctx, apply_fields, id_out, save};
use crate::cli::FieldArgs;
use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::output::Output;

pub fn run(ctx: Ctx, title: String, status: String, fields: FieldArgs) -> Result<Output> {
    let status = Status::parse(&status)?;
    if !matches!(status, Status::Idea | Status::Todo) {
        return Err(Error::Validation(
            "add accepts --status idea or todo".into(),
        ));
    }
    let now = crate::time::now();
    let mut task = Task {
        id: ctx.project.new_id()?,
        title,
        status,
        priority: 2,
        size: None,
        owner: None,
        created: now.clone(),
        updated: now,
        depends: vec![],
        parent: None,
        tags: vec![],
        spec: None,
        plan: None,
        step: None,
        body: String::new(),
        notes: vec![],
    };
    apply_fields(&ctx, &mut task, &fields)?;
    save(&ctx, &mut task)?;
    Ok(id_out(ctx, &task))
}
