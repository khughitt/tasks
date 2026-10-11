use super::Ctx;
use crate::error::{Error, Result};
use crate::output::{DocOut, Output};
use crate::resolve::{DocKind, Resolver};

pub fn run(mut ctx: Ctx, id: String, kind: DocKind) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let claims = crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (recorded, task, _) = super::show::read_record(&mut ctx, &id, &claims)?;
    let project = recorded.as_ref().unwrap_or(&ctx.project);
    let resolver = Resolver::new(project, &ctx.registry);
    let value = match kind {
        DocKind::Plan => task.plan.as_deref(),
        DocKind::Spec => task.spec.as_deref(),
    }
    .ok_or_else(|| Error::DocNotFound(format!("{} has no {} link", task.id, kind.name())))?;
    let doc = resolver.read(kind, value)?.required()?;
    ctx.warnings.extend(resolver.take_warnings());
    Ok(Output::Doc(DocOut {
        body: doc.body,
        warnings: ctx.warnings,
    }))
}
