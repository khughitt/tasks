//! Missing-record inspection hints; discovered files never become automatic read authority.
use super::{Ctx, Ownership, Writer};
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::recovery::{Budget, Candidate, Scan, SearchUnavailable};
use crate::repo::{Kind, Project};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn inspection_error(id: &TaskId, scan: &Scan) -> Error {
    let mut parts = Vec::new();
    match scan.unavailable {
        Some(SearchUnavailable::Detached) => parts
            .push("detached projects use one shared records home; no worktree search".to_string()),
        Some(SearchUnavailable::NoGit) => parts.push(
            "worktree search unavailable: records outside Git or Git unavailable".to_string(),
        ),
        None if !scan.complete() => parts.push("worktree search incomplete".into()),
        None if scan.matches.is_empty() => {
            parts.push("no matching record file found in inspected eligible worktrees".into())
        }
        None if scan.matches.len() > 1 => parts.push("record-file location is ambiguous".into()),
        None => {}
    }
    for gap in scan.gaps.iter().take(8) {
        let root = gap
            .root
            .as_ref()
            .map(|p| format!("{}: ", p.display()))
            .unwrap_or_default();
        parts.push(format!("{root}{}", gap.detail));
    }
    if scan.gaps.len() > 8 {
        parts.push(format!(
            "{} further unresolved locations omitted",
            scan.gaps.len() - 8
        ));
    }
    for found in scan.matches.iter().take(8) {
        parts.push(format!("record file observed in {}", found.root.display()));
        parts.push(crate::stale::rerun_line(
            &found.root,
            &["show".into(), id.to_string()],
        ));
    }
    if scan.matches.len() > 8 {
        parts.push(format!(
            "{} further matches omitted",
            scan.matches.len() - 8
        ));
    }
    if !scan.matches.is_empty() {
        parts.push("location hints only; explicit retry revalidates the record".into());
    }
    Error::TaskNotFoundNext {
        id: id.to_string(),
        next: parts.join("; "),
    }
}

pub(super) fn write_error(
    ctx: &mut Ctx,
    id: &TaskId,
    writer: Writer<'_>,
    mut scan: Scan,
    snapshot: &crate::claims::ClaimSnapshot,
) -> Error {
    let inspection = inspection_error(id, &scan);
    if !scan.complete() || scan.matches.len() != 1 {
        return inspection;
    }
    let Error::TaskNotFoundNext { next: inspect, .. } = inspection else {
        unreachable!()
    };
    let refusal = |next| Error::TaskNotFoundNext {
        id: id.to_string(),
        next,
    };
    if ctx
        .shorthand
        .local_prefix()
        .map(|p| ctx.registry.canonical_prefix(p))
        != Some(ctx.registry.canonical_prefix(&ctx.project.prefix))
    {
        return refusal(format!(
            "{inspect}; original shorthand project differs or is unavailable; inspect first and use full task IDs"
        ));
    }
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect()
    {
        Ok(args) => args,
        Err(_) => {
            return refusal(format!(
                "{inspect}; non-UTF-8 arguments require manual inspection to preserve their meaning"
            ));
        }
    };
    let candidate = &scan.matches[0];
    let occupants = occupancy(ctx, candidate, &mut scan.budget, snapshot);
    let work = occupants
        .work
        .iter()
        .take(8)
        .map(|(task, session, root)| {
            format!("{task} (session {session}, resume {})", root.display())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let work = if occupants.work.len() > 8 {
        format!(
            "{work}, {} further occupants omitted",
            occupants.work.len() - 8
        )
    } else {
        work
    };
    let location = candidate.root.display();
    if !occupants.unknown.is_empty() {
        let mut reasons = occupants
            .unknown
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        if occupants.unknown.len() > 8 {
            reasons.push_str(&format!(
                "; {} further unresolved occupancy location omitted",
                occupants.unknown.len() - 8
            ));
        }
        let known = if work.is_empty() {
            String::new()
        } else {
            format!("; known occupants: {work}")
        };
        return refusal(format!(
            "{inspect}; occupancy unknown: {reasons}{known}; no write retry offered"
        ));
    }
    if !occupants.work.is_empty() {
        let head = format!(
            "records in {location} are used by another session: {work}; coordinate record integration with its holder; nothing was written"
        );
        return if writer == Writer::Note {
            refusal(format!(
                "{head}. Or, to note that copy in place: {}{}",
                crate::stale::rerun_line(&candidate.root, &args),
                crate::stale::stdin_hint(&args)
            ))
        } else {
            refusal(format!("{head}; {inspect}"))
        };
    }
    let handoff = match main_handoff(ctx, candidate, &mut scan.budget) {
        Ok(handoff) => handoff,
        Err(e) => {
            return refusal(format!(
                "{inspect}; main-checkout handoff is unknown: {e}; no write retry offered"
            ));
        }
    };
    let remedy = if handoff {
        crate::stale::Remedy::Handoff(&args)
    } else {
        crate::stale::Remedy::Rerun(&args)
    };
    refusal(format!(
        "record file observed in {location}; nothing was written; explicit retry revalidates the record. {}",
        crate::stale::remedy(id, &candidate.root, remedy, None)
    ))
}

struct Occupancy {
    work: Vec<(String, String, PathBuf)>,
    unknown: Vec<String>,
}

fn occupancy(
    ctx: &mut Ctx,
    candidate: &Candidate,
    budget: &mut Budget,
    snapshot: &crate::claims::ClaimSnapshot,
) -> Occupancy {
    let mut out = Occupancy {
        work: Vec::new(),
        unknown: Vec::new(),
    };
    let me = crate::claims::resolve_identity(&ctx.host, &mut ctx.warnings);
    if let crate::claims::Resolution::Failed(e) = &me {
        out.unknown.push(e.to_string());
    }
    let mut homes: BTreeMap<PathBuf, std::result::Result<PathBuf, String>> = BTreeMap::new();
    let mut layouts = None;
    for (task, (claim, live)) in snapshot.iter() {
        if live.is_stale() || super::ownership(claim, &me, &ctx.host) != Ownership::Foreign {
            continue;
        }
        let root = PathBuf::from(&claim.worktree);
        let occupies = if root == candidate.root {
            true
        } else if candidate.kind != Kind::Linked {
            false
        } else {
            let home = homes.entry(root.clone()).or_insert_with(|| {
                linked_home(ctx, &root, budget, &mut layouts).map_err(|e| e.to_string())
            });
            match home {
                Ok(home) => *home == candidate.root,
                Err(e) => {
                    out.unknown.push(format!("{}: {e}", root.display()));
                    false
                }
            }
        };
        if occupies {
            out.work.push((task.clone(), claim.session.clone(), root));
        }
    }
    out.unknown.sort();
    out.unknown.dedup();
    out
}

fn registered_layouts(ctx: &Ctx, budget: &mut Budget) -> Result<Vec<Project>> {
    let mut layouts = Vec::new();
    for (prefix, registered) in &ctx.registry.projects {
        let dir = crate::scope::config_dir(&ctx.registry, prefix).expect("registered key");
        budget.admit(&dir)?;
        if !crate::scope::is_reachable(&ctx.registry, prefix)? {
            continue;
        }
        let dir = dir.canonicalize()?;
        let project = budget.project(&dir)?;
        if project.prefix != *prefix {
            return Err(Error::Validation(
                "registered configuration prefix mismatch".into(),
            ));
        }
        let paired = ctx.registry.records_dir(prefix).is_some();
        if paired {
            budget.admit(registered)?;
        }
        if paired != (project.kind == Kind::Detached)
            || (paired && crate::rename::root_identity(registered)? != project.checkout)
        {
            return Err(Error::Validation(
                "registered records pairing conflicts with its configuration".into(),
            ));
        }
        crate::scope::check_predecessor_pairing(&ctx.registry, &project)?;
        layouts.push(project);
    }
    Ok(layouts)
}

fn linked_home(
    ctx: &Ctx,
    root: &Path,
    budget: &mut Budget,
    layouts: &mut Option<std::result::Result<Vec<Project>, String>>,
) -> Result<PathBuf> {
    budget.admit(root)?;
    let root = root.canonicalize()?;
    match root.join(crate::repo::CONFIG_REL).symlink_metadata() {
        Ok(_) => {
            let project = budget.project(&root)?;
            if project.prefix != ctx.project.prefix || project.kind != Kind::Linked {
                return Err(Error::Validation(
                    "claim records layout cannot be established".into(),
                ));
            }
            crate::scope::check_predecessor_pairing(&ctx.registry, &project)?;
            return Ok(project.records);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let tree = crate::link::GitTree::of(&root)?
        .ok_or_else(|| Error::Validation("claim location has no resolvable Git layout".into()))?;
    if tree.top != root {
        return Err(Error::Validation(
            "claim location is not a code worktree root".into(),
        ));
    }
    let main = tree
        .main_checkout()
        .ok_or_else(|| Error::Validation("code main checkout cannot be established".into()))?;
    let layouts =
        layouts.get_or_insert_with(|| registered_layouts(ctx, budget).map_err(|e| e.to_string()));
    let layouts = layouts.as_ref().map_err(|e| Error::Validation(e.clone()))?;
    // Match project_of_tree's detached-pairing precedence without unbudgeted config reads.
    for (prefix, registered) in &ctx.registry.projects {
        budget.admit(registered)?;
        if crate::rename::root_identity(registered)? != main {
            continue;
        }
        if ctx.registry.records_dir(prefix).is_some() {
            let paired = layouts
                .iter()
                .find(|p| p.prefix == *prefix)
                .ok_or_else(|| Error::Validation("paired records home is unavailable".into()))?;
            return if paired.prefix == ctx.project.prefix {
                Ok(paired.records.clone())
            } else {
                Err(Error::Validation(
                    "code claim routes to a different paired project".into(),
                ))
            };
        }
        if !layouts.iter().any(|p| p.prefix == *prefix) {
            return Err(Error::Validation(
                "code checkout is registered without reachable records or a pairing".into(),
            ));
        }
        break;
    }
    let linked: Vec<_> = layouts
        .iter()
        .filter(|p| p.kind == Kind::Linked && p.checkout == main)
        .collect();
    match linked.as_slice() {
        [project] if project.prefix == ctx.project.prefix => Ok(project.records.clone()),
        _ => Err(Error::Validation(
            "code claim has missing or conflicting registered overlay bindings".into(),
        )),
    }
}

fn main_handoff(ctx: &Ctx, candidate: &Candidate, budget: &mut Budget) -> Result<bool> {
    let tree = crate::link::GitTree::of(&ctx.project.records)?
        .ok_or_else(|| Error::Validation("records Git layout unavailable".into()))?;
    if tree.is_main() {
        return Ok(false);
    }
    let main = tree
        .main_checkout()
        .ok_or_else(|| Error::Validation("records main checkout unavailable".into()))?;
    let offset = ctx
        .project
        .records
        .strip_prefix(&tree.top)
        .map_err(|e| Error::Validation(e.to_string()))?;
    let main = main.join(offset).canonicalize()?;
    budget.admit(&main)?;
    Ok(main == candidate.root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::{Candidate, Gap};
    #[test]
    fn display_is_capped_without_hiding_ambiguity_or_omitted_locations() {
        let id = TaskId::parse("sci-000001").unwrap();
        let scan = Scan {
            matches: (0..10)
                .map(|n| Candidate {
                    root: format!("/candidate-{n}").into(),
                    kind: crate::repo::Kind::InTree,
                    checkout: format!("/candidate-{n}").into(),
                })
                .collect(),
            gaps: (0..11)
                .map(|n| Gap {
                    root: Some(format!("/gap-{n}").into()),
                    detail: "unreadable".into(),
                })
                .collect(),
            ..Scan::default()
        };
        let text = inspection_error(&id, &scan).to_string();
        assert!(text.contains("incomplete"));
        assert_eq!(text.matches("show sci-000001").count(), 8);
        assert!(text.contains("2 further matches omitted"));
        assert!(text.contains("3 further unresolved locations omitted"));
        assert!(!text.contains("/candidate-8") && !text.contains("/gap-8"));
    }
}
