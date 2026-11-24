//! Project existing read-side predicates into action explanations for a captured scan.

use crate::claims::{ClaimSnapshot, Liveness, Park, Reason, WaitingOn};
use crate::dependency::{self, Verdict};
use crate::hierarchy::HierarchyView;
use crate::model::{Status, Task, TaskId};
use crate::output::{
    ActionActor, ActionEvidence, ActionExplanation, ActionKind, AnswerSubject, CurateWindow,
};
use crate::registry::Registry;
use crate::repo::Project;
use crate::review::{DepState, ReviewReason};
use serde_json::{Value, json};
use std::collections::HashMap;
use time::OffsetDateTime;

pub(super) struct Observation<'a> {
    pub registry: &'a Registry,
    pub host: &'a crate::config::HostConfig,
    pub projects: &'a [Project],
    pub scan: &'a [Task],
    pub claims: &'a ClaimSnapshot,
    pub now: OffsetDateTime,
}

/// Caches only the dependency, goal and host evidence requested by projected rows.
pub(super) struct Actions<'a> {
    observation: Observation<'a>,
    hierarchy: &'a HierarchyView<'a>,
    dependencies: HashMap<TaskId, Edge>,
    halts: HashMap<String, Result<crate::halt::HaltSnapshot, String>>,
    holds: Option<HostHolds>,
    membership: Option<crate::membership::Membership<'a>>,
    observed_at: String,
    hostname: String,
}

struct HostHolds {
    snapshot: crate::holds::HoldSnapshot,
    mine: crate::holds::Mine,
    warnings: Vec<String>,
}

#[derive(Clone)]
struct Edge {
    id: TaskId,
    state: DepState,
}

impl<'a> Actions<'a> {
    pub(super) fn new(observation: Observation<'a>, hierarchy: &'a HierarchyView<'a>) -> Self {
        Self {
            observed_at: crate::time::format(observation.now),
            hostname: crate::claims::hostname(),
            observation,
            hierarchy,
            dependencies: HashMap::new(),
            halts: HashMap::new(),
            holds: None,
            membership: None,
        }
    }

    pub(super) fn explain(
        &mut self,
        task: &Task,
        review: Option<ReviewReason>,
    ) -> Vec<ActionExplanation> {
        if task.status == Status::Moved {
            return vec![];
        }
        let id = self.observation.registry.canonical_id(&task.id);
        let goal = crate::hierarchy::is_goal(task, self.hierarchy.child_count(&id) != 0);
        let park = self.observation.claims.park(&task.id);
        let claim = self.claim_reason(task);
        let proposal = crate::proposal::pending(task).map(|text| {
            json!({
                "source":"latest_note", "task":id, "note_index":task.notes.len() - 1,
                "text":text
            })
        });
        let mut actions = Vec::new();

        if task.status.is_open() || task.every.is_some() {
            let reasons = self.execution_reasons(task, &id, goal, park, claim.as_ref());
            let mut pickup = reasons.clone();
            if !crate::query::is_candidate(task, self.observation.now)
                && !claim
                    .as_ref()
                    .is_some_and(|reason| reason.code == "already_active")
            {
                pickup.push(self.pickup_lifecycle(task));
            }
            actions.push(explanation(ActionKind::PickUp, pickup, vec![]));

            if park.is_some() || claim.is_some() || task.status == Status::Doing {
                let mut resume = reasons;
                if !super::parked::resumable_lifecycle(task) {
                    resume.push(self.pickup_lifecycle(task));
                } else if park.is_none() && claim.is_none() {
                    resume.push(evidence("evidence_unknown", json!({
                        "task":id, "source":"park", "host":self.hostname, "observed_at":self.observed_at
                    })));
                }
                actions.push(explanation(ActionKind::Resume, resume, vec![]));
            }
        }

        match task.status {
            Status::Idea | Status::Todo | Status::Blocked => {
                if !goal || review.is_some() {
                    actions.push(self.assess(task, &id, review, proposal.as_ref()));
                }
                actions.push(self.curate(task, claim, proposal.as_ref()));
            }
            Status::Shelved => actions.push(self.reconsider(task, review)),
            _ => {}
        }
        if task.status.is_open() {
            if proposal.is_some() {
                actions.push(answer(AnswerSubject::LatestNote {
                    task: id.clone(),
                    note_index: task.notes.len() - 1,
                }));
            }
            if task.awaiting_user {
                actions.push(answer(AnswerSubject::AwaitingUser { task: id.clone() }));
            }
            if let Some(park) = park.filter(|park| {
                park.waiting_on == WaitingOn::User
                    && matches!(
                        park.reason,
                        Some(Reason::Decision | Reason::Review | Reason::Approval)
                    )
            }) {
                actions.push(answer(AnswerSubject::Park {
                    task: id.clone(),
                    host: park.host.clone(),
                    at: park.at.clone(),
                    next_step: park.next_step.clone(),
                }));
            }
        }
        if goal && task.status.is_open() {
            actions.push(self.close_goal(task, &id));
        }
        actions
    }

    fn project(&self, prefix: &str) -> Option<&'a Project> {
        self.observation
            .projects
            .iter()
            .find(|project| project.prefix == prefix)
    }

    fn edge(&mut self, stored: &TaskId) -> &Edge {
        let registry = self.observation.registry;
        let canonical = registry.canonical_id(stored);
        self.dependencies
            .entry(canonical.clone())
            .or_insert_with(|| {
                let found = dependency::resolve_canonical(registry, &canonical, |id| {
                    match self.hierarchy.get(id) {
                        Some(task) => Ok(Some(task.clone())),
                        None if self
                            .observation
                            .projects
                            .iter()
                            .any(|p| p.prefix == id.prefix) =>
                        {
                            Ok(None)
                        }
                        None => crate::resolve::resolve_registered_canonical(registry, id),
                    }
                });
                match found {
                    Ok(Some(task)) if dependency::verdict(Some(&task)) == Verdict::Satisfied => {
                        Edge {
                            id: registry.canonical_id(&task.id),
                            state: DepState::Closed {
                                closed: task.closed,
                            },
                        }
                    }
                    Ok(Some(task)) => Edge {
                        id: registry.canonical_id(&task.id),
                        state: DepState::Open,
                    },
                    // An optional evidence read cannot change the enclosing command's errors.
                    Ok(None) | Err(_) => Edge {
                        id: canonical,
                        state: DepState::Unresolved,
                    },
                }
            })
    }

    fn dependency_reasons(&mut self, dependencies: &[TaskId]) -> Vec<ActionEvidence> {
        dependencies
            .iter()
            .filter_map(|dependency| {
                let edge = self.edge(dependency);
                let code = match edge.state {
                    DepState::Open => "dependency_open",
                    DepState::Unresolved => "dependency_unknown",
                    DepState::Closed { .. } => return None,
                };
                Some(evidence(code, json!({"task":edge.id})))
            })
            .collect()
    }

    fn claim_reason(&self, task: &Task) -> Option<ActionEvidence> {
        let (claim, liveness) = self.observation.claims.get(&task.id)?;
        let mut fact = json!({
            "task":task.id, "host":claim.host, "observed_at":self.observed_at,
            "owner":claim.owner, "seen":claim.seen
        });
        let code = match liveness {
            Liveness::Stale(_) => return None,
            Liveness::Live if task.status == Status::Doing => "already_active",
            Liveness::Live => "claim_held",
            Liveness::Unverifiable(detail) => {
                fact["detail"] = json!(detail);
                "claim_unknown"
            }
        };
        Some(evidence(code, fact))
    }

    fn pickup_lifecycle(&self, task: &Task) -> ActionEvidence {
        let code = match task.status {
            Status::Blocked => "blocked",
            Status::Shelved => "shelved",
            Status::Done if task.every.is_some() => {
                return evidence(
                    "recurrence_not_due",
                    json!({
                        "date":crate::periodic::due(task).map(crate::time::format)
                    }),
                );
            }
            _ => "lifecycle_ineligible",
        };
        evidence(code, json!({"status":task.status}))
    }

    fn execution_reasons(
        &mut self,
        task: &Task,
        id: &TaskId,
        goal: bool,
        park: Option<&Park>,
        claim: Option<&ActionEvidence>,
    ) -> Vec<ActionEvidence> {
        let mut reasons = self.dependency_reasons(&task.depends);
        if goal {
            reasons.push(evidence("goal", json!({"task":id})));
        }
        if let Some(defer) = self.deferred(task) {
            reasons.push(defer);
        }
        if let Some(lane) = self.hierarchy.paused_lane(id) {
            reasons.push(evidence("lane_paused", json!({"task":lane.id})));
        }
        if let Some(claim) = claim {
            reasons.push(claim.clone());
        }
        if let Some(park) = park.filter(|park| park.waiting_on == WaitingOn::User) {
            let code = if matches!(park.reason, Some(Reason::Quiet | Reason::Environment)) {
                "host_condition"
            } else {
                "user_handoff"
            };
            reasons.push(evidence(
                code,
                json!({
                    "task":id, "host":park.host, "observed_at":self.observed_at, "at":park.at,
                    "next_step":park.next_step
                }),
            ));
        }
        if let Some(project) = self.project(&id.prefix) {
            self.add_halt_reasons(task, project, &mut reasons);
            self.add_hold_reasons(task, project, &mut reasons);
        } else {
            reasons.push(evidence("evidence_unknown", json!({"project":id.prefix})));
        }
        reasons
    }

    fn add_halt_reasons(
        &mut self,
        task: &Task,
        project: &Project,
        reasons: &mut Vec<ActionEvidence>,
    ) {
        let snapshot = self.halts.entry(project.prefix.clone()).or_insert_with(|| {
            crate::halt::observed(project, self.observation.registry, self.observation.scan)
                .map_err(|error| error.to_string())
        });
        match snapshot {
            Ok(snapshot) => reasons.extend(
                snapshot
                    .blocking(task)
                    .into_iter()
                    .map(|halt| evidence("halt", json!({"task":halt.id}))),
            ),
            Err(detail) => reasons.push(evidence(
                "evidence_unknown",
                json!({
                    "project":project.prefix, "source":"halt", "detail":detail
                }),
            )),
        }
    }

    fn add_hold_reasons(
        &mut self,
        task: &Task,
        project: &Project,
        reasons: &mut Vec<ActionEvidence>,
    ) {
        let needs = crate::needs::exclusive_of(&project.needs, &task.needs);
        if needs.is_empty() {
            return;
        }
        let holds = self.holds.get_or_insert_with(|| {
            let (snapshot, mut warnings) =
                crate::holds::HoldSnapshot::load(self.observation.registry, self.observation.now);
            let mine = if snapshot.is_empty() {
                Ok(crate::holds::Mine::default())
            } else {
                // The picker treats known holders as foreign when identity is unavailable.
                // Identity diagnostics do not make an unrelated free need uncertain.
                super::list::view_holds(&snapshot, &mut Vec::new(), self.observation.host)
            };
            let mine = mine.unwrap_or_else(|error| {
                warnings.push(error.to_string());
                crate::holds::Mine::default()
            });
            HostHolds {
                snapshot,
                mine,
                warnings,
            }
        });
        for need in needs {
            for holder in holds
                .snapshot
                .matching_holders(&need, &task.id, &holds.mine)
            {
                reasons.push(evidence(
                    "resource_held",
                    json!({
                        "resource":need, "task":holder.task,
                        "host":self.hostname, "observed_at":self.observed_at
                    }),
                ));
            }
        }
        reasons.extend(holds.warnings.iter().map(|detail| evidence("evidence_unknown", json!({
            "source":"holds", "host":self.hostname, "observed_at":self.observed_at, "detail":detail
        }))));
    }

    fn deferred(&self, task: &Task) -> Option<ActionEvidence> {
        task.defer
            .filter(|_| crate::defer::is_deferred(task, self.observation.now))
            .map(|date| evidence("deferred", json!({"date":date.to_string()})))
    }

    fn assess(
        &mut self,
        task: &Task,
        id: &TaskId,
        review: Option<ReviewReason>,
        proposal: Option<&Value>,
    ) -> ActionExplanation {
        let mut reasons = Vec::new();
        if review.is_none() {
            if task.status == Status::Idea {
                reasons.extend(self.dependency_reasons(&task.depends));
                reasons.extend(self.deferred(task));
                if task.awaiting_user {
                    reasons.push(evidence(
                        "decision_open",
                        json!({"source":"awaiting_user", "task":id}),
                    ));
                }
                if let Some(proposal) = proposal {
                    reasons.push(evidence("decision_open", proposal.clone()));
                }
            }
            if reasons.is_empty() {
                reasons.push(evidence(
                    "assessment_current",
                    json!({
                        "source":if task.status == Status::Idea { "reviewed" } else { "process" },
                        "task":id, "process":task.process,
                    }),
                ));
            }
        }
        let signals = if matches!(
            review,
            Some(ReviewReason::Unreviewed | ReviewReason::Unassessed)
        ) {
            vec![evidence(
                "scope_unverified",
                json!({
                    "source":if task.status == Status::Idea { "reviewed" } else { "process" },
                    "task":id
                }),
            )]
        } else {
            vec![]
        };
        explanation(ActionKind::Assess, reasons, signals)
    }

    fn reconsider(&mut self, task: &Task, review: Option<ReviewReason>) -> ActionExplanation {
        let mut signals = Vec::new();
        if crate::defer::is_due(task, self.observation.now) {
            signals.push(evidence(
                "shelf_due",
                json!({"date":task.defer.map(|date| date.to_string())}),
            ));
        }
        for target in &task.wake_on {
            let edge = self.edge(target);
            let code = match edge.state {
                DepState::Closed { .. } => "wake_target_closed",
                DepState::Unresolved => "wake_target_unknown",
                DepState::Open => continue,
            };
            signals.push(evidence(code, json!({"task":edge.id})));
        }
        let reasons = if review.is_some() {
            vec![]
        } else {
            vec![evidence(
                "shelf_sleeping",
                json!({
                    "date":task.defer.map(|date| date.to_string()),
                    "wake_on":task.wake_on
                }),
            )]
        };
        explanation(ActionKind::Reconsider, reasons, signals)
    }

    fn curate(
        &self,
        task: &Task,
        claim: Option<ActionEvidence>,
        proposal: Option<&Value>,
    ) -> ActionExplanation {
        let window =
            super::sample::Window::new(super::sample::DEFAULT_AGE_DAYS, self.observation.now);
        let mut reasons = Vec::new();
        reasons.extend(self.deferred(task));
        reasons.extend(claim);
        match window.recent(task) {
            Ok(true) => reasons.push(evidence("sample_recent", json!({"updated":task.updated}))),
            Ok(false) => {}
            Err(error) => reasons.push(evidence(
                "evidence_unknown",
                json!({"detail":error.to_string()}),
            )),
        }
        if let Some(proposal) = proposal {
            reasons.push(evidence("decision_open", proposal.clone()));
        }
        let mut action = explanation(ActionKind::Curate, reasons, vec![]);
        action.window = Some(CurateWindow {
            older_than_days: window.days,
        });
        action
    }

    fn close_goal(&mut self, task: &Task, id: &TaskId) -> ActionExplanation {
        let mut reasons = self.dependency_reasons(&task.depends);
        let membership = self.membership.get_or_insert_with(|| {
            let prefixes = self
                .observation
                .projects
                .iter()
                .map(|p| p.prefix.clone())
                .collect::<Vec<_>>();
            crate::membership::Membership::new(
                self.observation.registry,
                &prefixes,
                self.observation.scan,
            )
        });
        match membership.closeout_members(task) {
            Ok(closeout) => {
                reasons.extend(
                    closeout
                        .open
                        .iter()
                        .map(|id| evidence("member_open", json!({"task":id}))),
                );
                reasons.extend(
                    closeout
                        .missing
                        .iter()
                        .map(|(_, id)| evidence("member_unknown", json!({"task":id}))),
                );
            }
            Err(error) => reasons.push(evidence(
                "evidence_unknown",
                json!({
                    "task":id, "source":"membership", "detail":error.to_string()
                }),
            )),
        }
        if !matches!(task.status, Status::Todo | Status::Doing | Status::Blocked)
            || (task.pieces.is_empty() && self.hierarchy.child_count(id) == 0)
        {
            reasons.push(evidence(
                "lifecycle_ineligible",
                json!({"status":task.status}),
            ));
        }
        explanation(ActionKind::CloseGoal, reasons, vec![])
    }
}

fn evidence(code: &'static str, evidence: Value) -> ActionEvidence {
    ActionEvidence { code, evidence }
}

fn explanation(
    action: ActionKind,
    mut reasons: Vec<ActionEvidence>,
    mut signals: Vec<ActionEvidence>,
) -> ActionExplanation {
    for facts in [&mut reasons, &mut signals] {
        facts.sort_by(|a, b| {
            a.code.cmp(b.code).then_with(|| {
                a.evidence["task"]
                    .as_str()
                    .cmp(&b.evidence["task"].as_str())
            })
        });
        facts.dedup_by(|a, b| a.code == b.code && a.evidence == b.evidence);
    }
    ActionExplanation {
        action,
        actor: if matches!(action, ActionKind::Answer) {
            ActionActor::Human
        } else {
            ActionActor::Agent
        },
        available: reasons.is_empty(),
        reasons,
        signals,
        subject: None,
        window: None,
    }
}

fn answer(subject: AnswerSubject) -> ActionExplanation {
    let mut action = explanation(ActionKind::Answer, vec![], vec![]);
    action.subject = Some(subject);
    action
}
