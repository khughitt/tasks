use super::{Ctx, open_id_ctx, parse_id};
use crate::claims::MutationLock;
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::moving::classify::{self, Recovery};
use crate::moving::journal::Journal;
use crate::moving::{self, MoveCtx};
use crate::output::{MoveOut, Output};
use crate::registry::Registry;
use crate::rename::root_identity;
use crate::scope::{self, Origin};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn open(dir: Option<&Path>, spelling: &str, to: &str) -> Result<MoveCtx> {
    let source = open_id_ctx(dir, spelling)?;
    let source_id = parse_id(&source.registry, &source.shorthand, spelling)?;
    open_source(source, source_id, to)
}

fn open_source(source: Ctx, source_id: TaskId, to: &str) -> Result<MoveCtx> {
    let registry = Registry::load()?;
    let target_prefix = registry.canonical_prefix(to);
    if target_prefix == source_id.prefix {
        return Err(Error::Config(format!(
            "source and destination are both {target_prefix}; choose another project"
        )));
    }
    let root = registry.project_root(target_prefix).ok_or_else(|| {
        Error::Config(format!(
            "no project registered as {to:?}; register it or choose another project"
        ))
    })?;
    if !scope::is_reachable(&registry, target_prefix)? {
        return Err(Error::Config(format!(
            "{target_prefix} at {} is unreachable; restore it or choose another project",
            root.display()
        )));
    }
    let registered = registry
        .project_root(&source_id.prefix)
        .ok_or_else(|| Error::Config(format!("no project registered as {:?}", source_id.prefix)))?;
    if root_identity(&source.project.root)? != root_identity(registered)? {
        return Err(Error::Validation(format!(
            "{source_id} is moved from {}'s registered checkout, which foreign readers and check read; run `tasks -C {} move {source_id} --to {target_prefix}`",
            source_id.prefix,
            crate::stale::quote(&registered.display().to_string())
        )));
    }
    let target = scope::open_registered(&registry, target_prefix, Origin::Prefix)?;
    Ok(MoveCtx {
        source,
        target,
        registry,
        source_id,
    })
}

fn identity(ctx: &MoveCtx) -> Result<(crate::model::TaskId, String, PathBuf, PathBuf)> {
    Ok((
        ctx.source_id.clone(),
        ctx.target.prefix.clone(),
        root_identity(&ctx.source.project.root)?,
        root_identity(&ctx.target.root)?,
    ))
}

pub fn run(dir: Option<&Path>, id: &str, to: &str, explain: bool) -> Result<Output> {
    if explain {
        let mut from = id.to_string();
        let result = (|| {
            let source = open_id_ctx(dir, id)?;
            let source_id = parse_id(&source.registry, &source.shorthand, id)?;
            from = source_id.to_string();
            let mut requested = open_source(source, source_id, to)?;
            decide(&mut requested, true)
        })();
        return Ok(Output::Move(match result {
            Ok(out) => out,
            Err(error) => empty_output(from, "refuse", vec![error.to_string()]),
        }));
    }
    let mut requested = open(dir, id, to)?;
    for _ in 0..4 {
        let mut prefixes = [&requested.source_id.prefix, &requested.target.prefix];
        prefixes.sort();
        let _first = MutationLock::acquire(prefixes[0])?;
        let _second = MutationLock::acquire(prefixes[1])?;
        let _registry = Registry::lock()?;
        let mut refreshed = open(dir, id, to)?;
        if identity(&refreshed)? == identity(&requested)? {
            let mut out = decide(&mut refreshed, false)?;
            crate::record_commit::commit_written(&mut out.warnings);
            return Ok(Output::Move(out));
        }
        requested = refreshed;
    }
    Err(Error::Io(
        "the projects' identities kept changing while acquiring move locks; retry".into(),
    ))
}

fn empty_output(from: String, recovery: &str, warnings: Vec<String>) -> MoveOut {
    MoveOut {
        id: String::new(),
        from,
        recovery: recovery.into(),
        attachments: Vec::new(),
        park: false,
        inbound: Vec::new(),
        paths: BTreeMap::new(),
        warnings,
    }
}

fn decide(ctx: &mut MoveCtx, explain: bool) -> Result<MoveOut> {
    let mut journal = None;
    let result = (|| {
        journal = Journal::load(&ctx.source_id)?;
        if let Some(journal) = &journal {
            resume(ctx, journal, explain)
        } else {
            fresh_or_complete(ctx, explain)
        }
    })();
    match result {
        Err(error) if explain => {
            let mut out = journal
                .as_ref()
                .map(|j| moving::output(ctx, j, "refuse"))
                .unwrap_or_else(|| {
                    empty_output(
                        ctx.source_id.to_string(),
                        "refuse",
                        ctx.source.warnings.clone(),
                    )
                });
            out.warnings.push(error.to_string());
            Ok(out)
        }
        other => other,
    }
}

fn fresh_or_complete(ctx: &mut MoveCtx, explain: bool) -> Result<MoveOut> {
    let journal = match moving::preflight::prepare(ctx) {
        Ok(journal) => Some(journal),
        Err(Error::Moved(_)) => None,
        Err(error) => return Err(error),
    };
    if journal.is_none() {
        let source = ctx.source.project.read_task(&ctx.source_id)?;
        let target = ctx
            .registry
            .canonical_id(source.moved_to.as_ref().expect("validated tombstone"));
        if target.prefix != ctx.target.prefix {
            super::guard_write(&ctx.registry, &ctx.source.shorthand, &source)?;
        }
        moving::reject_operations(ctx, None)?;
        let counterpart = crate::resolve::read_present(&ctx.target, &target)?;
        if !counterpart
            .as_ref()
            .and_then(|task| task.moved_from.as_ref())
            .is_some_and(|from| ctx.registry.canonical_id(from) == ctx.source_id)
        {
            return Err(Error::Validation(format!(
                "move_incomplete: {} and {target} do not point back to each other; run `tasks check` and finish the move on the host that started it",
                ctx.source_id
            )));
        }
        let mut out = empty_output(
            ctx.source_id.to_string(),
            "complete",
            ctx.source.warnings.clone(),
        );
        out.id = target.to_string();
        return Ok(out);
    }
    let journal = journal.expect("a live source prepared its journal");
    if explain {
        let mut out = moving::output(ctx, &journal, "fresh");
        moving::inbound(ctx, &mut out)?;
        return Ok(out);
    }
    moving::run(ctx, journal)
}

fn resume(ctx: &mut MoveCtx, journal: &Journal, explain: bool) -> Result<MoveOut> {
    moving::reject_operations(ctx, Some(journal))?;
    super::reject_retired(&ctx.registry, &ctx.source.project)?;
    super::reject_retired(&ctx.registry, &ctx.target)?;
    match classify::classify(&moving::snapshot(ctx, journal)?) {
        Recovery::Refuse(reason) => Err(Error::MovePending(reason)),
        Recovery::Resume if explain => {
            let mut out = moving::output(ctx, journal, "resume");
            moving::inbound(ctx, &mut out)?;
            Ok(out)
        }
        Recovery::Resume => moving::run(ctx, journal.clone()),
    }
}
