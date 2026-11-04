use super::ReadCtx;
use crate::error::{Error, Result};
use crate::output::{Output, TreeOut};

/// One forest per project in scope, concatenated in scope order. The forest builder
/// sorts every root globally, so a single run over the union would interleave projects
/// (docs/task-model.md). With an id the scope is a single project -- the one its prefix names,
/// resolved by `open_id_read_ctx`, since `--all-projects` and an id are a clap conflict --
/// so that project's scan is checked for it.
pub fn run(mut ctx: ReadCtx, id: Option<String>, all: bool) -> Result<Output> {
    let root = id
        .as_deref()
        .map(|id| super::parse_id(&ctx.registry, &ctx.shorthand, id))
        .transpose()?;
    let now = crate::time::parse(&crate::time::now())?;
    let mut nodes = Vec::new();
    let scans: Vec<_> = ctx
        .scope
        .scan_each()?
        .into_iter()
        .map(|(_, tasks)| tasks)
        .collect();
    for tasks in scans {
        if let Some(root) = &root
            && !tasks.iter().any(|task| &task.id == root)
        {
            return Err(Error::TaskNotFound(root.to_string()));
        }
        let deps = super::list::review_deps(&mut ctx, &tasks)?;
        nodes.extend(crate::hierarchy::forest(
            &tasks,
            &deps,
            root.as_ref(),
            all,
            crate::hierarchy::Shelved::UnderShownParent,
            None,
            &ctx.registry,
            now,
        ));
    }
    let colors = ctx.project_colors();
    let mut out = Output::Tree(TreeOut {
        nodes,
        warnings: ctx.warnings,
    });
    out.paint_projects(&colors);
    Ok(out)
}
