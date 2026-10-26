# Blocker ranking design review ledger

Spec round 2: c45fc7f:docs/specs/2026-10-25-blocker-ranking-design.md
Reviewer /root/blocker_spec_check, codex/gpt-6.1-sol. Verdict: accept; findings: none.
Full returned report: spec-builder-check-report.md.
No Ruling entries or minor (deferred) findings arose in this builder check.
User design approval is pending. No plan or implementation is authorized yet.

Spec round 3: c45fc7f:docs/specs/2026-10-25-blocker-ranking-design.md
Reviewer identity supplied by the user: claude-code/claude-opus-5-5.
Verdict: revise; findings: Important 3, Minor 5.
Full returned review: spec-round-3-report.md (also attached to the task).

Addressed in frozen revision d2940d0:docs/specs/2026-10-25-blocker-ranking-design.md:
- Important 1: unfollowed_targets marks cut foreign paths partial; corrected bb-scope
  fixture and added aa -> bb -> aa local/group/all-projects fixture.
- Important 2: own known-open and unresolved dependency counts in JSON/pretty;
  explicit zero/unknown semantics and fixture assertions.
- Important 3: internal shared-verdict extraction can ship here; no wait on the
  separate public-contract design/plan/build.
- Minor 1: name review::Deps::of_scan and open_deps; preserve caller-specific
  unresolved handling and closed timestamps under a common internal verdict.
- Minor 2: explain and test the cost of no implicit goal-to-child impact.
- Minor 3: awaiting_user signal limited to reviewed ideas, not ordinary todo.
- Minor 4: task_needs distinct from existing park.needs host-capability enum.
- Minor 5: optional ClaimInfo for live/unverifiable holders; omit stale claims.

Author decision: retain equal-weight total ranking with per-status breakdowns;
no new status filters or scheduling policy. Optional wording and width feedback
also addressed with simpler absent-park wording and compact rows/continuations.
No round-3 findings rejected or minor (deferred). No Ruling entries required.
A fresh builder re-review of d2940d0 is pending; user design approval is pending.

Spec round 4: d2940d01ace1af6a83c0d79a24787029f5ca4d92:docs/specs/2026-10-25-blocker-ranking-design.md
Fresh reviewer /root/blocker_spec_recheck; verified codex/gpt-6.1-sol.
Verdict: accept for planning; findings: none; nothing important left.
Full returned review: spec-round-4-report.md.
No rejected findings, Ruling entries or minor (deferred) findings.
The round-3 corrective revision is verified by round 4. User design approval is
still pending; no plan or implementation has been authored.

Spec round 5: d2940d0:docs/specs/2026-10-25-blocker-ranking-design.md
Supplied reviewer claude-code/claude-opus-5-5. Verdict accept; findings Minor 1.
Full report: spec-round-5-report.md, attached to tasks-e637a1.
Controller accepts the design following the user's delegation to decide and plan.
Round-5 Minor 1 is addressed in plan Task 2: bb boundary set B,L,X and unfollowed
set L, plus X-only no-cut coverage variant. Approved spec bytes remain frozen.
No rejected findings, Ruling entries or minor (deferred) findings remain.

Plan round 1: 1b1a11c5dd2bc897d93fa0f87214d8e842e3eaf6:docs/plans/2026-10-25-blocker-ranking.md
Controller self-review codex/gpt-6.1-sol; accept for user review, findings none.
Full self-review: plan-self-review-report.md. This is not independent review.
User implementation-plan review is pending. No implementation has begun.

Plan round 2: cd801493b06883d4415457f45d71840a94f505ad:docs/plans/2026-10-25-blocker-ranking.md
Controller self-review codex/gpt-6.1-sol, verified again this turn. Verdict accept
for user review; findings none. Full self-review: plan-self-review-round-2-report.md.
This revision supersedes the 1b1a11c plan for user review: final handoff now preserves
reinstallation already authorized by the repository guide, from main after
integration, instead of introducing another approval gate. No source code changed.
No Ruling entries or minor (deferred) findings. Plan user review remains pending.

Plan round 3: cd80149:docs/plans/2026-10-25-blocker-ranking.md
Reviewer supplied as claude-code/claude-opus-5-5. Verdict revise; Important 2, Minor 3.
Full report: plan-round-3-report.md. All five findings addressed in 327fe22.
Important 1: exact isolated/normal suite gates and failure classification at every
step. Minor 1: disk-backed Resolver assertions. Minor 2: Coverage seed/derived
ownership. Minor 3: explicit warning and heading assertions. None deferred.

Ruling: S1 — remove tasks-5d0d99's hard prerequisite on tasks-e8d471; supersede only
that sequencing in spec d2940d0 and plan cd80149 with revised plan 327fe22.
Why: existing Scope::scan supplies the captured rows; build consumes no new
show-dependents artifact and adds no project walker, so the hard edge delays work
without interface coupling. A coordination note is on e8d471 in the registered root.
Cost: independent features can drift in scan semantics and require coordination
or factoring when actual shared callers justify it. User review of the revised
plan explicitly includes this spec-level amendment. No report policy changes.

Plan round 4: 327fe22b8fa7676dcc153ef6bd2d181a8d1e3be1:docs/plans/2026-10-25-blocker-ranking.md
Controller self-review codex/gpt-6.1-sol, freshly verified this turn. Accept for
user review; findings none. Full report: plan-round-4-self-review-report.md.
Not independent review. No minor (deferred) findings. User plan approval pending.
