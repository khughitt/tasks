#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_id() {
        let id = TaskId::parse("sci-4f2a9c").unwrap();
        assert_eq!(id.prefix, "sci");
        assert_eq!(id.hex, "4f2a9c");
        assert_eq!(id.to_string(), "sci-4f2a9c");
    }

    #[test]
    fn rejects_bad_ids() {
        for bad in [
            "sci-4f2a",
            "SCI-4f2a9c",
            "sci-4F2A9C",
            "4f2a9c",
            "sci_4f2a9c",
            "s-4f2a9c",
            "sci-4f2a9cz",
        ] {
            assert!(TaskId::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn status_roundtrip_and_openness() {
        for s in Status::ALL {
            assert_eq!(Status::parse(s.as_str()).unwrap(), s);
        }
        assert_eq!(Status::ALL.len(), 7);
        assert_eq!(Status::parse("shelved").unwrap(), Status::Shelved);
        assert!(Status::Idea.is_open());
        assert!(Status::Blocked.is_open());
        assert!(
            Status::Shelved.is_open(),
            "shelved keeps blocking dependents"
        );
        assert!(!Status::Done.is_open());
        assert!(!Status::Dropped.is_open());
    }

    #[test]
    fn transition_table() {
        use Status::*;
        assert!(Status::can_transition(Idea, Todo));
        assert!(Status::can_transition(Doing, Blocked));
        assert!(Status::can_transition(Todo, Done));
        assert!(Status::can_transition(Blocked, Dropped));
        assert!(Status::can_transition(Done, Todo));
        assert!(Status::can_transition(Dropped, Todo));
        assert!(Status::can_transition(Idea, Shelved));
        assert!(Status::can_transition(Doing, Shelved));
        assert!(Status::can_transition(Shelved, Idea));
        assert!(Status::can_transition(Shelved, Todo));
        assert!(
            !Status::can_transition(Done, Shelved),
            "closed reopens to todo only"
        );
        assert!(!Status::can_transition(Dropped, Shelved));
        assert!(!Status::can_transition(Done, Doing));
        assert!(!Status::can_transition(Done, Dropped));
        assert!(!Status::can_transition(Dropped, Done));
        assert!(Status::can_transition(Todo, Todo));
    }

    #[test]
    fn size_order() {
        assert!(Size::Xs < Size::Xl);
        assert_eq!(Size::parse("m").unwrap(), Size::M);
        assert!(Size::parse("huge").is_err());
    }

    #[test]
    fn complexity_order_and_parse() {
        assert!(Complexity::Low < Complexity::Mid);
        assert!(Complexity::Mid < Complexity::High);
        for level in Complexity::ALL {
            assert_eq!(Complexity::parse(level.as_str()).unwrap(), level);
        }
        let error = Complexity::parse("medium").unwrap_err().to_string();
        assert!(error.contains("low, mid, high"), "{error}");
    }

    fn task_with(
        status: Status,
        spec: Option<&str>,
        plan: Option<&str>,
        step: Option<&str>,
    ) -> Task {
        Task {
            id: TaskId::parse("sci-000001").unwrap(),
            title: "T".into(),
            status,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            every: None,
            owner: None,
            created: "2026-09-09T00:00:00Z".into(),
            updated: "2026-09-09T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            model: None,
            agent: None,
            spec: spec.map(Into::into),
            plan: plan.map(Into::into),
            step: step.map(Into::into),
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn phase_is_derived_first_match_wins() {
        assert_eq!(
            Phase::of(&task_with(Status::Idea, Some("s"), Some("p"), None)),
            Phase::Brainstorming
        );
        assert_eq!(
            Phase::of(&task_with(Status::Todo, None, Some("p"), None)),
            Phase::Implementing
        );
        assert_eq!(
            Phase::of(&task_with(Status::Doing, Some("s"), None, Some("Task 1"))),
            Phase::Implementing
        );
        assert_eq!(
            Phase::of(&task_with(Status::Todo, Some("s"), None, None)),
            Phase::Planning
        );
        assert_eq!(
            Phase::of(&task_with(Status::Todo, None, None, None)),
            Phase::Implementing,
            "scoped work needs no spec"
        );
        assert_eq!(
            Phase::of(&task_with(Status::Blocked, Some("s"), None, None)),
            Phase::Planning
        );
        assert_eq!(Phase::Planning.as_str(), "planning");
    }
}
use crate::error::{Error, Result};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId {
    pub prefix: String,
    pub hex: String,
}

impl TaskId {
    pub fn parse(s: &str) -> Result<TaskId> {
        let Some((prefix, hex)) = s.rsplit_once('-') else {
            return Err(Error::InvalidId(
                s.into(),
                "expected <prefix>-<hex6>".into(),
            ));
        };
        if !is_valid_prefix(prefix) {
            return Err(Error::InvalidId(
                s.into(),
                "prefix must match [a-z][a-z0-9]{1,7}".into(),
            ));
        }
        let hex_ok = hex.len() == 6
            && hex
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if !hex_ok {
            return Err(Error::InvalidId(
                s.into(),
                "suffix must be six lowercase hex digits".into(),
            ));
        }
        Ok(TaskId {
            prefix: prefix.into(),
            hex: hex.into(),
        })
    }
}

pub fn is_valid_prefix(p: &str) -> bool {
    let mut chars = p.chars();
    (2..=8).contains(&p.len())
        && matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.prefix, self.hex)
    }
}

impl Serialize for TaskId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idea,
    Todo,
    Doing,
    Blocked,
    Shelved,
    Done,
    Dropped,
}

impl Status {
    pub const ALL: [Status; 7] = [
        Status::Idea,
        Status::Todo,
        Status::Doing,
        Status::Blocked,
        Status::Shelved,
        Status::Done,
        Status::Dropped,
    ];

    pub fn parse(s: &str) -> Result<Status> {
        Status::ALL
            .into_iter()
            .find(|st| st.as_str() == s)
            .ok_or_else(|| Error::Validation(format!("unknown status {s:?}")))
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Idea => "idea",
            Status::Todo => "todo",
            Status::Doing => "doing",
            Status::Blocked => "blocked",
            Status::Shelved => "shelved",
            Status::Done => "done",
            Status::Dropped => "dropped",
        }
    }

    pub fn is_open(self) -> bool {
        !matches!(self, Status::Done | Status::Dropped)
    }

    pub fn can_transition(from: Status, to: Status) -> bool {
        if from == to {
            return true;
        }
        match (from.is_open(), to.is_open()) {
            (true, _) => true,
            (false, true) => to == Status::Todo,
            (false, false) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Xs,
    S,
    M,
    L,
    Xl,
}

impl Size {
    pub const ALL: [Size; 5] = [Size::Xs, Size::S, Size::M, Size::L, Size::Xl];
    pub fn parse(s: &str) -> Result<Size> {
        Size::ALL
            .into_iter()
            .find(|z| z.as_str() == s)
            .ok_or_else(|| Error::Validation(format!("unknown size {s:?}")))
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Size::Xs => "xs",
            Size::S => "s",
            Size::M => "m",
            Size::L => "l",
            Size::Xl => "xl",
        }
    }
}

/// The reasoning and judgment a task demands given its current spec, plan, and context.
/// Three levels, ordered; absent means unassessed and is never defaulted. See
/// docs/specs/2026-09-12-task-complexity-design.md §3.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    Low,
    Mid,
    High,
}

impl Complexity {
    pub const ALL: [Complexity; 3] = [Complexity::Low, Complexity::Mid, Complexity::High];

    pub fn parse(s: &str) -> Result<Complexity> {
        Complexity::ALL
            .into_iter()
            .find(|level| level.as_str() == s)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "unknown complexity {s:?}; expected one of low, mid, high"
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Complexity::Low => "low",
            Complexity::Mid => "mid",
            Complexity::High => "high",
        }
    }
}

/// The explicitly selected workflow; absence means unassessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Process {
    Direct,
    Planned,
}

impl Process {
    pub const ALL: [Process; 2] = [Process::Direct, Process::Planned];

    pub fn parse(s: &str) -> Result<Process> {
        Self::ALL
            .into_iter()
            .find(|process| process.as_str() == s)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "unknown process {s:?}; expected one of direct, planned"
                ))
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Planned => "planned",
        }
    }
}

/// Where a parked task was left. Derived from its design links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Brainstorming,
    Planning,
    Implementing,
}

impl Phase {
    pub fn of(task: &Task) -> Phase {
        if task.status == Status::Idea {
            return Phase::Brainstorming;
        }
        if task.plan.is_some() || task.step.is_some() {
            return Phase::Implementing;
        }
        if task.spec.is_some() {
            return Phase::Planning;
        }
        Phase::Implementing
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Brainstorming => "brainstorming",
            Phase::Planning => "planning",
            Phase::Implementing => "implementing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Note {
    pub at: String,
    pub by: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub status: Status,
    pub priority: u8,
    pub size: Option<Size>,
    /// The judgment the task demands; absent is unassessed. Set by `add`/`edit
    /// --complexity` and by `park --reason capability`. See
    /// docs/specs/2026-09-12-task-complexity-design.md.
    pub complexity: Option<Complexity>,
    /// Chosen explicitly; never inferred from complexity, parentage, or document links.
    pub process: Option<Process>,
    /// Marked safe to run beside any other task marked parallel. Hand-set; nothing
    /// infers or validates it. See docs/specs/2026-09-06-parallel-candidates-design.md.
    pub parallel: bool,
    /// The recurrence interval. A closed task carrying one falls due again; an open one is
    /// an ordinary task. See docs/specs/2026-09-09-periodic-design.md.
    pub every: Option<crate::periodic::Interval>,
    pub owner: Option<String>,
    pub created: String,
    pub updated: String,
    /// When work first began: stamped by the first transition into `doing` and never
    /// moved. See docs/specs/2026-09-11-park-reason-and-stamps-design.md §3.
    pub started: Option<String>,
    /// When the task was last completed: stamped by every completing transition and
    /// cleared by any transition out of `done`. Unlike `last_done` it exists on every
    /// record and does not survive a reopen. Same spec, §3.
    pub completed: Option<String>,
    /// The completion that anchors the current cycle. Stamped only alongside `every`, and
    /// only by a transition that actually completes the task (spec §4.4).
    pub last_done: Option<String>,
    pub depends: Vec<TaskId>,
    pub parent: Option<TaskId>,
    pub tags: Vec<String>,
    /// Where the task came from: an opaque, single-line reference such as a URL or a
    /// message id. Stored and returned, never interpreted or resolved. See
    /// docs/specs/2026-09-06-task-source-design.md.
    pub source: Option<String>,
    /// The model id the harness reported (`TASKS_MODEL`) for the session that ran the
    /// latest completion transition; `None` when unknown or cleared. Stored and returned,
    /// never interpreted. See docs/specs/2026-09-10-model-provenance-design.md.
    pub model: Option<String>,
    /// The harness and model that filed the task (`<harness>/<model>`, or the harness
    /// alone), from `add --agent` or `TASKS_AGENT` at creation; `None` when unknown.
    /// Stored and returned, never interpreted. Independent of `model`, which is the
    /// completion side. See docs/specs/2026-09-13-creation-provenance-design.md.
    pub agent: Option<String>,
    pub spec: Option<String>,
    pub plan: Option<String>,
    pub step: Option<String>,
    pub body: String,
    pub notes: Vec<Note>,
}
