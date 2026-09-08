use super::{Ctx, apply_fields, create};
use crate::cli::FieldArgs;
use crate::error::{Error, Result};
use crate::format::validate_line;
use crate::model::{Status, Task};
use crate::output::{AddOut, Output};
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
        parallel: false,
        owner: None,
        created: now.clone(),
        updated: now,
        depends: vec![],
        parent: None,
        tags: vec![],
        source: None,
        spec: None,
        plan: None,
        step: None,
        body: String::new(),
        notes: vec![],
    })
}

/// Tasks in `project` already carrying exactly this `source` and `title`, lowest id
/// first. Every status counts: refiling something already done or dropped is precisely
/// the duplicate this exists to catch. Title equality is exact, because the key has to be
/// something a caller can reproduce byte for byte, not something that merely looks alike.
fn duplicates_of(project: &Project, source: &str, title: &str) -> Result<Vec<Task>> {
    let mut found: Vec<Task> = project
        .scan()?
        .into_iter()
        .filter(|task| task.source.as_deref() == Some(source) && task.title == title)
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}

pub fn run(mut ctx: Ctx, title: String, status: String, fields: FieldArgs) -> Result<Output> {
    let status = Status::parse(&status)?;
    if !matches!(status, Status::Idea | Status::Todo) {
        return Err(Error::Validation(
            "add accepts --status idea or todo".into(),
        ));
    }
    // The context's lock keeps the duplicate check and create in one critical section.
    if let Some(source) = &fields.source {
        validate_line("source", source)?;
        let existing = duplicates_of(&ctx.project, source, &title)?;
        if let Some(task) = existing.first() {
            ctx.warnings.push(format!(
                "{} already carries this source and title; reused it, wrote nothing",
                task.id
            ));
            if existing.len() > 1 {
                let rest: Vec<String> = existing[1..]
                    .iter()
                    .map(|task| task.id.to_string())
                    .collect();
                ctx.warnings.push(format!(
                    "{} other task(s) carry the same source and title: {}",
                    rest.len(),
                    rest.join(", ")
                ));
            }
            return Ok(Output::Add(AddOut {
                id: task.id.to_string(),
                action: "reused".into(),
                warnings: ctx.warnings,
            }));
        }
    }
    let mut task = blank(&ctx.project, title, status)?;
    apply_fields(&ctx, &mut task, &fields)?;
    create(&ctx.project, &ctx.registry, &mut task)?;
    Ok(Output::Add(AddOut {
        id: task.id.to_string(),
        action: "created".into(),
        warnings: ctx.warnings,
    }))
}
