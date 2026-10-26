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
