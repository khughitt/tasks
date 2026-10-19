use super::Ctx;
use crate::error::{Error, Result};
use crate::output::{DocOut, Output};
use crate::refs::{Location, Presence};
use crate::resolve::{DocKind, Fetch, Resolver};

pub enum DocTarget {
    Plan,
    Spec,
    Ref(String),
}

pub fn run(mut ctx: Ctx, id: String, target: DocTarget, fetch: Fetch) -> Result<Output> {
    let id = super::parse_id(&ctx.registry, &ctx.shorthand, &id)?;
    let claims = crate::claims::ClaimSnapshot::load(std::iter::once(ctx.project.prefix.as_str()))?;
    let (recorded, task, _) = super::show::read_record(&mut ctx, &id, &claims)?;
    let project = recorded.as_ref().unwrap_or(&ctx.project);
    let mut resolver = Resolver::new(project, &ctx.registry);
    resolver.set_fetch(fetch);
    let body = match target {
        DocTarget::Ref(handle) => {
            let entry = task
                .refs
                .iter()
                .find(|entry| entry.handle == handle)
                .ok_or_else(|| {
                    Error::DocNotFound(format!("{} has no reference {handle}", task.id))
                })?;
            if matches!(crate::refs::location(&entry.reference)?, Location::External) {
                resolver.read_external(&entry.reference)?.required()?.body
            } else {
                let path = match crate::refs::present(&resolver, project, &task, entry)? {
                    Presence::Here(path) => path,
                    Presence::Main { root, path } => {
                        resolver.note_main_entry(&handle, &entry.reference, &root);
                        path
                    }
                    Presence::Absent(_) => {
                        return Err(Error::DocNotFound(format!(
                            "{} reference {handle} {} does not exist yet",
                            task.id, entry.reference
                        )));
                    }
                    Presence::Special(path) => {
                        crate::resolve::refuse_special(&path)?;
                        path
                    }
                    Presence::Unsafe { detail, .. } => return Err(Error::AttachmentUnsafe(detail)),
                };
                if path.is_dir() {
                    return Err(Error::RefNotText(format!(
                        "{} is a directory",
                        path.display()
                    )));
                }
                String::from_utf8(std::fs::read(&path)?).map_err(|_| {
                    Error::RefNotText(format!("{} is not UTF-8 text", path.display()))
                })?
            }
        }
        DocTarget::Plan | DocTarget::Spec => {
            let kind = if matches!(target, DocTarget::Plan) {
                DocKind::Plan
            } else {
                DocKind::Spec
            };
            let value = match kind {
                DocKind::Plan => task.plan.as_deref(),
                DocKind::Spec => task.spec.as_deref(),
            }
            .ok_or_else(|| {
                Error::DocNotFound(format!("{} has no {} link", task.id, kind.name()))
            })?;
            resolver.read(kind, value)?.required()?.body
        }
    };
    ctx.warnings.extend(resolver.take_warnings());
    Ok(Output::Doc(DocOut {
        body,
        warnings: ctx.warnings,
    }))
}
