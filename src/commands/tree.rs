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
    let mut captured = Vec::new();
    let mut projects = Vec::new();
    for (_, tasks) in ctx.scope.scan_each()? {
        let start = captured.len();
        captured.extend(tasks);
        projects.push(start..captured.len());
    }
    // Activity sees cross-project edges, while each forest keeps its project ordering.
    let activity = crate::activity::ActivityIndex::of_scan(&captured, &ctx.registry, now);
    for project in projects {
        let tasks = &captured[project];
        if let Some(root) = &root
            && !tasks.iter().any(|task| &task.id == root)
        {
            return Err(Error::TaskNotFound(root.to_string()));
        }
        let deps = super::list::review_deps(&mut ctx, tasks)?;
        nodes.extend(crate::hierarchy::forest(
            tasks,
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
    out.paint_activity(&activity);
    Ok(out)
}
