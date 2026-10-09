//! The `moved` tombstone and the pointer between a moved task's two records. See
//! docs/specs/2026-10-08-task-move-design.md §3.

use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use clap::Parser;
use std::collections::BTreeSet;

/// A tombstone carries exactly one pointer, to another project; a destination names
/// another project as its source. Anything else is a hand edit outside `tasks move`.
pub fn validate(task: &Task) -> Result<()> {
    match (task.status, &task.moved_to) {
        (Status::Moved, None) => {
            return Err(Error::Validation(format!(
                "{} is moved but has no moved_to",
                task.id
            )));
        }
        (Status::Moved, Some(to)) if to.prefix == task.id.prefix => {
            return Err(Error::Validation(format!(
                "{} moved_to {to} is in the same project",
                task.id
            )));
        }
        (status, Some(_)) if status != Status::Moved => {
            return Err(Error::Validation(format!(
                "{} has moved_to but status {}",
                task.id,
                status.as_str()
            )));
        }
        _ => {}
    }
    if let Some(from) = &task.moved_from
        && from.prefix == task.id.prefix
    {
        return Err(Error::Validation(format!(
            "{} moved_from {from} is in the same project",
            task.id
        )));
    }
    Ok(())
}

/// The tombstone half of `commands::guard_write`, which every record write passes.
pub fn refuse_write(task: &Task, spells: impl Fn(&str) -> bool, registry: &Registry) -> Result<()> {
    let (Status::Moved, Some(to)) = (task.status, &task.moved_to) else {
        return Ok(());
    };
    let mut args: Vec<String> = std::env::args()
        .skip(1)
        .map(|arg| if spells(&arg) { to.to_string() } else { arg })
        .collect();
    let mut alternative = String::new();
    // Reuse the parsed command so ordinary payloads never become feedback flags.
    if let crate::cli::Command::Feedback { summary, .. } = crate::cli::Cli::parse().command {
        let mut flags = args.iter_mut();
        while let Some(arg) = flags.next() {
            if arg == "--" {
                break;
            }
            if arg == "--project" {
                let prefix = flags.next().expect("feedback project was parsed");
                if registry.canonical_prefix(prefix) == task.id.prefix {
                    *prefix = to.prefix.clone();
                }
            } else if let Some(prefix) = arg.strip_prefix("--project=") {
                if registry.canonical_prefix(prefix) == task.id.prefix {
                    *arg = format!("--project={}", to.prefix);
                }
            } else if let Some(id) = arg.strip_prefix("--recur=")
                && spells(id)
            {
                *arg = format!("--recur={to}");
            }
        }
        alternative = format!(
            "; if {} does not accept feedback, append the report with `tasks note {to} -- {}`",
            to.prefix,
            crate::stale::quote(&summary)
        );
    }
    let args: Vec<_> = args.iter().map(|arg| crate::stale::quote(arg)).collect();
    Err(Error::Moved(format!(
        "{} moved to {to}; run `tasks {}` instead{alternative}",
        task.id,
        args.join(" ")
    )))
}

/// A new dependency or parent must name the live record, never a tombstone.
pub fn refuse_reference(task: &Task, role: &str) -> Result<()> {
    match (task.status, &task.moved_to) {
        (Status::Moved, Some(to)) => Err(Error::Moved(format!(
            "{role} {} moved to {to}; name {to} instead",
            task.id
        ))),
        _ => Ok(()),
    }
}

/// The record a dependency on `first` means: itself when live, else the end of its
/// `moved_to` chain (spec §3.4). `None` when a hop is unreachable or the chain revisits an
/// id; callers already treat `None` as an open, unreachable dependency.
pub fn follow(
    first: Option<Task>,
    mut resolve: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> Result<Option<Task>> {
    let mut seen = BTreeSet::new();
    let mut current = first;
    while let Some(task) = current {
        let (Status::Moved, Some(to)) = (task.status, task.moved_to.clone()) else {
            return Ok(Some(task));
        };
        if !seen.insert(task.id.clone()) {
            return Ok(None);
        }
        current = resolve(&to)?;
    }
    Ok(None)
}

/// The edges a record contributes to the dependency graph. A tombstone's retained
/// `depends` are history; its only edge is its pointer (spec §3.4).
pub fn edges(task: &Task) -> Vec<TaskId> {
    match (task.status, &task.moved_to) {
        (Status::Moved, Some(to)) => vec![to.clone()],
        _ => task.depends.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskId;

    fn task(id: &str, status: Status, to: Option<&str>, depends: &[&str]) -> Task {
        let text = format!(
            "---\nid: {id}\ntitle: T\nstatus: {}\npriority: 2\n\
created: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: [{}]\ntags: []\n{}---\n",
            status.as_str(),
            depends.join(", "),
            to.map(|to| format!("moved_to: {to}\n")).unwrap_or_default(),
        );
        crate::format::parse_task(&text, &format!("tasks/{id}.md")).unwrap()
    }

    #[test]
    fn follow_ends_at_the_live_record() {
        let a = task("dots-00000a", Status::Moved, Some("wali-00000b"), &[]);
        let b = task("wali-00000b", Status::Moved, Some("ops-00000c"), &[]);
        let c = task("ops-00000c", Status::Todo, None, &[]);
        let all = [b.clone(), c.clone()];
        let got = follow(Some(a), |id| Ok(all.iter().find(|t| &t.id == id).cloned())).unwrap();
        assert_eq!(got.unwrap().id, c.id);
    }

    #[test]
    fn follow_is_none_on_an_unreachable_hop_or_a_revisit() {
        let a = task("dots-00000a", Status::Moved, Some("wali-00000b"), &[]);
        assert!(follow(Some(a.clone()), |_| Ok(None)).unwrap().is_none());
        let b = task("wali-00000b", Status::Moved, Some("dots-00000a"), &[]);
        let all = [a.clone(), b];
        assert!(
            follow(Some(a), |id| Ok(all.iter().find(|t| &t.id == id).cloned()))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_tombstone_has_one_edge_its_pointer() {
        let a = task(
            "dots-00000a",
            Status::Moved,
            Some("wali-00000b"),
            &["dots-00000c"],
        );
        assert_eq!(edges(&a), vec![TaskId::parse("wali-00000b").unwrap()]);
        let live = task("dots-00000c", Status::Todo, None, &["dots-00000d"]);
        assert_eq!(edges(&live), vec![TaskId::parse("dots-00000d").unwrap()]);
    }
}
