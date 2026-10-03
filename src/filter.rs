//! One selection over record fields for the read commands that filter
//! (docs/specs/2026-09-30-task-filters-design.md). Repeats of one field widen (any of
//! them); different fields narrow (all of them); `--tag` and `--need` are all-of. Each command
//! keeps its own default status pool; an explicit `--status` replaces it here.

use crate::claims::ClaimSnapshot;
use crate::cli::FilterArgs;
use crate::error::{Error, Result};
use crate::model::{Complexity, Process, Size, Status, Task, TaskId};
use crate::output::ParkedRow;
use crate::registry::Registry;
use crate::shorthand::Shorthand;

/// The flag value that selects a record whose field is unset.
pub const NONE: &str = "none";

#[derive(Debug, Default)]
pub struct TaskFilter {
    statuses: Vec<Status>,
    priorities: Vec<u8>,
    sizes: Vec<Option<Size>>,
    complexities: Vec<Option<Complexity>>,
    processes: Vec<Option<Process>>,
    tags: Vec<String>,
    needs: Vec<String>,
    owner: Option<String>,
    source: Option<String>,
    parent: Option<TaskId>,
    parallel: bool,
}

/// What the filter reads from a record, whichever row type carries it.
pub struct Fields<'a> {
    pub status: Status,
    pub priority: u8,
    pub size: Option<Size>,
    /// The effective rating: the record's or a shared escalation's, whichever is higher.
    pub complexity: Option<Complexity>,
    pub process: Option<Process>,
    pub tags: &'a [String],
    pub needs: &'a [String],
    pub owner: Option<&'a str>,
    pub source: Option<&'a str>,
    /// Canonical, so a retired prefix and its live one compare equal.
    pub parent: Option<TaskId>,
    pub parallel: bool,
}

fn optional<T>(value: &str, parse: impl Fn(&str) -> Result<T>) -> Result<Option<T>> {
    if value == NONE {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}

fn any_of<T: PartialEq>(wanted: &[T], value: &T) -> bool {
    wanted.is_empty() || wanted.contains(value)
}

impl TaskFilter {
    /// Fallible: clap has refused unknown enum values already, but `--parent` is free
    /// text and fails `invalid_id` here, before any scan.
    pub fn parse(
        args: &FilterArgs,
        statuses: &[String],
        registry: &Registry,
        shorthand: &Shorthand,
    ) -> Result<TaskFilter> {
        Ok(TaskFilter {
            statuses: statuses
                .iter()
                .map(|status| Status::parse(status))
                .collect::<Result<_>>()?,
            priorities: args.priorities.clone(),
            sizes: args
                .sizes
                .iter()
                .map(|value| optional(value, Size::parse))
                .collect::<Result<_>>()?,
            complexities: args
                .complexities
                .iter()
                .map(|value| optional(value, Complexity::parse))
                .collect::<Result<_>>()?,
            processes: args
                .processes
                .iter()
                .map(|value| optional(value, Process::parse))
                .collect::<Result<_>>()?,
            tags: args.tags.clone(),
            needs: args.needs.clone(),
            owner: args.owner.clone(),
            source: args.source.clone(),
            parent: args
                .parent
                .as_deref()
                .map(|id| crate::commands::parse_id(registry, shorthand, id))
                .transpose()?,
            parallel: args.parallel,
        })
    }

    /// The explicit `--status` set; empty means the command's default pool applies.
    pub fn statuses(&self) -> &[Status] {
        &self.statuses
    }

    /// No field is constrained.
    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty()
            && self.priorities.is_empty()
            && self.sizes.is_empty()
            && self.complexities.is_empty()
            && self.processes.is_empty()
            && self.tags.is_empty()
            && self.needs.is_empty()
            && self.owner.is_none()
            && self.source.is_none()
            && self.parent.is_none()
            && !self.parallel
    }

    pub fn matches(&self, fields: &Fields) -> bool {
        any_of(&self.statuses, &fields.status)
            && any_of(&self.priorities, &fields.priority)
            && any_of(&self.sizes, &fields.size)
            && any_of(&self.complexities, &fields.complexity)
            && any_of(&self.processes, &fields.process)
            && self.tags.iter().all(|tag| fields.tags.contains(tag))
            && self.needs.iter().all(|need| fields.needs.contains(need))
            && self
                .owner
                .as_deref()
                .is_none_or(|owner| fields.owner == Some(owner))
            && self
                .source
                .as_deref()
                .is_none_or(|source| fields.source == Some(source))
            && self
                .parent
                .as_ref()
                .is_none_or(|parent| fields.parent.as_ref() == Some(parent))
            && (!self.parallel || fields.parallel)
    }
}

impl<'a> Fields<'a> {
    pub fn of_task(task: &'a Task, claims: &ClaimSnapshot, registry: &Registry) -> Fields<'a> {
        Fields {
            status: task.status,
            priority: task.priority,
            size: task.size,
            complexity: crate::complexity::effective(task, claims),
            process: task.process,
            tags: &task.tags,
            needs: &task.needs,
            owner: task.owner.as_deref(),
            source: task.source.as_deref(),
            parent: task
                .parent
                .as_ref()
                .map(|parent| registry.canonical_id(parent)),
            parallel: task.parallel,
        }
    }

    /// None for an unresolved park: it has no record to match.
    pub fn of_row(row: &'a ParkedRow, registry: &Registry) -> Option<Fields<'a>> {
        Some(Fields {
            status: row.status?,
            priority: row.priority?,
            size: row.size,
            complexity: crate::complexity::higher(
                row.complexity,
                row.escalation.as_ref().map(|escalation| escalation.level),
            ),
            process: row.process,
            tags: &row.tags,
            needs: &row.needs,
            owner: row.owner.as_deref(),
            source: row.source.as_deref(),
            parent: row
                .parent
                .as_deref()
                .and_then(|parent| TaskId::parse(parent).ok())
                .map(|id| registry.canonical_id(&id)),
            parallel: row.parallel,
        })
    }
}

/// `--parent` must name a task in scope. `named_elsewhere` admits one that only a row
/// outside the scan names (a parked row read from another checkout).
pub fn check_parent(
    filter: &TaskFilter,
    all: &[Task],
    named_elsewhere: impl Fn(&TaskId) -> bool,
) -> Result<()> {
    match &filter.parent {
        Some(parent) if !all.iter().any(|task| task.id == *parent) && !named_elsewhere(parent) => {
            Err(Error::TaskNotFound(parent.to_string()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_TAGS: &[String] = &[];

    fn fields() -> Fields<'static> {
        Fields {
            status: Status::Todo,
            priority: 2,
            size: Some(Size::S),
            complexity: Some(Complexity::Mid),
            process: None,
            tags: NO_TAGS,
            needs: &[],
            owner: None,
            source: None,
            parent: None,
            parallel: false,
        }
    }

    #[test]
    fn an_empty_filter_matches_everything() {
        let filter = TaskFilter::default();
        assert!(filter.is_empty());
        assert!(filter.matches(&fields()));
    }

    #[test]
    fn repeats_widen_and_fields_narrow() {
        let filter = TaskFilter {
            priorities: vec![0, 2],
            sizes: vec![Some(Size::M), Some(Size::S)],
            ..TaskFilter::default()
        };
        assert!(!filter.is_empty());
        assert!(filter.matches(&fields()));
        let narrower = TaskFilter {
            priorities: vec![2],
            sizes: vec![Some(Size::M)],
            ..TaskFilter::default()
        };
        assert!(!narrower.matches(&fields()));
    }

    #[test]
    fn a_repeated_value_selects_as_once() {
        let twice = TaskFilter {
            sizes: vec![Some(Size::S), Some(Size::S)],
            ..TaskFilter::default()
        };
        let once = TaskFilter {
            sizes: vec![Some(Size::S)],
            ..TaskFilter::default()
        };
        assert_eq!(twice.matches(&fields()), once.matches(&fields()));
        assert!(twice.matches(&fields()));
    }

    #[test]
    fn none_selects_unset_alone_or_mixed() {
        let unassessed = TaskFilter {
            processes: vec![None],
            ..TaskFilter::default()
        };
        assert!(unassessed.matches(&fields()));
        let mixed = TaskFilter {
            processes: vec![Some(Process::Direct), None],
            ..TaskFilter::default()
        };
        assert!(mixed.matches(&fields()));
        let set_only = TaskFilter {
            processes: vec![Some(Process::Direct)],
            ..TaskFilter::default()
        };
        assert!(!set_only.matches(&fields()));
    }

    #[test]
    fn needs_are_all_of_like_tags() {
        let needs = vec!["quiet".to_string(), "owner".to_string()];
        let needy = Fields {
            needs: &needs,
            ..fields()
        };
        let one = TaskFilter {
            needs: vec!["quiet".into()],
            ..TaskFilter::default()
        };
        assert!(!one.is_empty());
        assert!(one.matches(&needy));
        assert!(!one.matches(&fields()));
        let both = TaskFilter {
            needs: vec!["quiet".into(), "owner".into()],
            ..TaskFilter::default()
        };
        assert!(both.matches(&needy));
        let extra = TaskFilter {
            needs: vec!["quiet".into(), "gpu".into()],
            ..TaskFilter::default()
        };
        assert!(!extra.matches(&needy));
    }

    #[test]
    fn tags_are_all_of_and_scalars_are_exact() {
        let tags = vec!["cli".to_string(), "picker".to_string()];
        let tagged = Fields {
            tags: &tags,
            owner: Some("keith"),
            parallel: true,
            ..fields()
        };
        let both = TaskFilter {
            tags: vec!["cli".into(), "picker".into()],
            owner: Some("keith".into()),
            parallel: true,
            ..TaskFilter::default()
        };
        assert!(both.matches(&tagged));
        let extra = TaskFilter {
            tags: vec!["cli".into(), "docs".into()],
            ..TaskFilter::default()
        };
        assert!(!extra.matches(&tagged));
        let parallel = TaskFilter {
            parallel: true,
            ..TaskFilter::default()
        };
        assert!(!parallel.matches(&fields()));
    }

    #[test]
    fn an_escalation_above_the_record_is_the_rating_matched() {
        // of_task reads complexity::effective; this pins the same rule of_row uses.
        assert_eq!(
            crate::complexity::higher(None, Some(Complexity::High)),
            Some(Complexity::High)
        );
        let escalated = Fields {
            complexity: crate::complexity::higher(Some(Complexity::Mid), Some(Complexity::High)),
            ..fields()
        };
        let high = TaskFilter {
            complexities: vec![Some(Complexity::High)],
            ..TaskFilter::default()
        };
        let none = TaskFilter {
            complexities: vec![None],
            ..TaskFilter::default()
        };
        assert!(high.matches(&escalated));
        assert!(!none.matches(&escalated));
    }

    #[test]
    fn an_unresolved_park_row_has_no_fields() {
        let park = crate::claims::Park {
            owner: "o".into(),
            session: "s:a".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: "2026-09-30T00:00:00Z".into(),
            next_step: "n".into(),
            waiting_on: crate::claims::WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "T".into(),
        };
        let row = ParkedRow::unresolved("sci-ffffff", &park);
        assert!(Fields::of_row(&row, &Registry::default()).is_none());
    }
}
