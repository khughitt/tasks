//! The checks a decision list passes before anything is written (spec §4.3): coverage,
//! header, staleness, claims, other checkouts, reasons, keys and pending operations.
//! Every refusal is reported, not the first; each names the key or command that settles it.

use super::MigrateCtx;
use super::list::{Decision, DecisionList, Entry, LoadedList};
use crate::claims::ClaimStore;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::RefusalOut;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn refuse(id: &str, rule: &str, settle: impl Into<String>) -> RefusalOut {
    RefusalOut {
        id: id.to_string(),
        rule: rule.to_string(),
        settle: settle.into(),
    }
}

/// What an entry's `id` spelling names: a canonical id, or a refusal already recorded.
enum Named {
    Id(String),
    /// A bare suffix of this eligible id: refused, but the id counts as covered so the
    /// refusal is not followed by a second one for the same task.
    Bare(String),
    Refused,
}

fn name_entry(
    ctx: &MigrateCtx,
    raw: &str,
    eligible: &BTreeMap<String, Task>,
    refusals: &mut Vec<RefusalOut>,
) -> Named {
    let source = &ctx.source.project;
    let typed = raw.trim();
    let id = match TaskId::parse(typed) {
        Ok(id) => ctx.registry.canonical_id(&id),
        Err(_) => {
            let is_suffix = typed.len() == 6 && typed.chars().all(|c| c.is_ascii_hexdigit());
            if is_suffix
                && let Some(full) = eligible.keys().find(|k| k.ends_with(&format!("-{typed}")))
            {
                refusals.push(refuse(
                    typed,
                    "foreign_id",
                    format!("write the full id {full}; a bare suffix is not an id"),
                ));
                return Named::Bare(full.clone());
            }
            refusals.push(refuse(
                typed,
                "foreign_id",
                format!(
                    "{typed:?} is not a task id of {}; use `tasks migrate --draft` ids",
                    source.prefix
                ),
            ));
            return Named::Refused;
        }
    };
    let text = id.to_string();
    if id.prefix != source.prefix {
        refusals.push(refuse(
            &text,
            "foreign_id",
            format!(
                "remove the entry: it is not a {} task, and a list covers only the source",
                source.prefix
            ),
        ));
        return Named::Refused;
    }
    if eligible.contains_key(&text) {
        return Named::Id(text);
    }
    if source.task_path(&id).exists() {
        refusals.push(refuse(
            &text,
            "not_open",
            "remove the entry: the task is closed, and only open tasks (and recurrences) are listed",
        ));
    } else {
        refusals.push(refuse(
            &text,
            "foreign_id",
            format!("remove the entry: {} has no task {text}", source.prefix),
        ));
    }
    Named::Refused
}

/// Every refusal the list earns against the source as it stands. Reads only.
pub fn check_list(ctx: &MigrateCtx, list: &LoadedList) -> Result<Vec<RefusalOut>> {
    let source = &ctx.source.project;
    crate::retained::publish::project_permission(source, true)?;
    let mut refusals = Vec::new();

    // Header.
    for (key, written, actual) in [
        ("source", &list.list.source, &source.prefix),
        ("successor", &list.list.successor, &ctx.target.prefix),
    ] {
        let live = ctx.registry.canonical_prefix(written);
        if live != actual {
            refusals.push(refuse(
                "list",
                "header_mismatch",
                format!("the list says {key} = {written:?} but this run's {key} is {actual:?}; fix `{key}` or run in the right checkout"),
            ));
        }
    }

    // Coverage.
    let eligible: BTreeMap<String, Task> = super::draft::eligible(ctx)?
        .into_iter()
        .map(|t| (t.id.to_string(), t))
        .collect();
    let mut first: BTreeMap<String, &Entry> = BTreeMap::new();
    let mut twice = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for entry in &list.list.tasks {
        match name_entry(ctx, &entry.id, &eligible, &mut refusals) {
            Named::Id(id) => {
                covered.insert(id.clone());
                match first.entry(id.clone()) {
                    std::collections::btree_map::Entry::Vacant(slot) => {
                        slot.insert(entry);
                    }
                    std::collections::btree_map::Entry::Occupied(_) => {
                        if twice.insert(id.clone()) {
                            refusals.push(refuse(
                                &id,
                                "listed_twice",
                                "keep one `[[task]]` entry for this id and delete the others",
                            ));
                        }
                    }
                }
            }
            Named::Bare(id) => {
                covered.insert(id);
            }
            Named::Refused => {}
        }
    }
    for id in eligible.keys().filter(|id| !covered.contains(*id)) {
        refusals.push(refuse(
            id,
            "undecided",
            "add a `[[task]]` entry with decision carry, drop or close, or redraft the list",
        ));
    }

    // Per-entry rules.
    for (id, entry) in &first {
        let task = &eligible[id];
        let decision = match Decision::parse(&entry.decision) {
            Some(d) => d,
            None if entry.decision.is_empty() => {
                refusals.push(refuse(
                    id,
                    "undecided",
                    "set `decision` to carry, drop or close",
                ));
                continue;
            }
            None => {
                refusals.push(refuse(
                    id,
                    "bad_key",
                    format!(
                        "`decision` is {:?}; it must be carry, drop or close",
                        entry.decision
                    ),
                ));
                continue;
            }
        };
        let bytes = std::fs::read(source.task_path(&task.id))?;
        let current = format!("sha256:{:x}", Sha256::digest(&bytes));
        if entry.digest != current {
            refusals.push(refuse(
                id,
                "stale_digest",
                format!("the record changed after the list was drafted; re-read it, then set `digest = \"{current}\"` or redraft"),
            ));
        }
        if decision != Decision::Carry
            && entry.reason.as_deref().is_none_or(|r| r.trim().is_empty())
        {
            refusals.push(refuse(
                id,
                "missing_reason",
                "set `reason` to a key of [reasons] or text saying why (for close, where the work was done)",
            ));
        }
        if decision != Decision::Carry
            && let Some(reason) = entry.reason.as_deref()
            && super::project::multi_line(super::project::reason_text(&list.list, reason.trim()))
        {
            let reason = reason.trim();
            let key = if list.list.reasons.contains_key(reason) {
                format!("`[reasons]` key {reason:?}")
            } else {
                "`reason`".to_string()
            };
            refusals.push(refuse(
                id,
                "bad_key",
                format!("{key} holds a line break; a migration note is one line, so write the reason on a single line"),
            ));
        }
        let mut wrong = Vec::new();
        if decision != Decision::Carry {
            for (key, present) in [
                ("parent", entry.parent.is_some()),
                ("rm_depends", !entry.rm_depends.is_empty()),
                ("links", entry.links.is_some()),
                ("body", entry.body.is_some()),
            ] {
                if present {
                    wrong.push(format!(
                        "`{key}` applies only to carry; remove it or carry the task"
                    ));
                }
            }
        } else if !entry.release.is_empty() {
            wrong.push("`release` applies only to drop or close; remove it".to_string());
        }
        for settle in wrong {
            refusals.push(refuse(id, "bad_key", settle));
        }
        if decision == Decision::Close && task.every.is_some() {
            refusals.push(refuse(
                id,
                "close_recurring",
                "a recurrence cannot be closed in a retiring project; set decision to drop (ends the cadence) or carry",
            ));
        }
    }

    // Live claims on any listed task.
    let store = ClaimStore::load(&source.prefix)?;
    for (id, claim) in store.iter() {
        if covered.contains(id) {
            let live = crate::claims::liveness(claim);
            if !live.is_stale() {
                refusals.push(refuse(
                    id,
                    "claimed",
                    format!(
                        "{}; wait for it to finish, or release it with `tasks release {id}`",
                        crate::commands::Ctx::describe_claim(claim, &live)
                    ),
                ));
            }
        }
    }

    // Other checkouts: each open record there blocks `retire`, whatever its bytes.
    let registered = canonical(&source.records)?;
    for (root, task, _) in crate::commands::retire::open_copies(source)? {
        if canonical(&root)? != registered {
            refusals.push(refuse(
                &task.id.to_string(),
                "other_checkout",
                format!(
                    "{} holds an open copy ({}); merge its records into the registered checkout or remove that checkout first",
                    root.display(),
                    task.status.as_str()
                ),
            ));
        }
    }

    // A pending rename or move on either project; any other failure propagates.
    let mut seen = BTreeSet::new();
    for project in [source, &ctx.target] {
        let pending =
            match crate::commands::pending_rename_at(Some(&project.root), &project.prefix)? {
                Some(detail) => Some(detail),
                None => match crate::commands::pending_move(&project.prefix)? {
                    Some(detail) => Some(detail),
                    // Another source's run naming this project; this source's own pending
                    // run is resumed, never preflighted.
                    None => crate::migrating::journal::pending_naming(
                        &project.prefix,
                        Some(&source.prefix),
                    )?
                    .map(|journal| journal.pending_error().to_string()),
                },
            };
        if let Some(detail) = pending
            && seen.insert(detail.clone())
        {
            refusals.push(refuse(&project.prefix, "pending_operation", detail));
        }
    }
    Ok(refusals)
}

fn canonical(path: &Path) -> Result<std::path::PathBuf> {
    std::fs::canonicalize(path)
        .map_err(|e| Error::UnreadableCheckout(format!("{}: {e}", path.display())))
}

/// A reason that looks like a `[reasons]` key but is not one is probably a typo, and is
/// still accepted as literal text.
pub fn reason_warnings(list: &DecisionList) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in &list.tasks {
        let Some(reason) = entry.reason.as_deref() else {
            continue;
        };
        // `^[a-z][a-z0-9-]*$`
        let mut chars = reason.chars();
        let key_shaped = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if key_shaped && !list.reasons.contains_key(reason) && seen.insert(reason.to_string()) {
            warnings.push(format!(
                "reason {reason:?} on {} is not a key of [reasons]; it is recorded as literal text (add the key if a shared reason was meant)",
                entry.id
            ));
        }
    }
    warnings
}
