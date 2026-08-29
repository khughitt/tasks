use super::Ctx;
use crate::error::Result;
use crate::output::{GraphOut, Output};
use crate::query::{render_graph, GraphFormat};

pub fn run(ctx: Ctx, format: String, all: bool) -> Result<Output> {
    let format = GraphFormat::parse(&format)?;
    let mut tasks = ctx.project.scan()?;
    if !all {
        tasks.retain(|task| task.status.is_open());
    }
    Ok(Output::Graph(GraphOut {
        format: format.as_str().into(),
        text: render_graph(&tasks, format),
        warnings: ctx.warnings,
    }))
}
