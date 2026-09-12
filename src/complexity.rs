//! The cutoff a session picks under, and the effective rating it is compared with.
//! See docs/specs/2026-09-12-task-complexity-design.md §4.

use crate::claims::ClaimSnapshot;
use crate::error::{Error, Result};
use crate::model::{Complexity, Task};

pub const ENV: &str = "TASKS_MAX_COMPLEXITY";

/// The cutoff in force: the flag, else `TASKS_MAX_COMPLEXITY`. The variable is validated
/// whenever a picker runs, even when the flag overrides it, so a harness with a bad value
/// hears about it at once (spec §4.2).
pub fn cutoff(flag: Option<&str>) -> Result<Option<Complexity>> {
    cutoff_with(flag, Some(std::env::var(ENV)))
}

/// `env` is the variable's lookup result, injected so tests never touch the process
/// environment; `None` stands for an unset variable.
pub fn cutoff_with(
    flag: Option<&str>,
    env: Option<std::result::Result<String, std::env::VarError>>,
) -> Result<Option<Complexity>> {
    let from_env = match env {
        None | Some(Err(std::env::VarError::NotPresent)) => None,
        Some(Err(std::env::VarError::NotUnicode(_))) => {
            return Err(Error::Validation(format!("{ENV} is not valid UTF-8")));
        }
        Some(Ok(value)) if value.is_empty() => None,
        Some(Ok(value)) => Some(Complexity::parse(&value).map_err(|_| {
            Error::Validation(format!(
                "{ENV} must be one of low, mid, high, got {value:?}"
            ))
        })?),
    };
    match flag {
        Some(flag) => Ok(Some(Complexity::parse(flag)?)),
        None => Ok(from_env),
    }
}

/// The record's rating or the shared escalation, whichever is higher (spec §4.1).
pub fn effective(task: &Task, claims: &ClaimSnapshot) -> Option<Complexity> {
    let escalated = claims
        .escalation(&task.id)
        .map(|escalation| escalation.level);
    match (task.complexity, escalated) {
        (Some(record), Some(escalation)) => Some(record.max(escalation)),
        (record, escalation) => record.or(escalation),
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Hidden {
    pub above: usize,
    pub unassessed: usize,
}

/// Removes what the cutoff hides and counts it by cause. Unassessed is hidden, never
/// defaulted (spec §4.3).
pub fn apply(tasks: &mut Vec<Task>, cutoff: Complexity, claims: &ClaimSnapshot) -> Hidden {
    let mut hidden = Hidden::default();
    tasks.retain(|task| match effective(task, claims) {
        None => {
            hidden.unassessed += 1;
            false
        }
        Some(level) if level > cutoff => {
            hidden.above += 1;
            false
        }
        Some(_) => true,
    });
    hidden
}

pub fn warnings(cutoff: Complexity, hidden: &Hidden) -> Vec<String> {
    let mut out = Vec::new();
    if hidden.above > 0 {
        out.push(format!(
            "max-complexity {}: {} above cutoff hidden",
            cutoff.as_str(),
            hidden.above
        ));
    }
    if hidden.unassessed > 0 {
        out.push(format!(
            "max-complexity {}: {} unassessed hidden",
            cutoff.as_str(),
            hidden.unassessed
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Status, TaskId};
    use std::collections::BTreeMap;

    fn task(id: &str, complexity: Option<Complexity>) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: "t".into(),
            status: Status::Todo,
            priority: 2,
            size: None,
            complexity,
            parallel: false,
            every: None,
            owner: None,
            created: "2026-09-12T00:00:00Z".into(),
            updated: "2026-09-12T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            model: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    fn snapshot_with(id: &str, level: Complexity) -> ClaimSnapshot {
        let mut escalations = BTreeMap::new();
        escalations.insert(
            id.to_string(),
            crate::claims::Escalation {
                level,
                at: "2026-09-12T00:00:00Z".into(),
                session: "s:a".into(),
            },
        );
        ClaimSnapshot::from_parts(BTreeMap::new(), BTreeMap::new(), escalations)
    }

    #[test]
    fn flag_wins_and_the_variable_is_validated_regardless() {
        assert_eq!(cutoff_with(None, None).unwrap(), None);
        assert_eq!(cutoff_with(None, Some(Ok(String::new()))).unwrap(), None);
        assert_eq!(
            cutoff_with(None, Some(Ok("mid".into()))).unwrap(),
            Some(Complexity::Mid)
        );
        assert_eq!(
            cutoff_with(Some("low"), Some(Ok("mid".into()))).unwrap(),
            Some(Complexity::Low)
        );
        let error = cutoff_with(Some("high"), Some(Ok("huge".into()))).unwrap_err();
        assert!(
            error.to_string().contains("TASKS_MAX_COMPLEXITY"),
            "{error}"
        );
        assert!(cutoff_with(Some("huge"), None).is_err());
    }

    #[test]
    fn effective_takes_the_higher_of_record_and_escalation() {
        let empty = ClaimSnapshot::from_parts(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        assert_eq!(effective(&task("sci-000001", None), &empty), None);
        assert_eq!(
            effective(&task("sci-000001", Some(Complexity::Low)), &empty),
            Some(Complexity::Low)
        );
        let escalated = snapshot_with("sci-000001", Complexity::High);
        assert_eq!(
            effective(&task("sci-000001", None), &escalated),
            Some(Complexity::High)
        );
        assert_eq!(
            effective(&task("sci-000001", Some(Complexity::Low)), &escalated),
            Some(Complexity::High)
        );
        let lower = snapshot_with("sci-000001", Complexity::Low);
        assert_eq!(
            effective(&task("sci-000001", Some(Complexity::Mid)), &lower),
            Some(Complexity::Mid)
        );
    }

    #[test]
    fn apply_hides_above_and_unassessed_and_counts_each() {
        let empty = ClaimSnapshot::from_parts(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let mut tasks = vec![
            task("sci-000001", Some(Complexity::Low)),
            task("sci-000002", Some(Complexity::Mid)),
            task("sci-000003", Some(Complexity::High)),
            task("sci-000004", None),
        ];
        let hidden = apply(&mut tasks, Complexity::Mid, &empty);
        assert_eq!(
            hidden,
            Hidden {
                above: 1,
                unassessed: 1
            }
        );
        let ids: Vec<String> = tasks.iter().map(|t| t.id.to_string()).collect();
        assert_eq!(ids, vec!["sci-000001", "sci-000002"]);
        assert_eq!(
            warnings(Complexity::Mid, &hidden),
            vec![
                "max-complexity mid: 1 above cutoff hidden",
                "max-complexity mid: 1 unassessed hidden"
            ]
        );
        assert!(warnings(Complexity::Mid, &Hidden::default()).is_empty());
    }
}
