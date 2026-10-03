//! The lanes view (docs/specs/2026-10-03-lanes-needs-groups-design.md §5): one row per
//! open, unshelved lane, with its guidance, the step it could take now, and why the rest
//! of its live work cannot. `tasks lanes` and `prime` share it; `commands::lanes` gathers
//! what it reads.

use crate::claims::{ClaimSnapshot, WaitingOn};
use crate::halt::HaltSnapshot;
use crate::holds::{HoldSnapshot, Mine};
use crate::model::{Complexity, Status, Task, TaskId};
use crate::needs::{Vocabularies, Without};
use crate::output::TaskSummary;
use crate::registry::Registry;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use time::OffsetDateTime;

/// §5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LaneState {
    /// The lane is `blocked`.
    Paused,
    /// A pick exists.
    Ready,
    /// Steps exist, but every one waits for an exclusive need.
    Held,
    /// No step exists, and live descendants remain; `causes` says why.
    Waiting,
    /// No live descendant.
    Empty,
}

/// Who holds the need a skipped step waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HeldBy {
    /// A live claim of another session.
    Claim,
    /// The pick of an earlier lane in this view.
    Pick,
}

/// A step skipped because an exclusive need it uses is held.
#[derive(Debug, Clone, Serialize)]
pub struct HeldStep {
    pub id: TaskId,
    pub need: String,
    /// The claimed task, or the earlier lane's pick.
    pub holder: TaskId,
    pub by: HeldBy,
}

/// Every live descendant that is neither the pick nor an unpicked step, under its first
/// cause in §5.1's table order. Serialized sparse, in that order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Causes {
    #[serde(skip_serializing_if = "is_zero")]
    pub active: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub held: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub without: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub cutoff: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub halt: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub deferred: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub periodic: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub user: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub blocked: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub depends: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub goal: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub other: usize,
}

fn is_zero(count: &usize) -> bool {
    *count == 0
}

impl Causes {
    /// The non-zero counts, in table order.
    pub fn entries(&self) -> Vec<(&'static str, usize)> {
        [
            ("active", self.active),
            ("held", self.held),
            ("without", self.without),
            ("cutoff", self.cutoff),
            ("halt", self.halt),
            ("deferred", self.deferred),
            ("periodic", self.periodic),
            ("user", self.user),
            ("blocked", self.blocked),
            ("depends", self.depends),
            ("goal", self.goal),
            ("other", self.other),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    fn add(&mut self, cause: Cause) {
        let slot = match cause {
            Cause::Active => &mut self.active,
            Cause::Without => &mut self.without,
            Cause::Cutoff => &mut self.cutoff,
            Cause::Halt => &mut self.halt,
            Cause::Deferred => &mut self.deferred,
            Cause::Periodic => &mut self.periodic,
            Cause::User => &mut self.user,
            Cause::Blocked => &mut self.blocked,
            Cause::Depends => &mut self.depends,
            Cause::Goal => &mut self.goal,
            Cause::Other => &mut self.other,
        };
        *slot += 1;
    }
}

/// The table's causes other than `held`, which only the pick loop can assign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    Active,
    Without,
    Cutoff,
    Halt,
    Deferred,
    Periodic,
    User,
    Blocked,
    Depends,
    Goal,
    Other,
}

/// §5.3's `LaneRow`.
#[derive(Clone, Serialize)]
pub struct LaneRow {
    pub lane: TaskSummary,
    pub guidance: Option<String>,
    pub state: LaneState,
    pub pick: Option<TaskSummary>,
    /// Unpicked steps that were not skipped for a held need.
    pub steps: usize,
    /// Descendants with a live claim, reported in every state.
    pub active: Vec<TaskSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub held: Vec<HeldStep>,
    #[serde(skip_serializing_if = "Causes::is_empty")]
    pub causes: Causes,
}

/// Everything the builder reads, gathered once by the command.
pub struct Inputs<'a> {
    /// The scope's scan.
    pub all: &'a [Task],
    pub claims: &'a ClaimSnapshot,
    pub registry: &'a Registry,
    /// `Some(closed?)` for a reachable dependency, `None` for an unreachable one. Only
    /// the dependencies of `dependency_readers` are ever asked for.
    pub dependency: &'a dyn Fn(&TaskId) -> Option<bool>,
    /// Per prefix; a prefix without one gates nothing.
    pub halts: &'a HashMap<String, HaltSnapshot>,
    pub cutoff: Option<Complexity>,
    pub without: &'a Without,
    pub holds: &'a HoldSnapshot,
    /// Each scanned project's need vocabulary, by prefix.
    pub vocabularies: &'a Vocabularies<'a>,
    /// The caller's own live holds (`commands::read_holds`), for the hold gate;
    /// `Mine::default()` counts every hold as another session's.
    pub mine: &'a Mine,
    pub now: OffsetDateTime,
}

/// §5.1: every open, unshelved lane in scope, in ready order (§3.7). Earlier lanes win
/// contested exclusive needs, because lane order is the person's ranking.
pub fn build(inputs: &Inputs) -> Vec<LaneRow> {
    let mut lanes: Vec<&Task> = inputs
        .all
        .iter()
        .filter(|task| task.lane && crate::hierarchy::is_active(task))
        .collect();
    lanes.sort_by(|a, b| crate::query::ready_order(a, b));
    let mut picked: BTreeMap<String, TaskId> = BTreeMap::new();
    lanes
        .into_iter()
        .map(|lane| row(inputs, lane, &mut picked))
        .collect()
}

/// §3.5: the first paragraph of the body, after leading blank and heading lines, up to
/// the next blank line. Its lines join with a space, so the guidance is one line.
pub fn guidance(body: &str) -> Option<String> {
    let paragraph: Vec<&str> = body
        .lines()
        .skip_while(|line| line.trim().is_empty() || is_heading(line))
        .take_while(|line| !line.trim().is_empty())
        .map(str::trim)
        .collect();
    (!paragraph.is_empty()).then(|| paragraph.join(" "))
}

/// An ATX heading: one to six `#`, then a space or the end of the line.
fn is_heading(line: &str) -> bool {
    let line = line.trim_start();
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes)
        && line[hashes..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
}

/// §5.1 step 4: open and unshelved, or a `done` recurrence between occurrences, which
/// `ready` offers again once due. A dropped record keeps its cadence but never returns.
fn is_live(task: &Task) -> bool {
    crate::hierarchy::is_active(task) || (task.status == Status::Done && task.every.is_some())
}

/// Work `next` could hand out once its other gates pass: a `ready_tasks` candidate (todo,
/// or a due recurrence) or an open, unblocked, unshelved parked-agent candidate. These
/// are the records `ready_tasks` and `parked::candidates` read dependencies for, and the
/// only ones the view reads them for.
fn could_step(task: &Task, claims: &ClaimSnapshot, now: OffsetDateTime) -> bool {
    crate::query::is_candidate(task, now)
        || claims.park(&task.id).is_some_and(|park| {
            park.waiting_on == WaitingOn::Agent
                && task.status.is_open()
                && !matches!(task.status, Status::Blocked | Status::Shelved)
        })
}

/// The records whose dependencies the view resolves: live descendants of open, unpaused
/// lanes that `could_step`. Closed and shelved descendants, an unparked `idea`, and all
/// work under a paused lane are never read, so a dependency only such work names (a
/// foreign record since garbled or gone) cannot fail `tasks lanes` or `prime`.
pub fn dependency_readers<'a>(
    all: &'a [Task],
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Vec<&'a Task> {
    all.iter()
        .filter(|lane| {
            lane.lane && crate::hierarchy::is_active(lane) && lane.status != Status::Blocked
        })
        .flat_map(|lane| crate::hierarchy::descendants(all, &lane.id, registry))
        .filter(|task| is_live(task) && could_step(task, claims, now))
        .collect()
}

/// Where one live descendant lands: a cause, or a step `next` would consider.
enum Class<'a> {
    Cause(Cause),
    /// A parked-agent candidate, with its park time.
    Parked(&'a str),
    /// A `ready` row.
    Ready,
}

/// The gates `next` applies before holds, in §5.1's table order, so a descendant that
/// several would stop counts under the first.
fn classify<'a>(inputs: &Inputs<'a>, task: &Task) -> Class<'a> {
    let claims = inputs.claims;
    let park = claims.park(&task.id);
    let goal = crate::hierarchy::is_goal(
        task,
        !crate::hierarchy::children(inputs.all, &task.id, inputs.registry).is_empty(),
    );
    // The session gates (`without`, `cutoff`, `halt`) stop steps, so, like `depends`, they
    // judge only work that could be a step. A sub-goal is never a step; it and every other
    // non-step fall through to their own cause.
    let gated = !goal && could_step(task, claims, inputs.now);
    let cause = if claims.live(&task.id).is_some() {
        Some(Cause::Active)
    } else if gated
        && inputs
            .without
            .hides(task, crate::needs::vocabulary_of(inputs.vocabularies, task))
    {
        Some(Cause::Without)
    } else if gated
        && inputs.cutoff.is_some_and(|cutoff| {
            crate::complexity::effective(task, claims).is_none_or(|level| level > cutoff)
        })
    {
        Some(Cause::Cutoff)
    } else if gated
        && inputs
            .halts
            .get(&task.id.prefix)
            .is_some_and(|halt| !halt.allows(task))
    {
        Some(Cause::Halt)
    } else if crate::defer::is_deferred(task, inputs.now) {
        Some(Cause::Deferred)
    } else if task.status == Status::Done && !crate::periodic::is_due(task, inputs.now) {
        Some(Cause::Periodic)
    } else if park.is_some_and(|park| park.waiting_on == WaitingOn::User) {
        Some(Cause::User)
    } else if task.status == Status::Blocked {
        Some(Cause::Blocked)
    } else if could_step(task, claims, inputs.now)
        && !task
            .depends
            .iter()
            .all(|dependency| (inputs.dependency)(dependency) == Some(true))
    {
        // Asked only of `dependency_readers` records, the ones whose dependencies were
        // resolved; a record that could never be a step falls through to `goal` or
        // `other`.
        Some(Cause::Depends)
    } else if goal {
        Some(Cause::Goal)
    } else {
        None
    };
    match (cause, park) {
        (Some(cause), _) => Class::Cause(cause),
        // `parked::candidates`: waiting on the agent and open; blocked and shelved are
        // already out above.
        (None, Some(park)) if park.waiting_on == WaitingOn::Agent && task.status.is_open() => {
            Class::Parked(park.at.as_str())
        }
        (None, _) if crate::query::is_candidate(task, inputs.now) => Class::Ready,
        // `idea`, or `doing` without a live claim.
        (None, _) => Class::Cause(Cause::Other),
    }
}

fn row(inputs: &Inputs, lane: &Task, picked: &mut BTreeMap<String, TaskId>) -> LaneRow {
    let summary = |task: &Task| {
        TaskSummary::of(
            task,
            inputs.all,
            Some(inputs.claims),
            inputs.registry,
            inputs.now,
        )
    };
    let mut live: Vec<&Task> = crate::hierarchy::descendants(inputs.all, &lane.id, inputs.registry)
        .into_iter()
        .filter(|task| is_live(task))
        .collect();
    live.sort_by(|a, b| crate::query::ready_order(a, b));
    let mut out = LaneRow {
        lane: summary(lane),
        guidance: guidance(&lane.body),
        state: LaneState::Paused,
        pick: None,
        steps: 0,
        active: live
            .iter()
            .filter(|task| inputs.claims.live(&task.id).is_some())
            .map(|task| summary(task))
            .collect(),
        held: Vec::new(),
        causes: Causes::default(),
    };
    // §5.1: a paused lane lists its active work and nothing else.
    if lane.status == Status::Blocked {
        return out;
    }
    let mut parked: Vec<(&str, &Task)> = Vec::new();
    let mut ready: Vec<&Task> = Vec::new();
    for &task in &live {
        match classify(inputs, task) {
            Class::Cause(cause) => out.causes.add(cause),
            Class::Parked(at) => parked.push((at, task)),
            Class::Ready => ready.push(task),
        }
    }
    // `next`'s order: parked candidates newest first, then ready order (`live` is sorted).
    parked.sort_by(|a, b| b.0.cmp(a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let mut pick: Option<&Task> = None;
    for step in parked.into_iter().map(|(_, task)| task).chain(ready) {
        if pick.is_some() {
            out.steps += 1;
            continue;
        }
        match held_by(inputs, step, picked) {
            Some(held) => {
                out.causes.held += 1;
                out.held.push(held);
            }
            None => pick = Some(step),
        }
    }
    if let Some(step) = pick
        && let Some(vocabulary) = inputs.vocabularies.get(step.id.prefix.as_str())
    {
        for need in crate::needs::exclusive_of(vocabulary, &step.needs) {
            picked.entry(need).or_insert_with(|| step.id.clone());
        }
    }
    out.state = match (pick, out.held.is_empty(), live.is_empty()) {
        (Some(_), _, _) => LaneState::Ready,
        (None, false, _) => LaneState::Held,
        (None, true, false) => LaneState::Waiting,
        (None, true, true) => LaneState::Empty,
    };
    out.pick = pick.map(summary);
    out
}

/// §5.1 step 3: the first exclusive need of `step` held by another session's live claim,
/// else by an earlier lane's pick. Exclusive names are matched host-wide (§4.1).
fn held_by(inputs: &Inputs, step: &Task, picked: &BTreeMap<String, TaskId>) -> Option<HeldStep> {
    let vocabulary = inputs.vocabularies.get(step.id.prefix.as_str())?;
    if let Some((need, holder)) =
        crate::holds::held_back(inputs.holds, vocabulary, step, inputs.mine)
    {
        return Some(HeldStep {
            id: step.id.clone(),
            need,
            holder: holder.task,
            by: HeldBy::Claim,
        });
    }
    crate::needs::exclusive_of(vocabulary, &step.needs)
        .into_iter()
        .find_map(|need| {
            picked.get(&need).map(|pick| HeldStep {
                id: step.id.clone(),
                holder: pick.clone(),
                need,
                by: HeldBy::Pick,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::{ClaimSnapshot, Park, WaitingOn};
    use crate::holds::HoldSnapshot;
    use crate::model::{Status, Task, TaskId};
    use crate::needs::{NeedDecl, Vocabulary, Without};
    use crate::registry::Registry;
    use std::collections::{BTreeMap, HashMap};
    use time::OffsetDateTime;

    const NOW: &str = "2026-10-03T12:00:00Z";

    fn at(stamp: &str) -> OffsetDateTime {
        crate::time::parse(stamp).unwrap()
    }

    fn task(id: &str, parent: Option<&str>, status: Status, priority: u8) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: id.into(),
            status,
            priority,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            lane: false,
            needs: vec![],
            every: None,
            defer: None,
            owner: None,
            created: "2026-09-01T00:00:00Z".into(),
            updated: "2026-09-01T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: parent.map(|parent| TaskId::parse(parent).unwrap()),
            tags: vec![],
            source: None,
            model: None,
            agent: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    fn lane(id: &str, priority: u8) -> Task {
        let mut lane = task(id, None, Status::Todo, priority);
        lane.lane = true;
        lane
    }

    fn needs_quiet(mut task: Task) -> Task {
        task.needs = vec!["quiet".into()];
        task
    }

    fn agent_park() -> Park {
        Park {
            owner: "o".into(),
            session: "s:a".into(),
            host: "h".into(),
            worktree: "/w".into(),
            at: "2026-10-01T00:00:00Z".into(),
            next_step: "n".into(),
            waiting_on: WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "T".into(),
        }
    }

    struct World {
        claims: ClaimSnapshot,
        vocabulary: Vocabulary,
        without: Without,
        holds: HoldSnapshot,
        registry: Registry,
    }

    fn world(claims: ClaimSnapshot) -> World {
        let registry = Registry::default();
        let (holds, _) = HoldSnapshot::load(&registry, at(NOW));
        World {
            claims,
            vocabulary: Vocabulary::from([(
                "quiet".to_string(),
                NeedDecl {
                    meaning: "an idle host".into(),
                    exclusive: true,
                },
            )]),
            without: Without::default(),
            holds,
            registry,
        }
    }

    fn run(all: &[Task], world: &World) -> Vec<LaneRow> {
        run_with_cutoff(all, world, None)
    }

    fn run_with_cutoff(all: &[Task], world: &World, cutoff: Option<Complexity>) -> Vec<LaneRow> {
        let dependency = |id: &TaskId| {
            all.iter()
                .find(|task| task.id == *id)
                .map(|task| !task.status.is_open())
        };
        let halts = HashMap::new();
        let vocabularies = HashMap::from([("xx", &world.vocabulary)]);
        // The fixture's own session is "me": its claims are this caller's holds.
        let mine = world.holds.mine(|claim| Ok(claim.session == "me")).unwrap();
        build(&Inputs {
            all,
            claims: &world.claims,
            registry: &world.registry,
            dependency: &dependency,
            halts: &halts,
            cutoff,
            without: &world.without,
            holds: &world.holds,
            vocabularies: &vocabularies,
            mine: &mine,
            now: at(NOW),
        })
    }

    fn pick(row: &LaneRow) -> &str {
        row.pick
            .as_ref()
            .map(|pick| pick.id.as_str())
            .unwrap_or("-")
    }

    #[test]
    fn guidance_is_the_first_paragraph_after_blank_and_heading_lines() {
        assert_eq!(
            guidance(
                "\n# Captures\n\n## Why\n\nShip the captures.\nFirst milestone: one run.\n\nMore."
            )
            .as_deref(),
            Some("Ship the captures. First milestone: one run.")
        );
        assert_eq!(guidance("# Only a heading"), None);
        assert_eq!(guidance("## Heading\n\n### Another"), None);
        assert_eq!(guidance(""), None);
        assert_eq!(
            guidance("#hashtag opens the paragraph").as_deref(),
            Some("#hashtag opens the paragraph"),
            "a heading needs a space after its hashes"
        );
    }

    #[test]
    fn lanes_come_in_ready_order_and_a_blocked_one_is_paused() {
        let mut paused = lane("xx-0000a1", 0);
        paused.status = Status::Blocked;
        let mut shelved = lane("xx-0000a3", 0);
        shelved.status = Status::Shelved;
        let mut closed = lane("xx-0000a4", 0);
        closed.status = Status::Done;
        let all = vec![
            paused,
            task("xx-000001", Some("xx-0000a1"), Status::Todo, 0),
            lane("xx-0000a2", 2),
            shelved,
            closed,
        ];
        let rows = run(&all, &world(ClaimSnapshot::default()));
        let ids: Vec<&str> = rows.iter().map(|row| row.lane.id.as_str()).collect();
        assert_eq!(
            ids,
            ["xx-0000a1", "xx-0000a2"],
            "shelved and closed lanes are absent"
        );
        assert_eq!(rows[0].state, LaneState::Paused);
        assert!(rows[0].pick.is_none() && rows[0].steps == 0 && rows[0].causes.is_empty());
        assert_eq!(rows[1].state, LaneState::Empty);
    }

    #[test]
    fn a_parked_step_is_picked_before_ready_order_as_next_does() {
        let all = vec![
            lane("xx-0000a1", 0),
            task("xx-000001", Some("xx-0000a1"), Status::Todo, 0),
            task("xx-000002", Some("xx-0000a1"), Status::Todo, 3),
        ];
        let claims = ClaimSnapshot::from_parts(
            BTreeMap::new(),
            BTreeMap::from([("xx-000002".to_string(), agent_park())]),
            BTreeMap::new(),
        );
        let rows = run(&all, &world(claims));
        assert_eq!(pick(&rows[0]), "xx-000002");
        assert_eq!(rows[0].steps, 1);
    }

    #[test]
    fn a_sub_goal_counts_as_goal_and_its_children_are_steps() {
        let all = vec![
            lane("xx-0000a1", 0),
            task("xx-0000b1", Some("xx-0000a1"), Status::Todo, 0),
            task("xx-000001", Some("xx-0000b1"), Status::Todo, 2),
        ];
        let rows = run(&all, &world(ClaimSnapshot::default()));
        assert_eq!(rows[0].state, LaneState::Ready);
        assert_eq!(
            pick(&rows[0]),
            "xx-000001",
            "picked from inside the sub-goal"
        );
        assert_eq!(rows[0].causes.entries(), vec![("goal", 1)]);
    }

    #[test]
    fn the_partition_counts_every_live_descendant_once() {
        let mut deferred_and_blocked = task("xx-000006", Some("xx-0000a1"), Status::Blocked, 2);
        deferred_and_blocked.defer = Some(crate::defer::Defer::parse("2099-01-01").unwrap());
        let mut depends = task("xx-000007", Some("xx-0000a1"), Status::Todo, 2);
        depends.depends = vec![TaskId::parse("xx-000005").unwrap()];
        let all = vec![
            lane("xx-0000a1", 1),
            task("xx-000001", Some("xx-0000a1"), Status::Todo, 0),
            task("xx-000002", Some("xx-0000a1"), Status::Todo, 1),
            task("xx-000003", Some("xx-0000a1"), Status::Idea, 2),
            task("xx-000004", Some("xx-0000a1"), Status::Doing, 2),
            task("xx-000005", Some("xx-0000a1"), Status::Blocked, 2),
            deferred_and_blocked,
            depends,
            task("xx-000008", Some("xx-0000a1"), Status::Done, 2),
            task("xx-000009", Some("xx-0000a1"), Status::Dropped, 2),
            task("xx-00000a", Some("xx-0000a1"), Status::Shelved, 2),
        ];
        let rows = run(&all, &world(ClaimSnapshot::default()));
        let row = &rows[0];
        assert_eq!(pick(row), "xx-000001");
        assert_eq!(row.steps, 1);
        assert_eq!(
            row.causes.entries(),
            vec![
                ("deferred", 1),
                ("blocked", 1),
                ("depends", 1),
                ("other", 2)
            ],
            "deferred and blocked counts once, under the earlier cause"
        );
        let counted: usize =
            1 + row.steps + row.causes.entries().iter().map(|(_, n)| n).sum::<usize>();
        assert_eq!(counted, 7, "seven live descendants, each once");
    }

    #[test]
    fn session_gates_count_only_work_that_could_be_a_step() {
        let rated = |mut task: Task| {
            task.complexity = Some(Complexity::Low);
            task
        };
        let mut deferred = rated(task("xx-000004", Some("xx-0000b1"), Status::Todo, 2));
        deferred.defer = Some(crate::defer::Defer::parse("2099-01-01").unwrap());
        let all = vec![
            lane("xx-0000a1", 0),
            task("xx-000001", Some("xx-0000a1"), Status::Idea, 2),
            rated(task("xx-000002", Some("xx-0000a1"), Status::Blocked, 2)),
            task("xx-0000b1", Some("xx-0000a1"), Status::Todo, 2),
            deferred,
            task("xx-000005", Some("xx-0000a1"), Status::Todo, 2),
        ];
        let world = world(ClaimSnapshot::default());

        let open = run(&all, &world);
        assert_eq!(pick(&open[0]), "xx-000005");
        assert_eq!(
            open[0].causes.entries(),
            vec![("deferred", 1), ("blocked", 1), ("goal", 1), ("other", 1)]
        );

        let cut = run_with_cutoff(&all, &world, Some(Complexity::Mid));
        let row = &cut[0];
        assert_eq!(row.state, LaneState::Waiting);
        assert_eq!(
            row.causes.entries(),
            vec![
                ("cutoff", 1),
                ("deferred", 1),
                ("blocked", 1),
                ("goal", 1),
                ("other", 1)
            ],
            "only the unrated step counts under cutoff; the unrated idea and sub-goal keep \
             their own causes"
        );
        let counted: usize = row.pick.iter().count()
            + row.steps
            + row.causes.entries().iter().map(|(_, n)| n).sum::<usize>();
        assert_eq!(counted, 5, "five live descendants, each once");
    }

    #[test]
    fn recurrences_count_while_live_and_a_dropped_one_does_not() {
        let every = crate::periodic::Interval::parse("30d").unwrap();
        let mut due = task("xx-000001", Some("xx-0000a1"), Status::Done, 2);
        due.every = Some(every);
        let mut soon = task("xx-000002", Some("xx-0000a2"), Status::Done, 2);
        soon.every = Some(every);
        soon.last_done = Some("2026-10-02T00:00:00Z".into());
        let mut gone = task("xx-000003", Some("xx-0000a3"), Status::Dropped, 2);
        gone.every = Some(every);
        let all = vec![
            lane("xx-0000a1", 0),
            due,
            lane("xx-0000a2", 1),
            soon,
            lane("xx-0000a3", 2),
            gone,
        ];
        let rows = run(&all, &world(ClaimSnapshot::default()));
        assert_eq!(rows[0].state, LaneState::Ready);
        assert_eq!(pick(&rows[0]), "xx-000001");
        assert_eq!(rows[1].state, LaneState::Waiting);
        assert_eq!(rows[1].causes.entries(), vec![("periodic", 1)]);
        assert_eq!(rows[2].state, LaneState::Empty);
    }

    #[test]
    fn an_earlier_lane_pick_holds_its_exclusive_need_for_later_lanes() {
        let mut all = vec![
            lane("xx-0000a1", 0),
            needs_quiet(task("xx-000001", Some("xx-0000a1"), Status::Todo, 2)),
            lane("xx-0000a2", 1),
            needs_quiet(task("xx-000002", Some("xx-0000a2"), Status::Todo, 0)),
            task("xx-000003", Some("xx-0000a2"), Status::Todo, 1),
        ];
        let world = world(ClaimSnapshot::default());
        let rows = run(&all, &world);
        assert_eq!(pick(&rows[0]), "xx-000001");
        assert_eq!(pick(&rows[1]), "xx-000003");
        let held = &rows[1].held[0];
        assert_eq!(
            (
                held.id.to_string(),
                held.need.as_str(),
                held.holder.to_string(),
                held.by
            ),
            (
                "xx-000002".to_string(),
                "quiet",
                "xx-000001".to_string(),
                HeldBy::Pick
            )
        );
        assert_eq!(rows[1].causes.entries(), vec![("held", 1)]);
        assert_eq!(rows[1].steps, 0, "a held step counts under held, not steps");

        all.pop();
        let rows = run(&all, &world);
        assert_eq!(rows[1].state, LaneState::Held);
        assert!(rows[1].pick.is_none());
    }

    #[test]
    fn dependencies_are_read_only_for_work_that_could_be_a_step() {
        let mut paused = lane("xx-0000a2", 1);
        paused.status = Status::Blocked;
        let mut due = task("xx-000005", Some("xx-0000a1"), Status::Done, 2);
        due.every = Some(crate::periodic::Interval::parse("30d").unwrap());
        let all = vec![
            lane("xx-0000a1", 0),
            task("xx-000001", Some("xx-0000a1"), Status::Todo, 2),
            task("xx-000002", Some("xx-0000a1"), Status::Done, 2),
            task("xx-000003", Some("xx-0000a1"), Status::Shelved, 2),
            task("xx-000004", Some("xx-0000a1"), Status::Idea, 2),
            due,
            task("xx-000006", Some("xx-0000a1"), Status::Idea, 2),
            paused,
            task("xx-000007", Some("xx-0000a2"), Status::Todo, 2),
            task("xx-000008", None, Status::Todo, 2),
        ];
        let claims = ClaimSnapshot::from_parts(
            BTreeMap::new(),
            BTreeMap::from([("xx-000006".to_string(), agent_park())]),
            BTreeMap::new(),
        );
        let mut readers: Vec<String> =
            dependency_readers(&all, &claims, &Registry::default(), at(NOW))
                .iter()
                .map(|task| task.id.to_string())
                .collect();
        readers.sort();
        assert_eq!(
            readers,
            ["xx-000001", "xx-000005", "xx-000006"],
            "a todo, a due recurrence, and an agent-parked idea; never closed, shelved, \
             an unparked idea, a paused lane's work, or work outside every lane"
        );
    }
}
