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
