use crate::attachments::{self, Ledger};
use crate::error::Result;
use crate::model::{Ref, Relation, Task};
use crate::output::{InventoryRow, Origin, RowLocation, RowStatus};
use crate::refs::{self, Location, Presence};
use crate::repo::Project;
use crate::resolve::{DocKind, DocRead, Fetch, Located, LocatedEntry, Resolver, UnavailableCause};

pub fn build(
    resolver: &Resolver,
    project: &Project,
    task: &Task,
    step_found: Option<bool>,
    fetch: Fetch,
    warnings: &mut Vec<String>,
) -> Result<Vec<InventoryRow>> {
    let mut rows = Vec::new();
    for (link, origin, kind) in [
        (&task.spec, Origin::Spec, DocKind::Spec),
        (&task.plan, Origin::Plan, DocKind::Plan),
    ] {
        let Some(reference) = link else { continue };
        let external = crate::format::external_doc(reference)?.is_some();
        let mut row = InventoryRow::new(
            origin,
            reference.clone(),
            if external {
                RowLocation::External
            } else {
                RowLocation::Path
            },
        );
        row.required = true;
        if !external {
            match resolver.locate(reference)? {
                Located::Here(path) | Located::Main { path, .. } => {
                    row.status = RowStatus::Present;
                    row.path = Some(path.display().to_string());
                }
                Located::Missing => match resolver.locate_entry(reference)? {
                    LocatedEntry::Special(path) => {
                        row.status = RowStatus::Special;
                        row.path = Some(path.display().to_string());
                        warn_special(&row, warnings);
                    }
                    _ => {
                        row.status = RowStatus::Missing;
                        row.path = Some(resolver.abs(kind, reference)?);
                    }
                },
            }
        } else if origin == Origin::Plan && task.step.is_some() && fetch == Fetch::Allowed {
            match resolver.read(DocKind::Plan, reference)? {
                DocRead::Available(_) => row.status = RowStatus::Available,
                DocRead::Unavailable {
                    cause: UnavailableCause::FetchDisabled,
                    ..
                } => {}
                DocRead::Unavailable { cause, .. } => {
                    row.status = RowStatus::Unavailable;
                    row.cause = Some(cause.detail());
                }
            }
        }
        if origin == Origin::Plan {
            row.step.clone_from(&task.step);
            row.step_found = step_found;
        }
        rows.push(row);
    }
    let governing_count = rows.len();
    for entry in &task.refs {
        let row = entry_row(resolver, project, task, entry, warnings)?;
        if let Some(governing) = rows[..governing_count]
            .iter_mut()
            .find(|r| r.reference == entry.reference)
        {
            let agrees = governing.location == RowLocation::External
                || (governing.status == RowStatus::Missing
                    && matches!(row.status, RowStatus::Missing | RowStatus::Pending))
                || (governing.status == row.status
                    && matches!(row.status, RowStatus::Present | RowStatus::Special)
                    && governing.path == row.path);
            if agrees {
                governing.handle = row.handle;
                governing.relation = row.relation;
                governing.kind = row.kind;
                governing.description = row.description;
                continue;
            }
            let target = |r: &InventoryRow| {
                if matches!(r.status, RowStatus::Missing | RowStatus::Pending) {
                    "missing"
                } else {
                    r.path.as_deref().expect("local target has a path")
                }
                .to_string()
            };
            warnings.push(format!(
                "reference {} {} resolves to {}, but the {} link resolves to {}",
                entry.handle,
                entry.reference,
                target(&row),
                governing.origin.as_str(),
                target(governing)
            ));
        }
        rows.push(row);
    }
    if let Some(source) = &task.source {
        rows.push(InventoryRow::new(
            Origin::Source,
            source.clone(),
            RowLocation::Opaque,
        ));
    }
    for (name, state) in attachments::ledger(task) {
        let reference = format!("files:{name}");
        if state != Ledger::Attached || task.refs.iter().any(|entry| entry.reference == reference) {
            continue;
        }
        let entry = Ref {
            handle: name,
            relation: Relation::Context,
            required: false,
            kind: None,
            reference,
            description: None,
        };
        let mut row = entry_row(resolver, project, task, &entry, warnings)?;
        row.origin = Origin::Attachment;
        row.handle = None;
        row.relation = None;
        rows.push(row);
    }
    Ok(rows)
}

fn entry_row(
    resolver: &Resolver,
    project: &Project,
    task: &Task,
    entry: &Ref,
    warnings: &mut Vec<String>,
) -> Result<InventoryRow> {
    let location = match refs::location(&entry.reference)? {
        Location::Path(_) => RowLocation::Path,
        Location::Attachment(_) => RowLocation::Attachment,
        Location::External => RowLocation::External,
    };
    let mut row = InventoryRow::new(Origin::Ref, entry.reference.clone(), location);
    row.required = entry.required;
    row.handle = Some(entry.handle.clone());
    row.relation = Some(entry.relation);
    row.kind.clone_from(&entry.kind);
    row.description.clone_from(&entry.description);
    if location != RowLocation::External {
        let (status, path) = match refs::present(resolver, project, task, entry)? {
            Presence::Here(path) => (RowStatus::Present, path),
            Presence::Main { root, path } => {
                resolver.note_main_entry(&entry.handle, &entry.reference, &root);
                (RowStatus::Present, path)
            }
            Presence::Absent(path) => (
                if entry.relation == Relation::Output {
                    RowStatus::Pending
                } else {
                    RowStatus::Missing
                },
                path,
            ),
            Presence::Special(path) => (RowStatus::Special, path),
            Presence::Unsafe { path, .. } => (RowStatus::Missing, path),
        };
        row.status = status;
        row.path = Some(path.display().to_string());
        if status == RowStatus::Special {
            warn_special(&row, warnings);
        }
    }
    Ok(row)
}

fn warn_special(row: &InventoryRow, warnings: &mut Vec<String>) {
    warnings.push(format!(
        "reference {} is a special file; it is not opened",
        row.path.as_deref().expect("special target has a path")
    ));
}
