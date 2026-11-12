use super::{Ctx, append_note, guard_write, owner_name};
use crate::claims::{ClaimStore, Liveness};
use crate::config::HostConfig;
use crate::error::{Error, Result};
use crate::format::validate_task;
use crate::model::{Complexity, Status, Task, TaskId};

/// What `save` must do to the claim store once every validation has passed. Recorded by the
/// guard in `transition` (or by `park`); **nothing is persisted until `save` acts on it.**
pub enum ClaimIntent {
    Acquire(crate::claims::Claim),
    Release {
        clear_park: bool,
        /// Report a removed escalation even without an explicit reassessment. `shelve`
        /// sets it because a shelved task is not resumed; `done` and `drop` stay silent.
        announce_escalation: bool,
    },
    Park {
        park: crate::claims::Park,
        escalation: Option<crate::claims::Escalation>,
    },
    PreserveStore,
    /// An explicit rating replaced an escalation; the store must be saved even on a
    /// same-status edit, which otherwise preserves it.
    ClearEscalation(crate::claims::Escalation),
}

/// How this caller's right to act on an existing claim was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ownership {
    /// The resolved identity equals the claim's session. Today's path, unchanged: the
    /// resolved identity is used, with its current fields.
    ByIdentity,
    /// Identity did not match, or did not resolve at all, but the claim's own recorded
    /// process proof names this caller's nearest harness boundary.
    ByProof,
    /// Not the owner. Anything from here is acquisition.
    Foreign,
}

impl Ctx {
    /// The claim store, loaded on first use. Only reachable with the lock held, so every
    /// read-check-write against it sits inside one critical section.
    pub fn claims_mut(&mut self) -> Result<&mut ClaimStore> {
        if self.lock.is_none() {
            return Err(Error::Io(
                "claim store touched without the mutation lock".into(),
            ));
        }
        if self.claims.is_none() {
            self.claims = Some(ClaimStore::load(&self.project.prefix)?);
        }
        Ok(self.claims.as_mut().expect("just loaded"))
    }

    /// What a task record keeps of a takeover: the displaced logical session and owner
    /// and why it could be taken, never a Unix session PID or `describe_claim`'s machine
    /// details (work-claims design, "Command behaviour").
    pub fn takeover_summary(claim: &crate::claims::Claim, live: &Liveness) -> String {
        let how = match live {
            Liveness::Live => "live, forced",
            Liveness::Unverifiable(_) => "unverifiable, forced",
            Liveness::Stale(_) => "stale",
        };
        format!(
            "took over {} (owner {}, {how})",
            Ctx::session_label(claim),
            claim.owner
        )
    }

    /// How a task record names a claim's session: a Unix session's pid never reaches git.
    pub(crate) fn session_label(claim: &crate::claims::Claim) -> String {
        if is_unix_session(&claim.session) {
            "Unix session".to_string()
        } else {
            format!("session {}", claim.session)
        }
    }

    pub fn describe_claim(claim: &crate::claims::Claim, live: &Liveness) -> String {
        let pid = match claim.pid {
            Some(pid) => format!(", pid {pid}"),
            None => String::new(),
        };
        let state = match live {
            Liveness::Live => "live".to_string(),
            Liveness::Unverifiable(why) => format!("unverifiable: {why}"),
            Liveness::Stale(why) => format!("stale: {why}"),
        };
        let age = (time::OffsetDateTime::now_utc()
            - crate::time::parse(&claim.started).expect("loaded claim timestamp is valid"))
        .whole_seconds()
        .max(0);
        format!(
            "session {} (owner {}, host {}{pid}, worktree {}, since {}, age {age}s, {state})",
            claim.session, claim.owner, claim.host, claim.worktree, claim.started
        )
    }
    pub(crate) fn foreign_claim_detail(
        &self,
        id: &TaskId,
        claim: &crate::claims::Claim,
        live: &Liveness,
        include_continuation: bool,
    ) -> String {
        format_foreign_claim(
            id,
            claim,
            live,
            self.project.resume_root(),
            include_continuation,
        )
    }
}

pub(crate) fn format_foreign_claim(
    id: &TaskId,
    claim: &crate::claims::Claim,
    live: &Liveness,
    resume_root: &std::path::Path,
    include_continuation: bool,
) -> String {
    let description = Ctx::describe_claim(claim, live);
    let task_id = id.to_string();
    let continuation =
        if include_continuation && std::path::Path::new(&claim.worktree) == resume_root {
            let holder = Ctx::session_label(claim);
            format!(
                "; if this conversation continued from {holder}, take it over with \
             `tasks start --force {task_id} --reason \"continued from {holder}\"` \
             before retrying"
            )
        } else {
            String::new()
        };
    let release = match live {
        Liveness::Unverifiable(_) => crate::commands::release::hint(&task_id, &claim.session),
        _ => String::new(),
    };
    format!("{description}{continuation}{release}")
}

impl Ctx {
    /// Resolve identity, keeping a relay-level failure carryable so  can defer it. A
    /// failure with relay off is raised here, exactly where `identity` raised it before,
    /// which is what holds the relay-off guarantee at every call site rather than at some.
    pub(crate) fn resolve_for_guard(&mut self) -> Result<crate::claims::Resolution> {
        let me = crate::claims::resolve_identity(&self.host, &mut self.warnings);
        match me {
            crate::claims::Resolution::Failed(error) if !self.host.relay_identity => Err(error),
            other => Ok(other),
        }
    }

    pub fn refuse_foreign_live_claim(&mut self, id: &TaskId) -> Result<()> {
        self.refuse_foreign_claim(id, true)
    }

    pub(crate) fn refuse_foreign_claim(
        &mut self,
        id: &TaskId,
        include_continuation: bool,
    ) -> Result<()> {
        let me = self.resolve_for_guard()?;
        let existing = self.claims_mut()?.get(id).cloned();
        let Some(existing) = existing else {
            return me.require().map(|_| ());
        };
        if ownership(&existing, &me, &self.host) != Ownership::Foreign {
            return Ok(());
        }
        // Not the owner. Today's behaviour resolved an identity here whatever the verdict,
        // so a relay-level failure must still surface rather than be silently tolerated —
        // and before any refusal, so the operator sees why identity failed rather than a
        // refusal that merely follows from it.
        let held = existing.session.clone();
        me.require()
            .map_err(|error| crate::claims::name_the_claim(error, Some(&held)))?;
        let live = crate::claims::liveness(&existing);
        if !live.is_stale() {
            return Err(Error::Claimed(
                id.to_string(),
                self.foreign_claim_detail(id, &existing, &live, include_continuation),
            ));
        }
        Ok(())
    }

    /// Current contract (docs/user-guide.md): a save that changes `needs` under the caller's own live
    /// claim recomputes the claim's `holds` in the same save. A removed need drops out. An
    /// added exclusive need is an acquire: refused with `need_held` while another session's
    /// live claim on another task holds it, unless `force` and `need_reason` override. Only
    /// added needs are checked; what the claim already holds was checked when acquired.
    /// With no live claim of the caller's there is nothing to recompute. Runs after
    /// `refuse_foreign_live_claim`, under the project lock.
    pub(crate) fn update_holds(&mut self, task: &Task, force: bool) -> Result<()> {
        let Some(existing) = self.claims_mut()?.get(&task.id).cloned() else {
            return Ok(());
        };
        if crate::claims::liveness(&existing).is_stale() {
            return Ok(());
        }
        let me = self.resolve_for_guard()?;
        if ownership(&existing, &me, &self.host) == Ownership::Foreign {
            return Ok(());
        }
        let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);
        if holds == existing.holds {
            return Ok(());
        }
        // Any change, whether a hold is added, reduced, or the last one removed, is
        // published by `save` before the record write and restored if that write fails.
        // The lock spans both, so no other project's acquire sees a need released for the
        // instant in between and takes it.
        self.take_holds_lock()?;
        let added: Vec<String> = holds
            .iter()
            .filter(|need| !existing.holds.contains(*need))
            .cloned()
            .collect();
        if !added.is_empty() {
            let (snapshot, warnings) =
                crate::holds::HoldSnapshot::load(&self.registry, time::OffsetDateTime::now_utc());
            self.warnings.extend(warnings);
            // "Another session" by the ownership rule, as in `guard_holds`.
            let mine = own_holds(&snapshot, &me, &self.host)?;
            let held: Vec<(String, crate::holds::Holder)> = added
                .iter()
                .filter_map(|need| {
                    snapshot
                        .holder(need, &task.id, &mine)
                        .map(|holder| (need.clone(), holder.clone()))
                })
                .collect();
            if let Some((need, holder)) = held.first() {
                if !force {
                    return Err(Error::NeedHeld(format!(
                        "{id} needs {need}, held by {} ({}); add it past the hold with \
                         `tasks edit {id} --need {need} --force --reason \"...\"`",
                        holder.task,
                        holder.session,
                        id = task.id
                    )));
                }
                if self
                    .need_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(Error::Validation(format!(
                        "--force past a held need requires --reason: {} needs {need}, held \
                         by {} ({})",
                        task.id, holder.task, holder.session
                    )));
                }
                self.need_overrides = held;
            }
        }
        // `save` treats this as an acquire: store first, restored if the record write fails.
        self.pending_claim = Some((
            task.id.clone(),
            ClaimIntent::Acquire(crate::claims::Claim { holds, ..existing }),
        ));
        Ok(())
    }

    pub fn preserve_claim_store(&mut self, id: &TaskId) {
        self.pending_claim = Some((id.clone(), ClaimIntent::PreserveStore));
    }

    /// `edit --complexity` / `--no-complexity`: the explicit rating is the new truth, so the
    /// shared escalation goes (docs/task-model.md). Records only, with no warning yet -- whether the
    /// clear actually reached disk is `save`'s to know, and only `save` can report it, so it
    /// pushes the warning itself once it has saved. `given` is what was passed (`None` for
    /// `--no-complexity`), needed only if the store save fails and the retry has to be
    /// named. Called after the status handling so a transition's own intent — which saves
    /// the store anyway — is kept, and only a store-preserving intent is replaced.
    pub fn reassess(&mut self, id: &TaskId, given: Option<Complexity>) -> Result<()> {
        let Some(escalation) = self.claims_mut()?.escalation(id).cloned() else {
            return Ok(());
        };
        self.clear_escalation = Some(given);
        if matches!(
            self.pending_claim,
            None | Some((_, ClaimIntent::PreserveStore))
        ) {
            self.pending_claim = Some((id.clone(), ClaimIntent::ClearEscalation(escalation)));
        }
        Ok(())
    }

    /// docs/user-guide.md "Atomic across projects": the host-wide holds lock, after
    /// the project lock and never before. Kept on `Ctx` until the command ends, so it
    /// spans the hold check, `save`'s publish of the claim, the record write, and the
    /// restore of the previous claim when that write fails.
    fn take_holds_lock(&mut self) -> Result<()> {
        if self.lock.is_none() {
            return Err(Error::Io(
                "the holds lock was requested without the project lock".into(),
            ));
        }
        if self.holds_lock.is_none() {
            self.holds_lock = Some(crate::holds::lock()?);
        }
        Ok(())
    }

    /// Current contract (docs/user-guide.md): every acquire records the task's needs that this
    /// project declares exclusive (undeclared names ignored) as the claim's `holds`, and is
    /// refused with `need_held` while another session's live claim on another task holds
    /// one of them. Status and needs never change in one operation, so this always reads
    /// the needs on the record, and `start --force --reason` acquires the same ones.
    ///
    /// `force` comes only from `start` here (`edit --force` never reaches a move to doing).
    /// It overrides only with a reason; every held need is recorded for the audit notes.
    /// A claim on the task itself never holds it back, so a plain takeover needs none.
    /// A re-start that continues the caller's own live claim, already holding every one of
    /// these needs, acquires nothing new and is never held back.
    ///
    /// "Another session" is decided by `ownership`, as for the claim itself. The caller is
    /// the identity this acquire records: the resolved one, or the claim's own on a
    /// proof-only continuation. A hold whose session string differs but whose process
    /// proof names this caller's harness is therefore its own.
    pub(super) fn guard_holds(&mut self, task: &Task, force: bool) -> Result<()> {
        let holds = crate::needs::exclusive_of(&self.project.needs, &task.needs);
        // What the claim this acquire replaces holds: a repeated start's, a takeover's, or
        // a dead entry's. `save` publishes the new claim before writing the record and
        // restores this one if the write fails, so a change either way needs the lock.
        let replaced = self
            .claims_mut()?
            .get(&task.id)
            .map(|claim| claim.holds.clone())
            .unwrap_or_default();
        let me = match self.pending_claim.as_mut() {
            Some((_, ClaimIntent::Acquire(claim))) => {
                claim.holds = holds.clone();
                crate::claims::Resolution::Resolved(crate::claims::continuation_identity(claim))
            }
            _ => unreachable!("claim_guard records an acquire for every move to doing"),
        };
        if holds.is_empty() && replaced.is_empty() {
            return Ok(());
        }
        // The project lock is already held (every write command takes it first).
        self.take_holds_lock()?;
        if holds.is_empty() {
            // Dropping a hold checks nothing; only its publish and rollback need the lock.
            return Ok(());
        }
        // A re-start that continues the caller's own live claim, which already holds every
        // one of these needs, acquires nothing new: another session's hold, even one taken
        // by override, never refuses it.
        if let Some(existing) = self.claims_mut()?.get(&task.id).cloned()
            && !crate::claims::liveness(&existing).is_stale()
            && holds.iter().all(|need| existing.holds.contains(need))
            && ownership(&existing, &me, &self.host) != Ownership::Foreign
        {
            return Ok(());
        }
        let (snapshot, warnings) =
            crate::holds::HoldSnapshot::load(&self.registry, time::OffsetDateTime::now_utc());
        self.warnings.extend(warnings);
        let mine = own_holds(&snapshot, &me, &self.host)?;
        let Some((need, holder)) =
            crate::holds::held_back(&snapshot, &self.project.needs, task, &mine)
        else {
            return Ok(());
        };
        if !force {
            return Err(Error::NeedHeld(format!(
                "{id} needs {need}, held by {} ({}); override with `tasks start {id} --force \
                 --reason \"...\"`",
                holder.task,
                holder.session,
                id = task.id
            )));
        }
        if self
            .need_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(Error::Validation(format!(
                "--force past a held need requires --reason: {} needs {need}, held by {} ({})",
                task.id, holder.task, holder.session
            )));
        }
        self.need_overrides = holds
            .iter()
            .filter_map(|need| {
                snapshot
                    .holder(need, &task.id, &mine)
                    .map(|holder| (need.clone(), holder.clone()))
            })
            .collect();
        Ok(())
    }

    /// Guard only. Decides whether this session may make the change and records what `save`
    /// should do — **and persists nothing**, so a validation failure, a rejected concurrent
    /// edit, or a failed write cannot leave the store mutated.
    ///
    /// Release is destination-based: any destination other than `doing` releases this
    /// session's claim. It must not key off *leaving* a local `doing`, because a session can
    /// hold the shared claim while its own checkout still reads `todo` — the ordinary
    /// cross-worktree case — and its `done` there would otherwise strand the claim.
    pub(super) fn claim_guard(&mut self, id: &TaskId, to: Status, force: bool) -> Result<()> {
        let resolution = self.resolve_for_guard()?;
        let owner = owner_name(&self.project)?;
        let worktree = self.project.resume_root().display().to_string();

        let existing = self.claims_mut()?.get(id).cloned();
        let ownership = match &existing {
            Some(claim) => ownership(claim, &resolution, &self.host),
            None => Ownership::Foreign,
        };
        let mine = ownership != Ownership::Foreign;

        // The identity to record, decided *before* any refusal. A held resolution error
        // must be raised as itself the moment ownership fails ; letting a
        // `Claimed` refusal return first would hide why identity could not resolve.
        let me = match (&existing, ownership) {
            // Ownership by identity means resolution succeeded, so `require` cannot fail.
            // The resolved identity is used with its current fields, exactly as today: a
            // repeated `start` still replaces a stale pid, and a native level's `tagged`
            // form still reaches the park and escalation records.
            (_, Ownership::ByIdentity) => resolution.require()?,
            (Some(claim), Ownership::ByProof) => crate::claims::continuation_identity(claim),
            _ => {
                let held = existing.as_ref().map(|claim| claim.session.clone());
                resolution
                    .require()
                    .map_err(|error| crate::claims::name_the_claim(error, held.as_deref()))?
            }
        };

        let mut warning = None;
        if let Some(existing) = &existing {
            let live = crate::claims::liveness(existing);
            match (live.is_stale(), mine) {
                (false, false) if !(force && to == Status::Doing) => {
                    return Err(Error::Claimed(
                        id.to_string(),
                        self.foreign_claim_detail(id, existing, &live, to != Status::Doing),
                    ));
                }
                (false, false) => {
                    let what = match live {
                        Liveness::Unverifiable(_) => "an unverifiable",
                        _ => "a live",
                    };
                    warning = Some(format!(
                        "took over {what} claim held by {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                    self.takeover = Some(Ctx::takeover_summary(existing, &live));
                }
                (true, false) => {
                    warning = Some(format!(
                        "took over {}",
                        Ctx::describe_claim(existing, &live)
                    ));
                    self.takeover = Some(Ctx::takeover_summary(existing, &live));
                }
                _ => {}
            }
        }

        self.pending_claim = Some(if to == Status::Doing {
            let root_session = if mine {
                let old = existing
                    .as_ref()
                    .and_then(|claim| claim.root_session.as_ref());
                match (old, me.root_session.as_ref()) {
                    (Some(old), Some(new)) if old != new => {
                        return Err(Error::Config(format!(
                            "{id}: CODEX_SESSION_ID disagrees with the recorded root; inspect tasks show and the environment; restore the exports, or park this task and start it again to intentionally reassociate it"
                        )));
                    }
                    (Some(old), None) => Some(old.clone()),
                    _ => me.root_session.clone(),
                }
            } else {
                me.root_session.clone()
            };
            let now = crate::time::now();
            let started = match (&existing, mine) {
                (Some(claim), true) => claim.started.clone(),
                _ => now.clone(),
            };
            let evidence = crate::claims::evidence(
                &me,
                existing.as_ref().filter(|_| mine),
                crate::pidview::current().as_ref(),
                crate::claims::boot_id(),
                time::OffsetDateTime::now_utc(),
                crate::claims::proc_stat,
            );
            (
                id.clone(),
                ClaimIntent::Acquire(crate::claims::Claim {
                    owner,
                    pid_start: evidence.pid_start,
                    boot_id: evidence.boot_id,
                    pid_view: evidence.pid_view,
                    host: match &me.proof {
                        Some(proof) => proof.host.clone(),
                        None => crate::claims::hostname(),
                    },
                    session: me.session,
                    root_session,
                    pid: evidence.pid,
                    worktree,
                    started,
                    seen: now,
                    // Filled by `guard_holds`, which knows the task's needs.
                    holds: Vec::new(),
                }),
            )
        } else {
            (
                id.clone(),
                ClaimIntent::Release {
                    clear_park: matches!(to, Status::Done | Status::Dropped | Status::Shelved),
                    announce_escalation: to == Status::Shelved,
                },
            )
        });

        if let Some(warning) = warning {
            self.warnings.push(warning);
        }
        Ok(())
    }
}

pub(crate) fn is_unix_session(session: &str) -> bool {
    session
        .strip_prefix("sid:")
        .and_then(|pid| pid.parse::<u32>().ok())
        .is_some()
}

/// docs/task-model.md steps 2 and 3. Which of the two established the caller's right to act
/// matters: only proof-only ownership records the claim's own identity, because only
/// then is there no resolved identity that already agrees.
pub(crate) fn ownership(
    claim: &crate::claims::Claim,
    me: &crate::claims::Resolution,
    host: &HostConfig,
) -> Ownership {
    if let Some(identity) = me.identity()
        && claim.session == identity.session
    {
        return Ownership::ByIdentity;
    }
    // Proof is a relay-mode fallback, and it never overrides the explicit pair: agents
    // sharing one process are distinguished by TASKS_SESSION and by nothing else, so an
    // explicit mismatch is foreign however the ancestry looks. Spec constraint.
    let explicit = std::env::var_os("TASKS_SESSION").is_some_and(|value| !value.is_empty());
    if explicit || !host.relay_identity {
        return Ownership::Foreign;
    }
    let proved = crate::claims::proves_ownership(
        claim,
        &crate::relay::ancestry::current_scope(),
        &crate::claims::hostname(),
        crate::claims::boot_id().as_deref(),
        &|key| std::env::var_os(key),
    );
    if proved {
        Ownership::ByProof
    } else {
        Ownership::Foreign
    }
}

/// docs/user-guide.md "Blocks other sessions only": the live holding claims in
/// `snapshot` that are the caller's own, by the rule every claim check uses (`ownership`:
/// the resolved identity, else the claim's process proof). A session that moved from
/// native to relay identity therefore keeps its holds as it keeps its claims.
pub(crate) fn own_holds(
    snapshot: &crate::holds::HoldSnapshot,
    me: &crate::claims::Resolution,
    host: &HostConfig,
) -> Result<crate::holds::Mine> {
    snapshot.mine(|claim| Ok(ownership(claim, me, host) != Ownership::Foreign))
}

/// Which holds a read view counts as the caller's own (docs/user-guide.md). A read view
/// whose identity cannot resolve owns nothing: every hold counts as another session's,
/// even one this process could prove by pid. A resolved identity follows `own_holds`,
/// including process proof across a native-to-relay change. Write paths call
/// `own_holds` directly, because their continuations need proof.
pub(crate) fn read_holds(
    snapshot: &crate::holds::HoldSnapshot,
    me: &crate::claims::Resolution,
    host: &HostConfig,
) -> Result<crate::holds::Mine> {
    match me {
        crate::claims::Resolution::Failed(_) => Ok(crate::holds::Mine::default()),
        crate::claims::Resolution::Resolved(_) => own_holds(snapshot, me, host),
    }
}

/// Current contract (docs/project-operations.md): a write by the holder of a live claim moves the claim to this
/// checkout and refreshes its heartbeat. Holder means `ownership` is not `Foreign`,
/// by identity or by proof. It runs after the record is saved and never fails the command.
/// `me` is the caller's identity when the command already resolved it; otherwise it is
/// resolved here, and only when a claim exists to follow.
pub(crate) fn follow_holder(
    ctx: &mut Ctx,
    id: &TaskId,
    me: Option<&crate::claims::Resolution>,
    landed: &str,
) {
    let claim = match ctx.claims_mut() {
        Ok(store) => {
            // A write must not revive a stale claim.
            store.prune_dead();
            match store.get(id) {
                Some(claim) => claim.clone(),
                None => return,
            }
        }
        Err(error) => {
            ctx.warnings.push(format!(
                "{landed}, but the claim on {id} could not be read ({error})"
            ));
            return;
        }
    };
    let resolved;
    let me = match me {
        Some(me) => me,
        None => {
            resolved = crate::claims::resolve_identity(&ctx.host, &mut ctx.warnings);
            &resolved
        }
    };
    let identity = match ownership(&claim, me, &ctx.host) {
        Ownership::Foreign => return,
        // Proof-only ownership records the claim's own identity (docs/task-model.md of the relay
        // design), whose pid this write did not supply, so its evidence is kept.
        Ownership::ByProof => crate::claims::continuation_identity(&claim),
        Ownership::ByIdentity => me
            .identity()
            .expect("ownership by identity means the identity resolved")
            .clone(),
    };
    // Current contract (docs/ownership.md): a heartbeat applies the evidence rules too.
    let evidence = crate::claims::evidence(
        &identity,
        Some(&claim),
        crate::pidview::current().as_ref(),
        crate::claims::boot_id(),
        time::OffsetDateTime::now_utc(),
        crate::claims::proc_stat,
    );
    let moved = crate::claims::Claim {
        worktree: ctx.project.resume_root().display().to_string(),
        seen: crate::time::now(),
        pid: evidence.pid,
        pid_start: evidence.pid_start,
        boot_id: evidence.boot_id,
        pid_view: evidence.pid_view,
        ..claim
    };
    let saved = ctx.claims_mut().and_then(|store| {
        store.insert(id, moved);
        store.save()
    });
    if let Err(error) = saved {
        ctx.warnings.push(format!(
            "{landed}, but the claim heartbeat on {id} was not refreshed ({error}); the \
             claim may look stale to other sessions"
        ));
    }
}

/// The notes a need override owes to holders in this project, written by
/// `note_need_holders` only once the acquiring save has landed. A note saying a task
/// acquired a need must never outlive a refused or rolled-back acquire.
pub(crate) struct HolderNotes {
    owner: String,
    /// Holder task, need, note text.
    notes: Vec<(TaskId, String, String)>,
}

/// Current contract (docs/user-guide.md): the notes a need override leaves. The acquired task gets one
/// per held need now, so they land with its own save or not at all. A holder gets a
/// matching one only when it is in this project, the only one whose lock this command
/// holds. Those are returned, not written: the caller passes them to `note_need_holders`
/// after `save` succeeds. Both texts are validated here, before anything is saved.
/// `None` when no held need was overridden.
pub(crate) fn record_need_overrides(ctx: &mut Ctx, task: &mut Task) -> Result<Option<HolderNotes>> {
    let overrides = std::mem::take(&mut ctx.need_overrides);
    if overrides.is_empty() {
        return Ok(None);
    }
    let reason = ctx
        .need_reason
        .clone()
        .expect("the hold guard records an override only with a reason");
    let session = match &ctx.pending_claim {
        Some((_, ClaimIntent::Acquire(claim))) => claim.session.clone(),
        _ => unreachable!("a need override is recorded only on an acquire"),
    };
    let owner = owner_name(&ctx.project)?;
    let mut holders = HolderNotes {
        owner: owner.clone(),
        notes: Vec::new(),
    };
    for (need, holder) in &overrides {
        let target_note = format!(
            "need override: acquired while {need} held by {} ({}): {reason}",
            holder.task, holder.session
        );
        crate::format::validate_note_text(&target_note)?;
        if holder.prefix == ctx.project.prefix {
            let holder_note = format!(
                "need override: {} acquired {need} by {session} while this task held it: \
                 {reason}",
                task.id
            );
            crate::format::validate_note_text(&holder_note)?;
            holders
                .notes
                .push((holder.task.clone(), need.clone(), holder_note));
        }
        append_note(task, &owner, &target_note)?;
    }
    Ok(Some(holders))
}

/// Writes the holder notes `record_need_overrides` returned. Call it only after the
/// acquiring `save` returned `Ok`, still under the project lock. The acquisition has
/// landed by then, so a holder that cannot be noted is a warning, never the command's
/// failure.
/// No retired check: a holder holds a claim, and a retired project has none (`retire`
/// refuses while one is live, and `start` is refused afterwards).
pub(crate) fn note_need_holders(ctx: &mut Ctx, holders: HolderNotes) {
    for (holder, need, text) in holders.notes {
        let written = ctx.project.read_task(&holder).and_then(|mut held| {
            guard_write(&ctx.registry, &ctx.shorthand, &held)?;
            append_note(&mut held, &holders.owner, &text)?;
            held.updated = crate::time::after(&held.updated)?;
            validate_task(&held)?;
            ctx.project.validate_docs(&held)?;
            // Append-only audit note, as halt's: it skips load's stale-copy guard.
            ctx.project.write_task(&ctx.registry, &held)
        });
        match written {
            Ok(()) => {}
            Err(Error::TaskNotFound(_)) => ctx.warnings.push(format!(
                "{holder} holds {need} but its record is not in this checkout; no override \
                 note was written on it"
            )),
            Err(error) => ctx.warnings.push(format!(
                "the need override landed, but its note on {holder} was not written ({error})"
            )),
        }
    }
}

pub fn retained_owner(ctx: &mut Ctx, id: &TaskId) -> Result<()> {
    ctx.claims = Some(ClaimStore::load(&ctx.project.prefix)?);
    let me = ctx.resolve_for_guard()?;
    let claim = ctx.claims_mut()?.get(id).cloned().ok_or_else(|| {
        Error::RetainedInput(format!(
            "Run tasks start {id} to establish ownership before retained publication"
        ))
    })?;
    let live = crate::claims::liveness(&claim);
    if live.is_stale() {
        return Err(Error::RetainedInput(format!(
            "Run tasks start {id} to re-establish the stale claim before retry"
        )));
    }
    if ownership(&claim, &me, &ctx.host) == Ownership::Foreign {
        let held = claim.session.clone();
        me.require()
            .map_err(|error| crate::claims::name_the_claim(error, Some(&held)))?;
        return Err(Error::Claimed(
            id.to_string(),
            ctx.foreign_claim_detail(id, &claim, &live, true),
        ));
    }
    follow_holder(ctx, id, Some(&me), "retained ownership was checked");
    if ctx.claims_mut()?.get(id).is_none() {
        return Err(Error::RetainedInput(format!(
            "Claim on {id} expired before retained publication; run tasks start first"
        )));
    }
    if !ctx.warnings.is_empty() {
        return Err(Error::RetainedHistoryBlocked(ctx.warnings.join("; ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::{Claim, Liveness};

    #[test]
    fn describe_claim_includes_unverifiable_reason() {
        let claim = Claim {
            session: "agent-a".into(),
            root_session: None,
            owner: "main".into(),
            host: "testhost".into(),
            pid: Some(42),
            pid_start: Some(900),
            boot_id: Some("boot-a".into()),
            pid_view: Some("5:100".into()),
            worktree: "/tmp/wt".into(),
            started: "2026-10-15T00:00:00Z".into(),
            seen: "2026-10-15T00:00:00Z".into(),
            holds: Vec::new(),
        };
        let shown = Ctx::describe_claim(
            &claim,
            &Liveness::Unverifiable("recorded in another PID view".into()),
        );
        assert!(
            shown.ends_with(", unverifiable: recorded in another PID view)"),
            "{shown}"
        );
        assert!(shown.contains("session agent-a (owner main, host testhost, pid 42, worktree /tmp/wt, since 2026-10-15T00:00:00Z, age "), "{shown}");
    }

    #[test]
    fn foreign_claim_detail_gates_continuation_and_release_hints() {
        let id = TaskId::parse("sci-123456").unwrap();
        let claim = Claim {
            session: "agent-a".into(),
            root_session: None,
            owner: "main".into(),
            host: "testhost".into(),
            pid: Some(42),
            pid_start: Some(900),
            boot_id: Some("boot-a".into()),
            pid_view: Some("5:100".into()),
            worktree: "/tmp/wt".into(),
            started: "2026-10-15T00:00:00Z".into(),
            seen: "2026-10-15T00:00:00Z".into(),
            holds: Vec::new(),
        };
        let unverifiable = Liveness::Unverifiable("recorded in another PID view".into());

        // 1. Non-acquisition + same worktree + Live: continuation hint, no release hint.
        let same_live = format_foreign_claim(
            &id,
            &claim,
            &Liveness::Live,
            std::path::Path::new("/tmp/wt"),
            true,
        );
        assert!(
            same_live.contains(
                "; if this conversation continued from session agent-a, take it over with \
                 `tasks start --force sci-123456 --reason \"continued from session agent-a\"` \
                 before retrying"
            ),
            "{same_live}"
        );
        assert!(!same_live.contains("before parking"), "{same_live}");
        assert!(!same_live.contains("tasks release"), "{same_live}");

        // 2. Non-acquisition + different worktree + Live: bare claim description only.
        let cross_live = format_foreign_claim(
            &id,
            &claim,
            &Liveness::Live,
            std::path::Path::new("/tmp/other"),
            true,
        );
        assert!(!cross_live.contains("continued from"), "{cross_live}");
        assert!(!cross_live.contains("tasks release"), "{cross_live}");

        // 3. Non-acquisition + same worktree + Unix session + Live: sanitizes sid:<pid> in --reason.
        let mut unix_claim = claim.clone();
        unix_claim.session = "sid:4242".into();
        let unix_live = format_foreign_claim(
            &id,
            &unix_claim,
            &Liveness::Live,
            std::path::Path::new("/tmp/wt"),
            true,
        );
        assert!(
            unix_live.contains(
                "; if this conversation continued from Unix session, take it over with \
                 `tasks start --force sci-123456 --reason \"continued from Unix session\"` \
                 before retrying"
            ),
            "{unix_live}"
        );
        assert!(
            !unix_live.contains("continued from session sid:4242"),
            "{unix_live}"
        );
        assert!(!unix_live.contains("tasks release"), "{unix_live}");

        // 4. Non-acquisition + same worktree + Unverifiable: both continuation and release hints.
        let same_unverifiable = format_foreign_claim(
            &id,
            &claim,
            &unverifiable,
            std::path::Path::new("/tmp/wt"),
            true,
        );
        assert!(
            same_unverifiable
                .contains("--reason \"continued from session agent-a\"` before retrying"),
            "{same_unverifiable}"
        );
        assert!(
            same_unverifiable
                .contains("; if session agent-a is gone, run `tasks release sci-123456 \"<why>\"`"),
            "{same_unverifiable}"
        );

        // 5. Non-acquisition + different worktree + Unverifiable: release hint only.
        let cross_unverifiable = format_foreign_claim(
            &id,
            &claim,
            &unverifiable,
            std::path::Path::new("/tmp/other"),
            true,
        );
        assert!(
            !cross_unverifiable.contains("continued from"),
            "{cross_unverifiable}"
        );
        assert!(
            cross_unverifiable
                .contains("; if session agent-a is gone, run `tasks release sci-123456 \"<why>\"`"),
            "{cross_unverifiable}"
        );

        // 6. Acquisition (start / edit --status doing) + same worktree: never continuation, release only if unverifiable.
        let acq_live = format_foreign_claim(
            &id,
            &claim,
            &Liveness::Live,
            std::path::Path::new("/tmp/wt"),
            false,
        );
        assert!(!acq_live.contains("continued from"), "{acq_live}");
        assert!(!acq_live.contains("tasks release"), "{acq_live}");

        let acq_unverifiable = format_foreign_claim(
            &id,
            &claim,
            &unverifiable,
            std::path::Path::new("/tmp/wt"),
            false,
        );
        assert!(
            !acq_unverifiable.contains("continued from"),
            "{acq_unverifiable}"
        );
        assert!(
            acq_unverifiable
                .contains("; if session agent-a is gone, run `tasks release sci-123456 \"<why>\"`"),
            "{acq_unverifiable}"
        );
    }
}
