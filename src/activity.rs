//! Direct and 1-hop neighborhood activity over a captured task scan.
//!
//! Direct activity weights frontmatter lifecycle timestamps and authored work notes in fixed
//! UTC calendar-date buckets (`t_e.date()`) with a 5-day exponential half-life. Neighborhood
//! activity spreads direct heat one hop across move-resolved `parent`/children/siblings,
//! `depends`/dependents, `wake_on`, and `related` links in `O(N + E)` time.

use std::collections::{HashMap, HashSet};
use time::{Date, OffsetDateTime};

#[cfg(test)]
use crate::model::Note;
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use crate::style::Activity;

/// Activity half-life in days (`2^(-age_days / HALF_LIFE_DAYS)`).
const HALF_LIFE_DAYS: f64 = 5.0;
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Combined direct and 1-hop neighborhood activity for every task in a captured scan,
/// keyed by canonical `TaskId`.
#[derive(Debug, Clone, Default)]
pub struct ActivityIndex {
    scores: HashMap<TaskId, Activity>,
}

impl ActivityIndex {
    /// Computes direct and 1-hop neighborhood activity for `all` at `now` in `O(N + E)` time.
    pub fn of_scan(all: &[Task], registry: &Registry, now: OffsetDateTime) -> Self {
        let n = all.len();
        let mut by_canonical_id = HashMap::with_capacity(n);
        let mut direct = Vec::with_capacity(n);
        for (i, task) in all.iter().enumerate() {
            by_canonical_id.insert(registry.canonical_id(&task.id), i);
            direct.push(direct_score(task, now));
        }

        let resolved_target = precompute_move_targets(all, registry, &by_canonical_id);
        let resolve_idx = |id: &TaskId| -> Option<usize> {
            by_canonical_id
                .get(&registry.canonical_id(id))
                .and_then(|&idx| resolved_target[idx])
        };

        let mut parent_of: Vec<Option<usize>> = vec![None; n];
        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut depends_of: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut dependents_of: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut related_of: Vec<Vec<usize>> = vec![Vec::new(); n];

        let mut dependency_edges = HashSet::new();
        let mut related_edges = HashSet::new();

        for (u, task) in all.iter().enumerate() {
            if task.status == Status::Moved {
                continue;
            }
            if let Some(parent_id) = &task.parent
                && let Some(p) = resolve_idx(parent_id)
                && p != u
            {
                parent_of[u] = Some(p);
                children_of[p].push(u);
            }
            for dep_id in &task.depends {
                if let Some(d) = resolve_idx(dep_id)
                    && d != u
                    && dependency_edges.insert((u, d))
                {
                    depends_of[u].push(d);
                    dependents_of[d].push(u);
                }
            }
            for link_id in task.related.iter().chain(task.wake_on.iter()) {
                if let Some(r) = resolve_idx(link_id)
                    && r != u
                    && related_edges.insert((u.min(r), u.max(r)))
                {
                    related_of[u].push(r);
                    related_of[r].push(u);
                }
            }
        }

        let mut parent_top4: Vec<Top4> = vec![Top4::default(); n];
        for p in 0..n {
            if !all[p].lane && !children_of[p].is_empty() {
                let mut top4 = Top4::default();
                for &c in &children_of[p] {
                    top4.push(c, direct[c]);
                }
                parent_top4[p] = top4;
            }
        }

        let mut scores = HashMap::with_capacity(n);
        for (u, task) in all.iter().enumerate() {
            let canonical = registry.canonical_id(&task.id);
            if task.status == Status::Moved {
                scores.insert(canonical, Activity::COLD);
                continue;
            }
            let mut top3 = Top3::default();

            if let Some(p) = parent_of[u]
                && !all[p].lane
            {
                top3.push(0.45 * direct[p]);
                let sib_scale = (children_of[p].len().saturating_sub(1).max(1) as f64).sqrt();
                let mut taken = 0;
                for &(s, s_direct) in &parent_top4[p].0 {
                    if s_direct <= 0.0 {
                        break;
                    }
                    if s == u {
                        continue;
                    }
                    top3.push(0.45 * s_direct / sib_scale);
                    taken += 1;
                    if taken == 3 {
                        break;
                    }
                }
            }

            if !children_of[u].is_empty() {
                let mut ch_top3 = Top3::default();
                for &c in &children_of[u] {
                    ch_top3.push(direct[c]);
                }
                let [x1, x2, x3] = ch_top3.0;
                let c_children = if !task.lane {
                    (0.75 * x1 + 0.15 * (x2 + x3)).min(0.85)
                } else {
                    let scale = (children_of[u].len() as f64).sqrt();
                    ((0.75 * x1 + 0.25 * (x2 + x3)) / scale).min(0.85)
                };
                top3.push(c_children);
            }

            for &d in &depends_of[u] {
                top3.push(0.55 * direct[d]);
            }
            if !dependents_of[u].is_empty() {
                let scale = (dependents_of[u].len() as f64).sqrt();
                for &v in &dependents_of[u] {
                    top3.push(0.55 * direct[v] / scale);
                }
            }
            if !related_of[u].is_empty() {
                let scale = (related_of[u].len() as f64).sqrt();
                for &r in &related_of[u] {
                    top3.push(0.35 * direct[r] / scale);
                }
            }

            let [y1, y2, y3] = top3.0;
            let a_neighbor = (y1 + 0.25 * (y2 + y3)).min(0.75);
            let a = direct[u].max(a_neighbor);
            scores.insert(canonical, Activity::from_unit(a));
        }

        Self { scores }
    }

    /// Computes activity for an additional checkout's scan (`prime` `claimed_elsewhere`)
    /// and inserts any canonical task IDs not already present in `self`.
    pub fn extend_scan(&mut self, all: &[Task], registry: &Registry, now: OffsetDateTime) {
        let other = Self::of_scan(all, registry, now);
        for (id, activity) in other.scores {
            self.scores.entry(id).or_insert(activity);
        }
    }

    pub fn get(&self, id: &TaskId) -> Option<Activity> {
        self.scores.get(id).copied()
    }

    pub fn get_id(&self, id: &str) -> Option<Activity> {
        TaskId::parse(id).ok().and_then(|parsed| self.get(&parsed))
    }
}

/// Direct activity `a_direct(task, now)` in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, Default)]
struct DayBucket {
    authored_and_lifecycle_weight: f64,
    transition_weight: f64,
    newest: Option<OffsetDateTime>,
}

impl DayBucket {
    fn add(&mut self, at: OffsetDateTime, weight: f64, is_transition: bool) {
        if weight <= 0.0 {
            return;
        }
        if is_transition {
            self.transition_weight = (self.transition_weight + weight).min(0.15);
        } else {
            self.authored_and_lifecycle_weight += weight;
        }
        self.newest = Some(match self.newest {
            Some(prev) if prev >= at => prev,
            _ => at,
        });
    }

    fn total_weight(&self) -> f64 {
        self.authored_and_lifecycle_weight + self.transition_weight
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteKind {
    Zero,
    Transition,
    Authored,
}

fn classify_note(text: &str) -> NoteKind {
    if text.starts_with("origin backfill")
        || text.starts_with("retained: ")
        || text.starts_with("retention operation: ")
        || text.starts_with("retention checkpoint: ")
        || text.starts_with("archived-notes: ")
        || text.starts_with("moved from ")
        || text.starts_with("moved to ")
        || text.starts_with("curate:")
        || text == "created"
        || text == "started"
        || text == "done"
        || text == "dropped"
        || text
            .strip_prefix("completed; next due ")
            .is_some_and(|day| crate::time::calendar_day(day).is_ok())
        || crate::attachments::parse_ledger_note(text).is_some()
        || crate::multipart::declaration::parse_declaration_note(text).is_some()
    {
        return NoteKind::Zero;
    }
    if text == "resumed"
        || text == "unshelved"
        || text.starts_with("parked (waiting on ")
        || text.starts_with("shelved: ")
        || text.starts_with("released (")
        || text.starts_with("takeover: ")
        || text.starts_with("halt override: ")
        || text.starts_with("need override: ")
    {
        return NoteKind::Transition;
    }
    NoteKind::Authored
}

/// Direct activity `a_direct(task, now)` in `[0.0, 1.0]`.
pub fn direct_score(task: &Task, now: OffsetDateTime) -> f64 {
    if task.status == Status::Moved {
        return 0.0;
    }
    let mut dates = HashMap::<Date, usize>::new();
    let mut buckets = Vec::<DayBucket>::new();
    let mut record_event = |stamp: &str, weight: f64, is_transition: bool| {
        let at = crate::time::parse(stamp).expect("record timestamps are validated at parse");
        let next = buckets.len();
        let idx = *dates.entry(at.date()).or_insert(next);
        if idx == buckets.len() {
            buckets.push(DayBucket::default());
        }
        buckets[idx].add(at, weight, is_transition);
    };

    record_event(&task.created, 1.0, false);
    for stamp in [
        task.started.as_deref(),
        task.completed.as_deref(),
        task.closed.as_deref(),
        task.last_done.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        record_event(stamp, 1.0, false);
    }
    if let Some(stamp) = task.reviewed_at.as_deref() {
        record_event(stamp, 0.6, false);
    }

    for note in &task.notes {
        let text = &note.text;
        let timestamp = &note.at;
        match classify_note(text) {
            NoteKind::Zero => {}
            NoteKind::Transition => record_event(timestamp, 0.15, true),
            NoteKind::Authored => record_event(timestamp, 0.35, false),
        }
    }

    let mut raw = 0.0;
    for bucket in &buckets {
        let w = bucket.total_weight();
        let Some(newest) = bucket.newest else {
            continue;
        };
        if w <= 0.0 {
            continue;
        }
        let age_days = ((now - newest).as_seconds_f64() / SECONDS_PER_DAY).max(0.0);
        let decay = (-age_days / HALF_LIFE_DAYS).exp2();
        raw += (1.0 + w).log2() * decay;
    }
    ((1.0 + raw.min(1.0)).ln() / std::f64::consts::LN_2).min(1.0)
}

#[derive(Debug, Clone, Copy, Default)]
struct Top3([f64; 3]);

impl Top3 {
    fn push(&mut self, x: f64) {
        if x > self.0[0] {
            self.0[2] = self.0[1];
            self.0[1] = self.0[0];
            self.0[0] = x;
        } else if x > self.0[1] {
            self.0[2] = self.0[1];
            self.0[1] = x;
        } else if x > self.0[2] {
            self.0[2] = x;
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Top4([(usize, f64); 4]);

impl Top4 {
    fn push(&mut self, idx: usize, score: f64) {
        if score > self.0[0].1 {
            self.0[3] = self.0[2];
            self.0[2] = self.0[1];
            self.0[1] = self.0[0];
            self.0[0] = (idx, score);
        } else if score > self.0[1].1 {
            self.0[3] = self.0[2];
            self.0[2] = self.0[1];
            self.0[1] = (idx, score);
        } else if score > self.0[2].1 {
            self.0[3] = self.0[2];
            self.0[2] = (idx, score);
        } else if score > self.0[3].1 {
            self.0[3] = (idx, score);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveState {
    Unvisited,
    Visiting,
    Resolved(Option<usize>),
}

fn precompute_move_targets(
    all: &[Task],
    registry: &Registry,
    by_canonical_id: &HashMap<TaskId, usize>,
) -> Vec<Option<usize>> {
    let n = all.len();
    let mut state = vec![MoveState::Unvisited; n];
    let mut stack: Vec<usize> = Vec::new();

    for start in 0..n {
        if state[start] != MoveState::Unvisited {
            continue;
        }
        stack.clear();
        let mut curr = Some(start);
        let outcome = loop {
            let Some(i) = curr else {
                break None;
            };
            match state[i] {
                MoveState::Resolved(target) => break target,
                MoveState::Visiting => break None,
                MoveState::Unvisited => {
                    if all[i].status != Status::Moved {
                        state[i] = MoveState::Resolved(Some(i));
                        break Some(i);
                    }
                    state[i] = MoveState::Visiting;
                    stack.push(i);
                    curr = all[i].moved_to.as_ref().and_then(|next| {
                        by_canonical_id.get(&registry.canonical_id(next)).copied()
                    });
                }
            }
        };
        for &visited in &stack {
            state[visited] = MoveState::Resolved(outcome);
        }
    }

    state
        .into_iter()
        .map(|s| match s {
            MoveState::Resolved(target) => target,
            MoveState::Unvisited | MoveState::Visiting => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::fixture;

    fn parse_time(s: &str) -> OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    fn make_task(id: &str, created: &str, updated: &str) -> Task {
        fixture(
            &format!(
                "---\nid: {id}\ntitle: Task {id}\nstatus: todo\npriority: 2\ncreated: {created}\nupdated: {updated}\ndepends: []\ntags: []\n---\n"
            ),
            "test",
        )
        .unwrap()
    }

    #[test]
    fn direct_score_is_monotonically_non_increasing_across_day_and_midnight_boundaries() {
        // Case 1: two authored work notes 12h apart on the same UTC date (02:00Z and 14:00Z).
        let mut same_day = make_task("sci-000001", "2026-01-01T00:00:00Z", "2026-11-01T14:00:00Z");
        same_day.notes = vec![
            Note {
                by: "test".into(),
                provenance: None,
                occurrence: Default::default(),
                at: "2026-11-01T02:00:00Z".into(),
                text: "investigated root cause in parser".into(),
            },
            Note {
                by: "test".into(),
                provenance: None,
                occurrence: Default::default(),
                at: "2026-11-01T14:00:00Z".into(),
                text: "drafted fix and verified unit tests".into(),
            },
        ];

        // Case 2: two authored work notes 12h apart straddling UTC midnight.
        let mut cross_midnight =
            make_task("sci-000002", "2026-01-01T00:00:00Z", "2026-11-02T06:00:00Z");
        cross_midnight.notes = vec![
            Note {
                by: "test".into(),
                provenance: None,
                occurrence: Default::default(),
                at: "2026-11-01T18:00:00Z".into(),
                text: "evening progress note".into(),
            },
            Note {
                by: "test".into(),
                provenance: None,
                occurrence: Default::default(),
                at: "2026-11-02T06:00:00Z".into(),
                text: "morning progress note".into(),
            },
        ];

        let start = parse_time("2026-11-02T06:00:00Z");
        for task in [&same_day, &cross_midnight] {
            let mut prev = direct_score(task, start);
            assert!(prev > 0.4, "initial score should be warm, got {prev}");
            for hour in 1..=72 {
                let now = start + time::Duration::hours(hour);
                let curr = direct_score(task, now);
                assert!(
                    curr <= prev + 1e-12,
                    "direct_score increased at hour {hour} for {}: {prev} -> {curr}",
                    task.id
                );
                prev = curr;
            }
        }
    }

    #[test]
    fn bookkeeping_notes_have_zero_weight_and_repeated_heartbeats_are_capped() {
        let now = parse_time("2026-11-08T12:00:00Z");
        let base = make_task("sci-000001", "2026-06-01T00:00:00Z", "2026-11-08T12:00:00Z");
        let base_score = direct_score(&base, now);
        assert!(
            base_score < 0.01,
            "90+ day old task should be cold: {base_score}"
        );

        let mut bookkeeping = base.clone();
        bookkeeping.notes = vec![
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:00:00Z".into(),
                text: "origin backfill".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:05:00Z".into(),
                text: "retained: keep worktree".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:10:00Z".into(),
                text: "retention operation: verified".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:15:00Z".into(),
                text: "retention checkpoint: 13667af4".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:20:00Z".into(),
                text: "archived-notes: 12 notes".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:25:00Z".into(),
                text: "moved from old-000001".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:30:00Z".into(),
                text: "moved to new-000001".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:35:00Z".into(),
                text: "curate: checked scope".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:40:00Z".into(),
                text: "attached: report.txt (120 bytes): log".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:45:00Z".into(),
                text: "detached: report.txt: removed".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:50:00Z".into(),
                text: "multipart: archive.json (1024 bytes, 2 parts, sha256 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef, catalog sha256 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef)".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:52:00Z".into(),
                text: "created".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:54:00Z".into(),
                text: "started".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:56:00Z".into(),
                text: "done".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:57:00Z".into(),
                text: "dropped".into(),
            },
            Note {
                by: "test".into(), provenance: None, occurrence: Default::default(),
                at: "2026-11-08T11:58:00Z".into(),
                text: "completed; next due 2026-12-08".into(),
            },
        ];
        assert!(
            (direct_score(&bookkeeping, now) - base_score).abs() < 1e-12,
            "generated bookkeeping and duplicate lifecycle notes must add zero weight"
        );

        // 10 repeated transition/heartbeat notes on the same UTC date must cap at W_d = 0.15.
        let mut heartbeats = base.clone();
        for minute in 0..10 {
            heartbeats.notes.push(Note {
                by: "test".into(),
                provenance: None,
                occurrence: Default::default(),
                at: format!("2026-11-08T11:{minute:02}:00Z"),
                text: if minute % 2 == 0 {
                    "parked (waiting on agent, quiet): waiting for idle host".into()
                } else {
                    "resumed".into()
                },
            });
        }
        let hb_score = direct_score(&heartbeats, now);
        assert!(
            hb_score < 0.27 && Activity::from_unit(hb_score).0 <= 69,
            "10 same-day heartbeats must stay capped below 0.27 (<= 69/255), got {hb_score}"
        );
    }

    #[test]
    fn move_chains_aliases_cycles_and_missing_targets_resolve_safely() {
        let now = parse_time("2026-11-08T12:00:00Z");
        let mut registry = Registry::default();
        let dir = tempfile::tempdir().unwrap();
        registry.register("sci", dir.path()).unwrap();
        registry.aliases.insert("old".into(), "sci".into());

        // Multi-hop chain: old-000001 (via alias -> sci-000001) -> sci-000002 -> sci-000003 (hot)
        let mut hop1 = make_task("sci-000001", "2026-06-01T00:00:00Z", "2026-11-08T12:00:00Z");
        hop1.status = Status::Moved;
        hop1.moved_to = Some(TaskId::parse("sci-000002").unwrap());

        let mut hop2 = make_task("sci-000002", "2026-06-01T00:00:00Z", "2026-11-08T12:00:00Z");
        hop2.status = Status::Moved;
        hop2.moved_to = Some(TaskId::parse("sci-000003").unwrap());

        let hot_dest = make_task("sci-000003", "2026-11-08T12:00:00Z", "2026-11-08T12:00:00Z");

        // Task depending on retired alias `old-000001` should receive prerequisite heat from `sci-000003`.
        let mut dependent = make_task("sci-000004", "2026-06-01T00:00:00Z", "2026-06-01T00:00:00Z");
        dependent.depends = vec![TaskId::parse("old-000001").unwrap()];

        // Tombstone pointing to an out-of-scan target, even with fresh timestamps today, transfers 0 heat.
        let mut dangling_tomb =
            make_task("sci-000005", "2026-11-08T12:00:00Z", "2026-11-08T12:00:00Z");
        dangling_tomb.status = Status::Moved;
        dangling_tomb.moved_to = Some(TaskId::parse("ext-999999").unwrap());

        let mut links_dangling =
            make_task("sci-000006", "2026-06-01T00:00:00Z", "2026-06-01T00:00:00Z");
        links_dangling.depends = vec![TaskId::parse("sci-000005").unwrap()];

        // Cyclic move chain: sci-000007 <-> sci-000008.
        let mut cyc1 = make_task("sci-000007", "2026-11-08T12:00:00Z", "2026-11-08T12:00:00Z");
        cyc1.status = Status::Moved;
        cyc1.moved_to = Some(TaskId::parse("sci-000008").unwrap());
        let mut cyc2 = make_task("sci-000008", "2026-11-08T12:00:00Z", "2026-11-08T12:00:00Z");
        cyc2.status = Status::Moved;
        cyc2.moved_to = Some(TaskId::parse("sci-000007").unwrap());

        let mut links_cycle =
            make_task("sci-000009", "2026-06-01T00:00:00Z", "2026-06-01T00:00:00Z");
        links_cycle.depends = vec![TaskId::parse("sci-000007").unwrap()];

        let scan = vec![
            hop1,
            hop2,
            hot_dest,
            dependent,
            dangling_tomb,
            links_dangling,
            cyc1,
            cyc2,
            links_cycle,
        ];
        let index = ActivityIndex::of_scan(&scan, &registry, now);

        // Tombstones themselves are always COLD (0).
        assert_eq!(index.get_id("sci-000001"), Some(Activity::COLD));
        assert_eq!(index.get_id("sci-000002"), Some(Activity::COLD));
        assert_eq!(index.get_id("sci-000005"), Some(Activity::COLD));
        // Live destination is HOT (255).
        assert_eq!(index.get_id("sci-000003"), Some(Activity::HOT));
        // Dependent of multi-hop moved task receives 0.55 * 1.0 = 0.55 -> round(0.55 * 255) = 140.
        assert_eq!(index.get_id("sci-000004"), Some(Activity(140)));
        // Links to out-of-scan tombstone and cycle receive 0 neighborhood heat.
        assert_eq!(index.get_id("sci-000006"), Some(Activity::COLD));
        assert_eq!(index.get_id("sci-000009"), Some(Activity::COLD));
    }

    #[test]
    fn high_degree_goal_and_lane_propagate_expected_sibling_and_child_heat() {
        let now = parse_time("2026-11-08T12:00:00Z");
        let registry = Registry::default();

        // Non-lane goal with 20 children: 4 hot children (created today), 16 cold children.
        let goal = make_task("sci-000100", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
        let mut scan = vec![goal];
        for i in 1..=20 {
            let stamp = if i <= 4 {
                "2026-11-08T12:00:00Z"
            } else {
                "2026-01-01T00:00:00Z"
            };
            let mut child = make_task(&format!("sci-{i:06}"), stamp, stamp);
            child.parent = Some(TaskId::parse("sci-000100").unwrap());
            scan.push(child);
        }

        // Lane with 20 children: 1 hot child, 19 cold children.
        let mut lane = make_task("sci-000200", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
        lane.lane = true;
        scan.push(lane);
        for i in 21..=40 {
            let stamp = if i == 21 {
                "2026-11-08T12:00:00Z"
            } else {
                "2026-01-01T00:00:00Z"
            };
            let mut child = make_task(&format!("sci-{i:06}"), stamp, stamp);
            child.parent = Some(TaskId::parse("sci-000200").unwrap());
            scan.push(child);
        }

        let index = ActivityIndex::of_scan(&scan, &registry, now);

        // Non-lane goal receives min(0.85, 0.75*1 + 0.15*(1 + 1)) = 0.85 -> min(0.75, 0.85) = 0.75 (191).
        assert_eq!(index.get_id("sci-000100"), Some(Activity::from_unit(0.75)));

        // Cold child of non-lane goal has 19 siblings (3 of which are hot with a_direct = 1.0):
        // each of top 3 siblings contributes 0.45 / sqrt(19), combined = (0.45 / sqrt(19)) * 1.5.
        let expected_sib = (0.45 / 19.0_f64.sqrt()) * 1.5;
        assert_eq!(
            index.get_id("sci-000020"),
            Some(Activity::from_unit(expected_sib))
        );

        // Cold child of lane (`lane: true`) receives NO sibling or parent heat from the lane!
        assert_eq!(index.get_id("sci-000040"), Some(Activity::COLD));

        // The lane itself receives degree-damped child heat: 0.75 * 1.0 / sqrt(20).
        let expected_lane = 0.75 / 20.0_f64.sqrt();
        assert_eq!(
            index.get_id("sci-000200"),
            Some(Activity::from_unit(expected_lane))
        );
    }

    #[test]
    fn clock_skew_clamps_future_timestamps_and_extend_scan_preserves_primary() {
        let now = parse_time("2026-11-08T12:00:00Z");
        let registry = Registry::default();
        let future = make_task("sci-000001", "2026-11-09T12:00:00Z", "2026-11-09T12:00:00Z");
        assert!((direct_score(&future, now) - 1.0).abs() < 1e-12);

        let cold = make_task("sci-000001", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z");
        let extra = make_task("sci-000002", "2026-11-08T12:00:00Z", "2026-11-08T12:00:00Z");
        let mut index = ActivityIndex::of_scan(&[cold], &registry, now);
        index.extend_scan(&[future, extra], &registry, now);
        assert_eq!(index.get_id("sci-000001"), Some(Activity::COLD));
        assert_eq!(index.get_id("sci-000002"), Some(Activity::HOT));
    }
}
