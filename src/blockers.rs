//! Downstream impact of unmet prerequisites over one captured scan: the graph, the counts,
//! the rank and an honest account of what the scan could not see. Pure: no I/O beyond the
//! caller's lookup and no host-local evidence. See
//! docs/specs/2026-10-25-blocker-ranking-design.md (graph and counts, honest coverage).

use crate::dependency::{Verdict, verdict};
use crate::error::Result;
use crate::model::{Status, Task, TaskId};
use crate::registry::Registry;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Which dependents a blockers report counted (blocker-ranking spec, scope and snapshot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageMode {
    Local,
    Project,
    Group,
    AllProjects,
}

/// What a blockers scan saw and could not see. `complete` means every requested project
/// was scanned, every examined reference resolved, and no boundary path was cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coverage {
    pub mode: CoverageMode,
    pub requested_projects: Vec<String>,
    pub scanned_projects: Vec<String>,
    pub unreachable_projects: Vec<String>,
    pub boundary_targets: Vec<TaskId>,
    pub unfollowed_targets: Vec<TaskId>,
    pub complete: bool,
}

impl Coverage {
    /// What opening the scope established; the analysis derives the targets and the final
    /// verdict from the references it examines.
    pub fn new(
        mode: CoverageMode,
        requested_projects: Vec<String>,
        scanned_projects: Vec<String>,
        unreachable_projects: Vec<String>,
    ) -> Coverage {
        let complete = unreachable_projects.is_empty();
        Coverage {
            mode,
            requested_projects,
            scanned_projects,
            unreachable_projects,
            boundary_targets: Vec::new(),
            unfollowed_targets: Vec::new(),
            complete,
        }
    }
}

/// Affected work by status, each open status present even at zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct WorkByStatus {
    pub idea: usize,
    pub todo: usize,
    pub doing: usize,
    pub blocked: usize,
    pub shelved: usize,
}

impl WorkByStatus {
    pub fn add(&mut self, status: Status) {
        match status {
            Status::Idea => self.idea += 1,
            Status::Todo => self.todo += 1,
            Status::Doing => self.doing += 1,
            Status::Blocked => self.blocked += 1,
            Status::Shelved => self.shelved += 1,
            Status::Done | Status::Dropped | Status::Moved => {
                unreachable!("only open records are affected work")
            }
        }
    }
}

/// A stored dependency no record answers for: missing, unregistered or a move loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnresolvedDependency {
    pub dependent: TaskId,
    pub reference: TaskId,
    /// The live spelling of `reference` when an alias renames it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<TaskId>,
    pub reason: &'static str,
}

/// The ranked rows of one scan, with what the scan could not see.
#[derive(Debug)]
pub struct Analysis {
    pub rows: Vec<Impact>,
    pub coverage: Coverage,
    pub unresolved: Vec<UnresolvedDependency>,
    pub cycles: Vec<Vec<TaskId>>,
}

/// One open prerequisite and what it holds up. No park, claim or authorization verdict.
#[derive(Debug)]
pub struct Impact {
    /// The terminal record, its id canonical.
    pub task: Task,
    /// `None` for an out-of-scope target that is not a lane: its children were not scanned.
    pub goal: Option<bool>,
    pub cycle: bool,
    pub direct_work_count: usize,
    pub transitive_work_count: usize,
    pub direct_goal_count: usize,
    pub transitive_goal_count: usize,
    pub direct_work_by_status: WorkByStatus,
    pub transitive_work_by_status: WorkByStatus,
    pub dependencies_cleared_direct_count: usize,
    pub unmet_dependency_count: usize,
    pub unresolved_dependency_count: usize,
    /// Up to three direct dependents, by canonical id.
    pub direct_examples: Vec<TaskId>,
}

/// Each stored reference resolved once, by its canonical spelling.
struct View<'a, F> {
    registry: &'a Registry,
    lookup: F,
    memo: HashMap<TaskId, Option<Task>>,
}

impl<F: FnMut(&TaskId) -> Result<Option<Task>>> View<'_, F> {
    fn resolve(&mut self, reference: &TaskId) -> Result<Option<Task>> {
        let key = self.registry.canonical_id(reference);
        if let Some(found) = self.memo.get(&key) {
            return Ok(found.clone());
        }
        let mut found = crate::dependency::resolve(self.registry, &key, &mut self.lookup)?;
        if let Some(task) = &mut found {
            task.id = self.registry.canonical_id(&task.id);
        }
        self.memo.insert(key, found.clone());
        Ok(found)
    }

    /// One record's dependencies: open targets by terminal id (recorded in `targets`), and
    /// unresolved references by canonical spelling, each once.
    fn judge(&mut self, task: &Task, targets: &mut BTreeMap<TaskId, Task>) -> Result<Judged> {
        let mut judged = Judged::default();
        for reference in &task.depends {
            let found = self.resolve(reference)?;
            match verdict(found.as_ref()) {
                Verdict::Satisfied => {}
                Verdict::Open => {
                    let found = found.expect("an open verdict has a record");
                    judged.open.insert(found.id.clone());
                    targets.entry(found.id.clone()).or_insert(found);
                }
                Verdict::Unresolved => {
                    judged
                        .unresolved
                        .entry(self.registry.canonical_id(reference))
                        .or_default()
                        .insert(reference.clone());
                }
            }
        }
        Ok(judged)
    }
}

#[derive(Default)]
struct Judged {
    open: BTreeSet<TaskId>,
    /// Canonical spelling -> every stored reference with it.
    unresolved: BTreeMap<TaskId, BTreeSet<TaskId>>,
}

impl Judged {
    fn unmet(&self, own: &TaskId) -> usize {
        self.open.iter().filter(|id| *id != own).count()
    }

    fn diagnostics(&self, dependent: &TaskId) -> impl Iterator<Item = UnresolvedDependency> {
        self.unresolved
            .iter()
            .flat_map(move |(canonical, references)| {
                references
                    .iter()
                    .map(move |reference| UnresolvedDependency {
                        dependent: dependent.clone(),
                        reference: reference.clone(),
                        canonical: (canonical != reference).then(|| canonical.clone()),
                        reason: "unresolved",
                    })
            })
    }
}

/// The analysis of `all`, one captured scan of the requested projects. `lookup` reads one
/// record by canonical id (the caller decides which copy answers); every move hop is
/// followed here. `coverage` carries what opening the scope established; the boundary,
/// the cut paths and the final verdict are derived here.
pub fn build(
    all: &[Task],
    registry: &Registry,
    mut coverage: Coverage,
    lookup: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> Result<Analysis> {
    let mut view = View {
        registry,
        lookup,
        memo: HashMap::new(),
    };
    let scanned: BTreeMap<TaskId, &Task> = all
        .iter()
        .map(|task| (registry.canonical_id(&task.id), task))
        .collect();
    let parents: HashSet<TaskId> = all
        .iter()
        .filter_map(|task| task.parent.as_ref())
        .map(|parent| registry.canonical_id(parent))
        .collect();
    let goal = |id: &TaskId| crate::hierarchy::is_goal(scanned[id], parents.contains(id));

    // Edges run from open scanned records to open targets; closed records add none.
    let mut targets = BTreeMap::new();
    let mut judged: BTreeMap<TaskId, Judged> = BTreeMap::new();
    for (id, task) in &scanned {
        if task.status.is_open() {
            judged.insert(id.clone(), view.judge(task, &mut targets)?);
        }
    }
    let mut dependents: BTreeMap<TaskId, BTreeSet<TaskId>> = BTreeMap::new();
    for (dependent, verdicts) in &judged {
        for target in verdicts.open.iter().filter(|target| *target != dependent) {
            dependents
                .entry(target.clone())
                .or_default()
                .insert(dependent.clone());
        }
    }

    // An out-of-scope target's own dependencies are judged for its counts and coverage;
    // none of them becomes an edge.
    let mut boundary: BTreeMap<TaskId, Judged> = BTreeMap::new();
    for id in dependents.keys().filter(|id| !scanned.contains_key(*id)) {
        let mut unused = BTreeMap::new();
        boundary.insert(id.clone(), view.judge(&targets[id], &mut unused)?);
    }
    let mut unresolved: Vec<UnresolvedDependency> = judged
        .iter()
        .chain(&boundary)
        .flat_map(|(id, verdicts)| verdicts.diagnostics(id))
        .collect();
    unresolved.sort_by_cached_key(|d| (d.dependent.to_string(), d.reference.to_string()));
    coverage.boundary_targets = boundary.keys().cloned().collect();
    coverage.unfollowed_targets = boundary
        .keys()
        .filter(|id| !targets[*id].depends.is_empty())
        .cloned()
        .collect();
    coverage.complete =
        coverage.complete && unresolved.is_empty() && coverage.unfollowed_targets.is_empty();

    let cycles = cycles(&judged);
    let in_cycle: HashSet<&TaskId> = cycles.iter().flatten().collect();
    let mut rows = Vec::new();
    for (id, direct) in &dependents {
        let task = &targets[id];
        let own = judged
            .get(id)
            .or_else(|| boundary.get(id))
            .expect("every target is judged");
        let mut row = Impact {
            task: task.clone(),
            goal: if scanned.contains_key(id) {
                Some(goal(id))
            } else {
                task.lane.then_some(true)
            },
            cycle: in_cycle.contains(id),
            direct_work_count: 0,
            transitive_work_count: 0,
            direct_goal_count: 0,
            transitive_goal_count: 0,
            direct_work_by_status: WorkByStatus::default(),
            transitive_work_by_status: WorkByStatus::default(),
            dependencies_cleared_direct_count: 0,
            unmet_dependency_count: own.unmet(id),
            unresolved_dependency_count: own.unresolved.len(),
            direct_examples: sorted(direct.iter().cloned()).into_iter().take(3).collect(),
        };
        for dependent in direct {
            if goal(dependent) {
                row.direct_goal_count += 1;
                continue;
            }
            row.direct_work_count += 1;
            row.direct_work_by_status.add(scanned[dependent].status);
            let verdicts = &judged[dependent];
            if verdicts.unmet(id) == 0 && verdicts.unresolved.is_empty() {
                row.dependencies_cleared_direct_count += 1;
            }
        }
        for dependent in reach(&dependents, id) {
            if goal(&dependent) {
                row.transitive_goal_count += 1;
            } else {
                row.transitive_work_count += 1;
                row.transitive_work_by_status
                    .add(scanned[&dependent].status);
            }
        }
        rows.push(row);
    }
    rows.sort_by_cached_key(|row| {
        (
            Reverse(row.transitive_work_count),
            Reverse(row.direct_work_count),
            row.task.priority,
            crate::time::parse(&row.task.created).expect("record stamps are validated at parse"),
            row.task.id.to_string(),
        )
    });
    Ok(Analysis {
        rows,
        coverage,
        unresolved,
        cycles,
    })
}

/// Ids in canonical full-id order.
fn sorted(ids: impl IntoIterator<Item = TaskId>) -> Vec<TaskId> {
    let mut ids: Vec<TaskId> = ids.into_iter().collect();
    ids.sort_by_cached_key(TaskId::to_string);
    ids
}

/// Every record reaching `target` over one or more edges, `target` itself excluded.
fn reach(dependents: &BTreeMap<TaskId, BTreeSet<TaskId>>, target: &TaskId) -> BTreeSet<TaskId> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![target];
    while let Some(id) = pending.pop() {
        for dependent in dependents.get(id).into_iter().flatten() {
            if dependent != target && seen.insert(dependent.clone()) {
                pending.push(dependent);
            }
        }
    }
    seen
}

/// Strongly connected components among open scanned records with more than one member or a
/// self-loop, each and all in canonical id order (iterative Tarjan).
fn cycles(judged: &BTreeMap<TaskId, Judged>) -> Vec<Vec<TaskId>> {
    let mut tarjan = Tarjan::default();
    let mut components = Vec::new();
    for root in judged.keys() {
        if tarjan.index.contains_key(root) {
            continue;
        }
        // Each frame: a node, its successors and how many of them were visited.
        let mut work = vec![tarjan.enter(root, judged)];
        while let Some((node, next, at)) = work.last_mut() {
            let node = *node;
            if let Some(&successor) = next.get(*at) {
                *at += 1;
                if !tarjan.index.contains_key(successor) {
                    work.push(tarjan.enter(successor, judged));
                } else if tarjan.on_stack.contains(successor) {
                    tarjan.lower(node, tarjan.index[successor]);
                }
                continue;
            }
            work.pop();
            if let Some((parent, _, _)) = work.last() {
                tarjan.lower(parent, tarjan.low[node]);
            }
            if tarjan.low[node] == tarjan.index[node] {
                let component = tarjan.pop_to(node);
                if component.len() > 1 || judged[node].open.contains(node) {
                    components.push(sorted(component));
                }
            }
        }
    }
    components.sort_by_cached_key(|component: &Vec<TaskId>| {
        component.iter().map(TaskId::to_string).collect::<Vec<_>>()
    });
    components
}

#[derive(Default)]
struct Tarjan<'a> {
    index: HashMap<&'a TaskId, usize>,
    low: HashMap<&'a TaskId, usize>,
    stack: Vec<&'a TaskId>,
    on_stack: HashSet<&'a TaskId>,
}

type Frame<'a> = (&'a TaskId, Vec<&'a TaskId>, usize);

impl<'a> Tarjan<'a> {
    fn enter(&mut self, id: &'a TaskId, judged: &'a BTreeMap<TaskId, Judged>) -> Frame<'a> {
        let at = self.index.len();
        self.index.insert(id, at);
        self.low.insert(id, at);
        self.stack.push(id);
        self.on_stack.insert(id);
        let next = judged[id]
            .open
            .iter()
            .filter(|target| judged.contains_key(*target))
            .collect();
        (id, next, 0)
    }

    fn lower(&mut self, id: &'a TaskId, to: usize) {
        let low = self.low.get_mut(id).expect("entered");
        *low = (*low).min(to);
    }

    fn pop_to(&mut self, node: &TaskId) -> Vec<TaskId> {
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack.remove(member);
            component.push(member.clone());
            if member == node {
                break;
            }
        }
        component
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::model::{Status, Task, TaskId};
    use crate::registry::Registry;
    use std::collections::HashMap;

    /// One record. `extra` adds frontmatter lines (parent, lane, every, moved_to, closed).
    fn rec(id: &str, status: Status, priority: u8, depends: &[&str], extra: &str) -> Task {
        rec_at(id, status, priority, depends, extra, "2026-10-01T00:00:00Z")
    }

    fn rec_at(
        id: &str,
        status: Status,
        priority: u8,
        depends: &[&str],
        extra: &str,
        created: &str,
    ) -> Task {
        let closed = if status.is_open() || status == Status::Moved {
            ""
        } else {
            "closed: 2026-10-02T00:00:00Z\n"
        };
        let text = format!(
            "---\nid: {id}\ntitle: T {id}\nstatus: {}\npriority: {priority}\n\
created: {created}\nupdated: {created}\ndepends: [{}]\ntags: []\n{extra}{closed}---\n",
            status.as_str(),
            depends.join(", "),
        );
        crate::format::parse_task(&text, &format!("tasks/{id}.md")).unwrap()
    }

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    fn ids(texts: &[&str]) -> Vec<TaskId> {
        texts.iter().map(|text| id(text)).collect()
    }

    /// Every record any project holds; `scope` names the prefixes scanned.
    struct World {
        records: Vec<Task>,
        registry: Registry,
    }

    impl World {
        fn new(records: Vec<Task>) -> World {
            World {
                records,
                registry: Registry::default(),
            }
        }

        fn scan(&self, scope: &[&str]) -> Vec<Task> {
            self.records
                .iter()
                .filter(|task| scope.contains(&task.id.prefix.as_str()))
                .cloned()
                .collect()
        }

        fn analyse(&self, scope: &[&str]) -> Analysis {
            self.analyse_scan(&self.scan(scope), scope)
        }

        fn analyse_scan(&self, all: &[Task], scope: &[&str]) -> Analysis {
            let by_id: HashMap<TaskId, Task> = self
                .records
                .iter()
                .map(|task| (task.id.clone(), task.clone()))
                .collect();
            let prefixes: Vec<String> = scope.iter().map(|p| p.to_string()).collect();
            let coverage = Coverage::new(CoverageMode::Group, prefixes.clone(), prefixes, vec![]);
            build(all, &self.registry, coverage, |id| {
                Ok(by_id.get(id).cloned())
            })
            .unwrap()
        }
    }

    fn row<'a>(analysis: &'a Analysis, of: &str) -> &'a Impact {
        analysis
            .rows
            .iter()
            .find(|row| row.task.id == id(of))
            .unwrap_or_else(|| panic!("no row for {of}"))
    }

    fn ranked(analysis: &Analysis) -> Vec<String> {
        analysis
            .rows
            .iter()
            .map(|row| row.task.id.to_string())
            .collect()
    }

    /// direct work, transitive work, direct goals, transitive goals, relief, own open, own ?
    fn counts(row: &Impact) -> [usize; 7] {
        [
            row.direct_work_count,
            row.transitive_work_count,
            row.direct_goal_count,
            row.transitive_goal_count,
            row.dependencies_cleared_direct_count,
            row.unmet_dependency_count,
            row.unresolved_dependency_count,
        ]
    }

    fn todo(n: usize) -> WorkByStatus {
        WorkByStatus {
            todo: n,
            ..WorkByStatus::default()
        }
    }

    /// The spec's verification table: B L R D M X G H.
    fn diamond() -> Vec<Task> {
        let open = Status::Todo;
        vec![
            rec("aa-000001", open, 2, &[], ""),
            rec("aa-000002", open, 3, &["aa-000001"], ""),
            rec("bb-000003", open, 1, &["aa-000001"], ""),
            rec("bb-000004", open, 1, &["aa-000002", "bb-000003"], ""),
            rec("bb-000005", open, 2, &["aa-000001", "aa-000006"], ""),
            rec("aa-000006", open, 2, &[], ""),
            rec("aa-000007", open, 0, &["aa-000001"], ""),
            rec("aa-000009", Status::Done, 2, &[], "parent: aa-000007\n"),
            rec("bb-000008", open, 0, &["aa-000001"], ""),
        ]
    }

    #[test]
    fn blockers_coverage_modes_use_the_spec_names() {
        let names: Vec<String> = [
            CoverageMode::Local,
            CoverageMode::Project,
            CoverageMode::Group,
            CoverageMode::AllProjects,
        ]
        .iter()
        .map(|mode| {
            serde_json::to_value(mode)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
        assert_eq!(names, ["local", "project", "group", "all_projects"]);
    }

    #[test]
    fn blockers_diamond_counts_and_ranking() {
        let world = World::new(diamond());
        let analysis = world.analyse(&["aa", "bb"]);
        assert_eq!(
            ranked(&analysis),
            ["aa-000001", "bb-000003", "aa-000006", "aa-000002"]
        );
        let b = row(&analysis, "aa-000001");
        assert_eq!(counts(b), [4, 5, 1, 1, 3, 0, 0]);
        assert_eq!(counts(row(&analysis, "bb-000003")), [1, 1, 0, 0, 0, 1, 0]);
        assert_eq!(counts(row(&analysis, "aa-000006")), [1, 1, 0, 0, 0, 0, 0]);
        assert_eq!(counts(row(&analysis, "aa-000002")), [1, 1, 0, 0, 0, 1, 0]);
        for row in &analysis.rows {
            assert_eq!(row.direct_work_by_status, todo(row.direct_work_count));
            assert_eq!(
                row.transitive_work_by_status,
                todo(row.transitive_work_count)
            );
            assert_eq!(row.goal, Some(false));
            assert!(!row.cycle);
        }
        assert_eq!(
            b.direct_examples,
            ids(&["aa-000002", "aa-000007", "bb-000003"])
        );
        assert!(analysis.coverage.complete);
        assert!(analysis.coverage.boundary_targets.is_empty());
        assert!(analysis.unresolved.is_empty() && analysis.cycles.is_empty());
    }

    #[test]
    fn blockers_scope_cut_and_boundary_targets() {
        let world = World::new(diamond());

        // bb scope: B, L and X are foreign boundary rows; only L has dependencies.
        let analysis = world.analyse(&["bb"]);
        assert_eq!(
            analysis.coverage.boundary_targets,
            ids(&["aa-000001", "aa-000002", "aa-000006"])
        );
        assert_eq!(analysis.coverage.unfollowed_targets, ids(&["aa-000002"]));
        assert!(!analysis.coverage.complete);
        assert_eq!(counts(row(&analysis, "aa-000001")), [3, 4, 0, 0, 2, 0, 0]);
        assert_eq!(counts(row(&analysis, "aa-000002")), [1, 1, 0, 0, 0, 1, 0]);
        assert_eq!(row(&analysis, "aa-000001").goal, None);

        // aa scope: L is work, G a goal; only L clears.
        let local = world.analyse(&["aa"]);
        assert_eq!(counts(row(&local, "aa-000001")), [1, 1, 1, 1, 1, 0, 0]);
        assert!(local.coverage.complete);

        // A boundary target with no dependencies alone keeps the scan complete.
        let only_x = vec![rec("bb-000005", Status::Todo, 2, &["aa-000006"], "")];
        let analysis = world.analyse_scan(&only_x, &["bb"]);
        assert_eq!(analysis.coverage.boundary_targets, ids(&["aa-000006"]));
        assert!(analysis.coverage.unfollowed_targets.is_empty());
        assert!(analysis.coverage.complete);

        // A -> F -> Y across aa/bb/aa: the cut hides Y in aa scope.
        let path = |y: Status| {
            World::new(vec![
                rec("aa-000010", Status::Todo, 2, &["bb-000011"], ""),
                rec("bb-000011", Status::Todo, 2, &["aa-000012"], ""),
                rec("aa-000012", y, 2, &[], ""),
            ])
        };
        let world = path(Status::Todo);
        let local = world.analyse(&["aa"]);
        assert_eq!(ranked(&local), ["bb-000011"]);
        let f = row(&local, "bb-000011");
        assert_eq!(counts(f), [1, 1, 0, 0, 1, 1, 0]);
        assert_eq!(f.goal, None);
        assert_eq!(local.coverage.unfollowed_targets, ids(&["bb-000011"]));
        assert!(!local.coverage.complete);
        let all = world.analyse(&["aa", "bb"]);
        assert_eq!(counts(row(&all, "aa-000012")), [1, 2, 0, 0, 1, 0, 0]);
        assert!(all.coverage.complete);

        // Its dependencies all closed, a foreign target is still conservatively partial.
        let local = path(Status::Done).analyse(&["aa"]);
        assert_eq!(counts(row(&local, "bb-000011")), [1, 1, 0, 0, 1, 0, 0]);
        assert_eq!(local.coverage.unfollowed_targets, ids(&["bb-000011"]));
        assert!(!local.coverage.complete);
    }

    #[test]
    fn blockers_identity_and_lifecycle() {
        // Each case: records, then the rows expected with their direct work counts.
        type Case<'a> = (&'a str, Vec<Task>, Vec<(&'a str, usize)>);
        let cases: Vec<Case> = vec![
            (
                "closed and dropped targets stop the path",
                vec![
                    rec("aa-000001", Status::Done, 2, &[], ""),
                    rec("aa-000002", Status::Dropped, 2, &[], ""),
                    rec(
                        "aa-000003",
                        Status::Todo,
                        2,
                        &["aa-000001", "aa-000002"],
                        "",
                    ),
                ],
                vec![],
            ),
            (
                "a closed consumer's history adds no edges",
                vec![
                    rec("aa-000001", Status::Todo, 2, &[], ""),
                    rec("aa-000002", Status::Done, 2, &["aa-000001"], ""),
                ],
                vec![],
            ),
            (
                "recurring done satisfies even when due",
                vec![
                    rec("aa-000001", Status::Done, 2, &[], "every: 1d\n"),
                    rec("aa-000002", Status::Todo, 2, &["aa-000001"], ""),
                ],
                vec![],
            ),
            (
                "a restarted recurrence blocks again",
                vec![
                    rec("aa-000001", Status::Doing, 2, &[], "every: 1d\n"),
                    rec("aa-000002", Status::Todo, 2, &["aa-000001"], ""),
                ],
                vec![("aa-000001", 1)],
            ),
            (
                "alias, old move id and destination are one prerequisite",
                vec![
                    rec(
                        "aa-000020",
                        Status::Moved,
                        2,
                        &["aa-000023"],
                        "moved_to: bb-000021\n",
                    ),
                    rec("bb-000021", Status::Todo, 2, &[], ""),
                    rec(
                        "aa-000022",
                        Status::Todo,
                        2,
                        &["old-000020", "aa-000020", "bb-000021"],
                        "",
                    ),
                    rec("aa-000023", Status::Todo, 2, &[], ""),
                ],
                vec![("bb-000021", 1)],
            ),
        ];
        for (name, records, expected) in cases {
            let mut world = World::new(records);
            world.registry.aliases.insert("old".into(), "aa".into());
            let analysis = world.analyse(&["aa", "bb"]);
            let got: Vec<(String, usize)> = analysis
                .rows
                .iter()
                .map(|row| (row.task.id.to_string(), row.direct_work_count))
                .collect();
            let want: Vec<(String, usize)> = expected
                .iter()
                .map(|(id, n)| (id.to_string(), *n))
                .collect();
            assert_eq!(got, want, "{name}");
            assert!(analysis.unresolved.is_empty(), "{name}");
            if let Some(row) = analysis.rows.first() {
                assert_eq!(row.dependencies_cleared_direct_count, 1, "{name}");
            }
        }

        // Counts deduplicate identities; diagnostics preserve every stored spelling.
        let mut world = World::new(vec![
            rec("aa-000020", Status::Moved, 2, &[], "moved_to: bb-000021\n"),
            rec("bb-000021", Status::Todo, 2, &[], ""),
            rec(
                "aa-000030",
                Status::Todo,
                2,
                &["aa-000020", "bb-000021", "old-0000ee", "aa-0000ee"],
                "",
            ),
            rec(
                "aa-000022",
                Status::Idea,
                2,
                &["old-000020", "aa-000020", "bb-000021"],
                "",
            ),
            rec("aa-000031", Status::Idea, 2, &["aa-000030"], ""),
        ]);
        world.registry.aliases.insert("old".into(), "aa".into());
        let analysis = world.analyse(&["aa", "bb"]);
        let own = row(&analysis, "aa-000030");
        assert_eq!(
            (own.unmet_dependency_count, own.unresolved_dependency_count),
            (1, 1)
        );
        let diagnostics: Vec<_> = analysis
            .unresolved
            .iter()
            .map(|d| {
                (
                    d.dependent.clone(),
                    d.reference.clone(),
                    d.canonical.clone(),
                )
            })
            .collect();
        assert_eq!(
            diagnostics,
            [
                (id("aa-000030"), id("aa-0000ee"), None),
                (id("aa-000030"), id("old-0000ee"), Some(id("aa-0000ee")))
            ]
        );
        let target = row(&analysis, "bb-000021");
        assert_eq!(counts(target), [2, 3, 0, 0, 1, 0, 0]);
        assert_eq!(target.direct_examples, ids(&["aa-000022", "aa-000030"]));
        let direct = WorkByStatus {
            idea: 1,
            todo: 1,
            ..WorkByStatus::default()
        };
        assert_eq!(target.direct_work_by_status, direct);
        let transitive = WorkByStatus {
            idea: 2,
            todo: 1,
            ..WorkByStatus::default()
        };
        assert_eq!(target.transitive_work_by_status, transitive);

        // Missing, unregistered and move-loop targets are diagnostics, never rows.
        let world = World::new(vec![
            rec(
                "aa-000001",
                Status::Todo,
                2,
                &["zz-000001", "aa-0000ff", "aa-000002"],
                "",
            ),
            rec("aa-000002", Status::Moved, 2, &[], "moved_to: bb-000002\n"),
            rec("bb-000002", Status::Moved, 2, &[], "moved_to: aa-000002\n"),
        ]);
        let analysis = world.analyse(&["aa", "bb"]);
        assert!(analysis.rows.is_empty());
        let diagnostics: Vec<(String, String, &str)> = analysis
            .unresolved
            .iter()
            .map(|d| (d.dependent.to_string(), d.reference.to_string(), d.reason))
            .collect();
        assert_eq!(
            diagnostics,
            [
                ("aa-000001".into(), "aa-000002".into(), "unresolved"),
                ("aa-000001".into(), "aa-0000ff".into(), "unresolved"),
                ("aa-000001".into(), "zz-000001".into(), "unresolved"),
            ]
        );
        assert!(!analysis.coverage.complete);

        // A missing dependency of a resolved foreign row is examined and makes it partial;
        // its own open and unresolved counts stay separate.
        let world = World::new(vec![
            rec("aa-000001", Status::Todo, 2, &["bb-000001"], ""),
            rec(
                "bb-000001",
                Status::Todo,
                2,
                &["bb-0000ff", "bb-000002"],
                "",
            ),
            rec("bb-000002", Status::Todo, 2, &[], ""),
        ]);
        let analysis = world.analyse(&["aa"]);
        assert_eq!(counts(row(&analysis, "bb-000001")), [1, 1, 0, 0, 1, 1, 1]);
        assert_eq!(analysis.unresolved.len(), 1);
        assert_eq!(analysis.unresolved[0].dependent, id("bb-000001"));
        assert_eq!(analysis.unresolved[0].reference, id("bb-0000ff"));
        assert!(!analysis.coverage.complete);

        // The same counts for an in-scope row: one open, one unresolved, one satisfied.
        let world = World::new(vec![
            rec(
                "aa-000001",
                Status::Todo,
                2,
                &["aa-000002", "aa-0000ff", "aa-000003"],
                "",
            ),
            rec("aa-000002", Status::Todo, 2, &[], ""),
            rec("aa-000003", Status::Done, 2, &[], ""),
            rec("aa-000004", Status::Todo, 2, &["aa-000001"], ""),
        ]);
        let analysis = world.analyse(&["aa"]);
        assert_eq!(counts(row(&analysis, "aa-000001")), [1, 1, 0, 0, 1, 1, 1]);

        // A failing lookup is the caller's typed error.
        let all = vec![rec("aa-000001", Status::Todo, 2, &["aa-000002"], "")];
        let coverage = Coverage::new(
            CoverageMode::Local,
            vec!["aa".into()],
            vec!["aa".into()],
            vec![],
        );
        let failed = build(&all, &Registry::default(), coverage, |_| {
            Err(Error::Validation("corrupt".into()))
        });
        assert!(matches!(failed, Err(Error::Validation(message)) if message == "corrupt"));
    }

    #[test]
    fn blockers_cycles_status_and_unknowns() {
        let todo_on = |id: &str, on: &[&str]| rec(id, Status::Todo, 2, on, "");

        // A two-node cycle, a self-loop (parsing refuses a literal one; an alias spelling
        // still reaches it) and a consumer of the cycle terminate and report.
        let mut world = World::new(vec![
            todo_on("aa-000001", &["aa-000002"]),
            todo_on("aa-000002", &["aa-000001"]),
            todo_on("aa-000003", &["old-000003"]),
            todo_on("aa-000004", &["aa-000001"]),
        ]);
        world.registry.aliases.insert("old".into(), "aa".into());
        let analysis = world.analyse(&["aa"]);
        assert_eq!(
            analysis.cycles,
            vec![ids(&["aa-000001", "aa-000002"]), ids(&["aa-000003"])]
        );
        assert_eq!(ranked(&analysis), ["aa-000001", "aa-000002"]);
        assert_eq!(counts(row(&analysis, "aa-000001")), [2, 2, 0, 0, 2, 1, 0]);
        assert_eq!(counts(row(&analysis, "aa-000002")), [1, 2, 0, 0, 1, 1, 0]);
        assert!(analysis.rows.iter().all(|row| row.cycle));

        // Goals: G (a lane) receives A's impact through an explicit edge, its open child
        // none; K reaches only a goal and ranks after every work row.
        let world = World::new(vec![
            rec("aa-000010", Status::Todo, 2, &[], "lane: true\n"),
            rec("aa-000011", Status::Todo, 2, &[], "parent: aa-000010\n"),
            todo_on("aa-000012", &["aa-000010"]),
            rec("aa-000013", Status::Todo, 0, &[], ""),
            rec("aa-000014", Status::Todo, 2, &["aa-000013"], "lane: true\n"),
        ]);
        let analysis = world.analyse(&["aa"]);
        assert_eq!(ranked(&analysis), ["aa-000010", "aa-000013"]);
        assert_eq!(row(&analysis, "aa-000010").goal, Some(true));
        assert_eq!(counts(row(&analysis, "aa-000013")), [0, 0, 1, 1, 0, 0, 0]);

        // Ten idea/shelved dependents outrank three todo/doing ones; the maps explain both.
        let mut records = vec![
            todo_on("aa-000020", &[]),
            todo_on("aa-000021", &[]),
            rec("aa-000030", Status::Todo, 2, &["aa-000021"], ""),
            rec("aa-000031", Status::Todo, 2, &["aa-000021"], ""),
            rec("aa-000032", Status::Doing, 2, &["aa-000021"], ""),
        ];
        for n in 0..10 {
            let status = if n < 6 { Status::Idea } else { Status::Shelved };
            records.push(rec(
                &format!("aa-0001{n:02}"),
                status,
                2,
                &["aa-000020"],
                "",
            ));
        }
        let analysis = World::new(records).analyse(&["aa"]);
        assert_eq!(ranked(&analysis), ["aa-000020", "aa-000021"]);
        let wide = WorkByStatus {
            idea: 6,
            shelved: 4,
            ..WorkByStatus::default()
        };
        assert_eq!(row(&analysis, "aa-000020").transitive_work_by_status, wide);
        assert_eq!(
            row(&analysis, "aa-000020").dependencies_cleared_direct_count,
            10
        );
        let narrow = WorkByStatus {
            todo: 2,
            doing: 1,
            ..WorkByStatus::default()
        };
        assert_eq!(row(&analysis, "aa-000021").direct_work_by_status, narrow);
        assert_eq!(row(&analysis, "aa-000021").transitive_work_count, 3);

        // Ties break by priority, creation time and full id, whatever the iteration order.
        let early = "2026-09-01T00:00:00Z";
        let late = "2026-10-01T00:00:00Z";
        let mut records = vec![
            rec_at("aa-000044", Status::Todo, 2, &[], "", late),
            rec_at("aa-000043", Status::Todo, 2, &[], "", late),
            rec_at("aa-000042", Status::Todo, 2, &[], "", early),
            rec_at("aa-000041", Status::Todo, 1, &[], "", late),
            todo_on("aa-000050", &["aa-000044", "aa-000043"]),
            todo_on("aa-000051", &["aa-000042", "aa-000041"]),
        ];
        let want = ["aa-000041", "aa-000042", "aa-000043", "aa-000044"];
        assert_eq!(ranked(&World::new(records.clone()).analyse(&["aa"])), want);
        records.reverse();
        for record in &mut records {
            record.depends.reverse();
        }
        assert_eq!(ranked(&World::new(records).analyse(&["aa"])), want);
    }
}
