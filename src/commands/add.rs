use super::{Ctx, apply_fields, create, id_out};
use crate::cli::FieldArgs;
use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::output::Output;
use crate::repo::Project;

/// A new task in `project` with a fresh id, timestamps of now, and every other field at
/// its default. The single constructor behind `add` and `feedback`, so a file created
/// in another project is shaped exactly as one created locally.
pub fn blank(project: &Project, title: String, status: Status) -> Result<Task> {
    let now = crate::time::now();
    Ok(Task {
        id: project.new_id()?,
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
    })
}

pub fn run(ctx: Ctx, title: String, status: String, fields: FieldArgs) -> Result<Output> {
    let status = Status::parse(&status)?;
    if !matches!(status, Status::Idea | Status::Todo) {
        return Err(Error::Validation(
            "add accepts --status idea or todo".into(),
        ));
    }
    let mut task = blank(&ctx.project, title, status)?;
    apply_fields(&ctx, &mut task, &fields)?;
    create(&ctx.project, &mut task)?;
    Ok(id_out(ctx, &task))
}
