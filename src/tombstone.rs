//! The `moved` tombstone and the pointer between a moved task's two records. See
//! docs/specs/2026-10-08-task-move-design.md §3.

use crate::error::{Error, Result};
use crate::model::{Status, Task};
use crate::registry::Registry;
use clap::Parser;

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
