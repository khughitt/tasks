use super::Ctx;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::output::{Output, TreeOut};

pub fn run(ctx: Ctx, id: Option<String>, all: bool) -> Result<Output> {
    let tasks = ctx.project.scan()?;
    let root = id.as_deref().map(TaskId::parse).transpose()?;
    if let Some(root) = &root
        && !tasks.iter().any(|task| &task.id == root)
    {
        return Err(Error::TaskNotFound(root.to_string()));
    }
    Ok(Output::Tree(TreeOut {
        nodes: crate::hierarchy::forest(&tasks, root.as_ref(), all),
        warnings: ctx.warnings,
    }))
}
