//! The `moved` tombstone and the pointer between a moved task's two records. See
//! docs/project-operations.md

use crate::error::{Error, Result};
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use clap::{CommandFactory, Parser};
use std::collections::BTreeSet;

/// A tombstone carries exactly one pointer, to another project; a destination names
/// another project as its source. Project validation handles a reclaimed source prefix.
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
    Ok(())
}

/// The tombstone half of `commands::guard_write`, which every record write passes.
pub fn refuse_write(task: &Task, spells: impl Fn(&str) -> bool, registry: &Registry) -> Result<()> {
    let (Status::Moved, Some(to)) = (task.status, &task.moved_to) else {
        return Ok(());
    };
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    for index in id_slots(&args, &spells) {
        args[index] = to.to_string();
    }
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
            "; if {to} is no longer open feedback, append the report with `tasks note {to} -- {}`",
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

/// The positions in `args` (argv after the binary) that spell the tombstone *as an id*:
/// clap parses them into an argument that takes a ref (`cli::takes_ref`). A payload that happens
/// to spell it (a note, a title, an attachment path) is not one, so the retry keeps it.
///
/// Each candidate is tested alone: it is swapped for a marker no id or flag can spell and
/// the command is parsed again. A parse that fails means the token sits in a typed slot
/// (a number, a date), which a task id never fills.
fn id_slots(args: &[String], spells: impl Fn(&str) -> bool) -> Vec<usize> {
    const MARKER: &str = "\u{1}tasks-id-slot";
    let command = crate::cli::Cli::command();
    (0..args.len())
        .filter(|&index| spells(&args[index]))
        .filter(|&index| {
            let argv = std::iter::once("tasks").chain(
                args.iter()
                    .enumerate()
                    .map(|(i, arg)| if i == index { MARKER } else { arg.as_str() }),
            );
            command
                .clone()
                .try_get_matches_from(argv)
                .is_ok_and(|matches| ref_value(&command, &matches, MARKER))
        })
        .collect()
}

/// Whether `value` was parsed into an argument that takes a ref, of the matched command path.
fn ref_value(command: &clap::Command, matches: &clap::ArgMatches, value: &str) -> bool {
    let here = command
        .get_arguments()
        .filter(|arg| crate::cli::takes_ref(arg))
        .any(|arg| {
            matches
                .get_raw(arg.get_id().as_str())
                .is_some_and(|mut raw| raw.any(|given| given == value))
        });
    here || matches.subcommand().is_some_and(|(name, sub)| {
        let sub_command = command
            .find_subcommand(name)
            .expect("a matched subcommand is defined");
        ref_value(sub_command, sub, value)
    })
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
/// `moved_to` chain (docs/task-model.md). `None` when a hop is unreachable or the chain revisits an
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
/// `depends` are history; its only edge is its pointer (docs/task-model.md).
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
        crate::history::fixture(&text, &format!("tasks/{id}.md")).unwrap()
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
