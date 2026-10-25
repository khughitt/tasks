# Filtered-read routing review ledger

Spec round 1: accepted with no findings by codex/gpt-6.1-sol.

Frozen revision: ce08cdc6be0068351729c6e0b94ab63dd84a0394,
docs/specs/2026-10-25-filtered-read-routing-design.md.

Full returned review: builder-check-report.md; also attached to tasks-284d9b.
No rulings or deferred minor findings.
User approval is pending; no implementation plan or product changes exist.

## Spec round 2

Reviewer: claude-code/claude-opus-5-5, relayed by the user.
Verdict: revise; Important 1, Minor 3 against ce08cdc6be0068351729c6e0b94ab63dd84a0394.
Full returned text: claude-spec-round-2-report.md (including the reviewer's
round-number suggestion). The recorded builder acceptance makes this round 2.

Ruling: accept Important 1; route filters like positional IDs inside and outside
projects without explicit scope. Why: hierarchy::validate_parent excludes foreign
parents, so implied local scope guaranteed failure instead of finding an
unambiguous answer. Cost: a mistyped registered prefix can select another project,
as with tree today. Rejected alternative: keep guaranteed failure with improved
task_not_found detail. Revision: replacement of ce08cdc rule 2 in the next frozen spec.

Ruling: accept Minor 1; add explicit scope guidance to no_project detail in the
shared filtered-read opener. Why: the motivating source-only call should explain
how to search without interpreting provenance. Cost: callers depending on exact
error text observe a change; error kind and JSON shape stay the same.
Revision: next frozen spec rule 3 and source example.

Ruling: accept Minor 2; keep filter and tree completion shared and use
local_or_foreign without explicit scope. Why: the shared function can follow the
same routing policy for both, avoiding an unnecessary split. Cost: tree completion
offers foreign candidates in a case where it was previously empty; execution
already supports them. Revision: next frozen spec completion section.

Ruling: accept Minor 3; retain explicit-scope predicate intersection and state that
cross-project hierarchy filters cannot match. Why: parents and children share a
project, irrespective of scan width. Cost: explicit wide calls with incompatible
filters still return empty results rather than a conflict error.
Revision: next frozen spec precedence explanation.

All four findings are addressed in the spec revision; none are minor (deferred).

Revision for all four rulings:
f03f0b400c5cbcaea64ed5273da0949f85bb7616,
docs/specs/2026-10-25-filtered-read-routing-design.md.
The original ce08cdc commit is unchanged.

## Spec round 3

Reviewer: codex/gpt-6.1-sol, verified by the reviewer's fresh relay model invocation.
Verdict: accept; no findings against f03f0b400c5cbcaea64ed5273da0949f85bb7616.
Full returned text: revised-builder-check-report.md; also attached to tasks-284d9b.
The fresh builder confirmed all four round-2 findings are addressed.
No rulings or minor (deferred) findings were introduced in this round.
User approval of f03f0b4 remains pending. Next: after approval, prepare the
implementation plan in the retained worktree for user review.

## Spec round 4 and user approval

Reviewer: claude-code/claude-opus-5-5, relayed by the user.
Verdict: accept; Minor 2 against f03f0b400c5cbcaea64ed5273da0949f85bb7616.
Full text: claude-spec-round-4-report.md; attach to tasks-284d9b.
Keith approved f03f0b4 for planning in the same turn. The reviewer's references
to tests describe the spec's required tests; no product tests were added or run.

Ruling: accept Minor 1; move the explicit-scope shorthand statement under rule 1.
Why: separate the universally applicable shorthand rule from unscoped routing.
Cost: wording-only change; no behavior changes. Revision: post-f03f0b4 wording commit.

Ruling: accept Minor 2; replace rule 3's "may" with "does".
Why: the implementation boundary already requires decorating every returned
NoProject in the shared opener. Cost: wording-only change; no behavior changes.
Revision: post-f03f0b4 wording commit.

No extra spec-review round is required by the reviewer. Both nits are addressed,
none are minor (deferred). Plan must state registry-before-discovery order for
ID-filter calls, preserve the no-ID discovery order, and pin dual-corruption
error detail as well as kind. Implementation still requires user plan review.

Both spec round-4 wording rulings are implemented at
a92a91accc1a698d5606f8ded28bbe51a8e01cdd,
docs/specs/2026-10-25-filtered-read-routing-design.md.

## Plan round 1

Frozen plan: 3a9752f3d0edffb94f2a66a7c8f829fe5fa8756f,
docs/plans/2026-10-25-filtered-read-routing-plan.md.
Reviewer: codex/gpt-6.1-sol, verified by a fresh relay model invocation in this turn.
Review type: controller self-review under writing-plans, not independent review.
Verdict: accept; no findings. Full text: plan-self-review-report.md; attach to task.
No new rulings or minor (deferred) findings.

Plan steps are linked to tasks-3b3f0b (routing, direct/mid) and tasks-92cf3a
(completion, direct/low, depends on tasks-3b3f0b). Both remain todo.
Inline execution is the recommended bounded choice; no implementation has begun.
Next gate: Keith reviews plan 3a9752f. After approval the agent starts Task 1,
rechecks tasks-e147b9, and runs the deferred full-suite baseline before code edits.
