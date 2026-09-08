use super::ReadCtx;
use crate::error::{Error, Result};
use crate::output::{Output, TreeOut};

/// One forest per project in scope, concatenated in scope order. The forest builder
/// sorts every root globally, so a single run over the union would interleave projects
/// (spec §3.2). With an id the scope is a single project -- the one its prefix names,
/// resolved by `open_id_read_ctx`, since `--all-projects` and an id are a clap conflict --
/// so that project's scan is checked for it.
pub fn run(ctx: ReadCtx, id: Option<String>, all: bool) -> Result<Output> {
    let root = id
        .as_deref()
        .map(|id| super::parse_id(&ctx.registry, id))
        .transpose()?;
    let mut nodes = Vec::new();
    for (_, tasks) in ctx.scope.scan_each()? {
        if let Some(root) = &root
            && !tasks.iter().any(|task| &task.id == root)
        {
            return Err(Error::TaskNotFound(root.to_string()));
        }
        nodes.extend(crate::hierarchy::forest(
            &tasks,
            root.as_ref(),
            all,
            None,
            &ctx.registry,
        ));
    }
    Ok(Output::Tree(TreeOut {
        nodes,
        warnings: ctx.warnings,
    }))
}
