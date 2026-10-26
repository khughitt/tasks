# Plan round 4 self-review

Frozen repository: tasks
Revision: 327fe22b8fa7676dcc153ef6bd2d181a8d1e3be1:docs/plans/2026-10-25-blocker-ranking.md
Reviewer: controller self-review, codex/gpt-6.1-sol, verified by a fresh command
in this turn. This is not independent review or user plan approval.
Verdict: accept for revised-plan user review; findings: none.

All round-3 findings are addressed:
- Important 1: each tasks-project implementation step requires green isolated
  fast tests. A separately recorded normal run must either be green or fail only
  the exact three named move tests at the known host identity-hook seed-commit
  failure. Other failures/causes or changed subsets require investigation; no
  blanket waiver or unrelated fixture repair. Setup baseline uses the same rule.
- Important 2: Ruling S1 explicitly amends d2940d0's hard sequencing and removes
  tasks-5d0d99 -> tasks-e8d471. Scope::scan already exists, and build takes captured
  rows without a future show-dependents artifact. Task 2 now depends only on Task 1.
  A coordination note was written to e8d471's unclaimed registered-root record.
- Minor 1: unit maps exercise callback selection; the Resolver local/foreign
  authority assertion is disk-backed integration with an explicit test-one run
  both before and after the refactor.
- Minor 2: Coverage input is scope-opening metadata with empty derived target sets;
  build derives/overwrites boundary_targets, unfollowed_targets and complete.
- Minor 3: blockers_pretty_contract_and_existing_reads explicitly asserts JSON's
  history-unavailable warning and complete/partial pretty headings, including n0.

Self-review checklist: spec coverage is preserved except the explicitly reviewed
S1 sequencing amendment. Types and interfaces still connect the same three tasks.
Each step states a concrete RED/GREEN or characterization test, command and gate;
all five Review Focus cases remain assigned. The plan stays proportionate: added
text specifies failure classification and the amendment rather than code bodies.
All implementation children and the ops prerequisite remain blocked for review.

Observed verification, not feature runtime tests: normal fast suite exit 101,
1725 passed, 3 failed, 4 ignored; exact known names and identity-hook cause verified.
Isolated fast suite exit 0, 1728 passed, 4 ignored. Task/document pre-commit checks
passed. Product code and the approved spec bytes are unchanged. User review of
this frozen plan, including S1, is still pending; implementation has not begun.
