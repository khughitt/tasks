//! What a stored dependency means once resolved, shared by every caller that judges one:
//! the move-aware lookup and the single satisfaction rule. Each caller supplies its own
//! lookup, so which copy of a record answers stays the caller's policy. See
//! docs/specs/2026-10-25-blocker-ranking-design.md (existing contracts).

use crate::error::Result;
use crate::model::{Task, TaskId};
use crate::registry::Registry;

/// What one resolved dependency says. Callers keep their own handling of `Unresolved`;
/// none of them may treat it as satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Satisfied,
    Open,
    Unresolved,
}

/// The record a dependency on `id` means: the end of its `moved_to` chain, read through
/// `lookup` with every hop canonicalized first. `None` for a missing record or a move loop;
/// a failing lookup is the caller's error.
pub fn resolve(
    registry: &Registry,
    id: &TaskId,
    mut lookup: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> Result<Option<Task>> {
    let mut read = |id: &TaskId| lookup(&registry.canonical_id(id));
    let first = read(id)?;
    crate::tombstone::follow(first, read)
}

/// The one satisfaction rule: a closed record satisfies (done, recurring done until it is
/// restarted, dropped); an open one, shelved included, does not.
pub fn verdict(task: Option<&Task>) -> Verdict {
    match task {
        Some(task) if !task.status.is_open() => Verdict::Satisfied,
        Some(_) => Verdict::Open,
        None => Verdict::Unresolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::model::{Status, Task, TaskId};
    use crate::registry::Registry;
    use std::collections::HashMap;

    fn task(id: &str, status: Status, extra: &str) -> Task {
        let text = format!(
            "---\nid: {id}\ntitle: T\nstatus: {}\npriority: 2\n\
created: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: []\ntags: []\n{extra}---\n",
            status.as_str(),
        );
        crate::format::parse_task(&text, &format!("tasks/{id}.md")).unwrap()
    }

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    fn map(tasks: &[Task]) -> HashMap<TaskId, Task> {
        tasks
            .iter()
            .map(|task| (task.id.clone(), task.clone()))
            .collect()
    }

    fn judge(registry: &Registry, records: &HashMap<TaskId, Task>, of: &str) -> Verdict {
        let found = resolve(registry, &id(of), |id| Ok(records.get(id).cloned())).unwrap();
        verdict(found.as_ref())
    }

    #[test]
    fn dependency_lookup_authority_and_verdicts() {
        let registry = Registry::default();
        let closed = "closed: 2026-10-02T00:00:00Z\n";

        // The lookup the caller passes is the authority: one id, two copies, two answers.
        let a = map(&[task("sci-000001", Status::Todo, "")]);
        let b = map(&[task("sci-000001", Status::Done, closed)]);
        assert_eq!(judge(&registry, &a, "sci-000001"), Verdict::Open);
        assert_eq!(judge(&registry, &b, "sci-000001"), Verdict::Satisfied);
        let found = resolve(&registry, &id("sci-000001"), |id| Ok(b.get(id).cloned()))
            .unwrap()
            .unwrap();
        assert_eq!(found.closed.as_deref(), Some("2026-10-02T00:00:00Z"));

        // A recurring done satisfies even when its next occurrence is long due; a
        // restarted occurrence blocks again.
        let due = map(&[task(
            "sci-000002",
            Status::Done,
            &format!("every: 1d\n{closed}"),
        )]);
        assert_eq!(judge(&registry, &due, "sci-000002"), Verdict::Satisfied);
        let restarted = map(&[task("sci-000002", Status::Doing, "every: 1d\n")]);
        assert_eq!(judge(&registry, &restarted, "sci-000002"), Verdict::Open);

        let dropped = map(&[task("sci-000003", Status::Dropped, closed)]);
        assert_eq!(judge(&registry, &dropped, "sci-000003"), Verdict::Satisfied);
        let shelved = map(&[task("sci-000004", Status::Shelved, "")]);
        assert_eq!(judge(&registry, &shelved, "sci-000004"), Verdict::Open);
        assert_eq!(judge(&registry, &a, "sci-00000f"), Verdict::Unresolved);
        let looped = map(&[
            task("sci-000005", Status::Moved, "moved_to: fam-000005\n"),
            task("fam-000005", Status::Moved, "moved_to: sci-000005\n"),
        ]);
        assert_eq!(judge(&registry, &looped, "sci-000005"), Verdict::Unresolved);

        // An alias resolves to its live prefix before every lookup, including each move
        // hop, and the terminal record keeps its own canonical id.
        let mut aliased = Registry::default();
        aliased.aliases.insert("old".into(), "sci".into());
        aliased.aliases.insert("was".into(), "fam".into());
        let chain = map(&[
            task("sci-000006", Status::Moved, "moved_to: was-000006\n"),
            task("fam-000006", Status::Todo, ""),
        ]);
        let mut asked = Vec::new();
        let found = resolve(&aliased, &id("old-000006"), |id| {
            asked.push(id.to_string());
            Ok(chain.get(id).cloned())
        })
        .unwrap()
        .unwrap();
        assert_eq!(found.id, id("fam-000006"));
        assert_eq!(asked, ["sci-000006", "fam-000006"]);
        assert_eq!(verdict(Some(&found)), Verdict::Open);

        // A lookup that fails is the caller's error, never an unresolved dependency.
        let failed = resolve(&registry, &id("sci-000007"), |_| {
            Err(Error::Validation("corrupt record".into()))
        });
        assert!(matches!(failed, Err(Error::Validation(message)) if message == "corrupt record"));
    }
}
