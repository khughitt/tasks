//! `tasks register` and `tasks notices`: take up what the records stores hold and this
//! host's registry does not (docs/project-operations.md#taking-up-synced-records).
use crate::catalogue::discovery::Catalogue;
use crate::catalogue::pending::{Pending, Registration};
use crate::error::{Error, Result};
use crate::output::{NoticeRow, NoticesOut, Output, RegisterOut, RegisterRow};
use crate::registry::Registry;

/// Runs `tasks init` for every registerable entry, or for the named ones, which must all
/// be registerable; a named prefix that is not refuses before anything is registered.
/// Every entry is attempted; a failed init is an `error` row and the command exits 1.
pub fn run(prefixes: Vec<String>, explain: bool) -> Result<Output> {
    let registry = Registry::load()?;
    let catalogue = Catalogue::build(&registry);
    let pending = Pending::find(&registry, &catalogue);
    let chosen = select(&registry, &pending, &prefixes)?;
    let mut warnings = Vec::new();
    let results = chosen
        .into_iter()
        .map(|entry| {
            let mut row = RegisterRow {
                prefix: entry.prefix.clone(),
                kind: entry.kind.as_str(),
                records: entry.records.display().to_string(),
                checkout: entry.checkout.display().to_string(),
                command: entry.command(),
                outcome: "would_register",
                detail: None,
            };
            if explain {
                return row;
            }
            match super::init::run(
                Some(&entry.dir),
                Some(entry.prefix.clone()),
                entry.linked_checkout.clone(),
                entry.records_root.clone(),
                false,
            ) {
                Ok(Output::Init(out)) => {
                    row.outcome = "registered";
                    warnings.extend(
                        out.warnings
                            .into_iter()
                            .map(|warning| format!("{}: {warning}", entry.prefix)),
                    );
                }
                Ok(_) => unreachable!("init returns its own output"),
                Err(error) => {
                    row.outcome = "error";
                    row.detail = Some(format!("{}: {error}", error.kind()));
                }
            }
            row
        })
        .collect();
    Ok(Output::Register(RegisterOut {
        explain,
        results,
        warnings,
    }))
}

fn select<'a>(
    registry: &Registry,
    pending: &'a Pending,
    prefixes: &[String],
) -> Result<Vec<&'a Registration>> {
    if prefixes.is_empty() {
        return Ok(pending.registrations.iter().collect());
    }
    let mut chosen = Vec::new();
    let mut refused = Vec::new();
    for prefix in prefixes {
        if let Some(entry) = pending.registrations.iter().find(|e| &e.prefix == prefix) {
            if !chosen.contains(&entry) {
                chosen.push(entry);
            }
        } else if let Some(skip) = pending.skipped.get(prefix) {
            refused.push(format!("{prefix}: {skip}"));
        } else if registry
            .projects
            .contains_key(registry.canonical_prefix(prefix))
        {
            refused.push(format!("{prefix}: already registered on this host"));
        } else {
            refused.push(format!(
                "{prefix}: no records store this host knows holds that entry"
            ));
        }
    }
    if !refused.is_empty() {
        return Err(Error::Validation(format!(
            "nothing was registered; {}",
            refused.join("; ")
        )));
    }
    Ok(chosen)
}

/// One row per registerable entry and per unadopted rename; empty when the host is up to
/// date, which the CLI prints as nothing. Reads what `resolve --all` reads: no locks, no
/// registry writes, no git.
pub fn notices() -> Result<Output> {
    let registry = Registry::load()?;
    let catalogue = Catalogue::build(&registry);
    let pending = Pending::find(&registry, &catalogue);
    let mut notices: Vec<NoticeRow> = pending
        .registrations
        .iter()
        .map(|entry| NoticeRow {
            kind: "unregistered",
            prefix: entry.prefix.clone(),
            old: None,
            command: entry.command(),
            detail: format!(
                "{} is a records entry at {} with its checkout {} on this host, but this host \
                 has not registered it",
                entry.prefix,
                entry.records.display(),
                entry.checkout.display()
            ),
        })
        .collect();
    notices.extend(pending.adoptions.iter().map(|adoption| NoticeRow {
        kind: "unadopted_rename",
        prefix: adoption.new.clone(),
        old: Some(adoption.old.clone()),
        command: adoption.command(),
        detail: format!(
            "{} was renamed to {} on another host, and this host has not adopted the rename",
            adoption.old, adoption.new
        ),
    }));
    Ok(Output::Notices(NoticesOut { notices }))
}
