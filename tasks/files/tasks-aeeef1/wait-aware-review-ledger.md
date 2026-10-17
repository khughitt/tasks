# SDD ledger — plan: docs/plans/2026-10-17-wait-aware-review.md

Spec: docs/specs/2026-10-17-wait-aware-review-design.md (accepted, review round 2).
Steps: T1 tasks-c37991, T2 tasks-83ea67, T3 tasks-f6461c, T4 tasks-e136d5, T5 tasks-a84c07, T6 tasks-7c094d, T7 tasks-370a50; goal tasks-aeeef1.
Setup: rebased feat/wait-aware-review onto main f177385 before Task 1 (no file overlap; plan asks for it before Task 2). Baseline `just test-fast` run after rebase.

## Pre-flight scan

| Pair / task | Produces → consumes | Finding |
|---|---|---|
| T1 → T3 | ops cli.toml rows (`--awaiting-user` on note+edit, `--no-awaiting-user` on edit) → `vendored adopt` + clap args | consistent (names, value = none) |
| T2 → T3 | `Task::{awaiting_user,reviewed_at,closed}`, `review::not_reviewed`, check_invariants refusals → `apply`, `stamp_edited` | T3 editor test drops `reviewed:` but leaves `reviewed_at:`; passes only if `stamp_edited` runs before `review::validate` on the edited task (validate refuses reviewed_at without reviewed). Ruling 3. |
| T2 → T4 | fields, `closed` stamped on close → `Deps::of_scan` reads `found.closed` | consistent |
| T2 → T5 | `seed`, `restamp`, `task`, `check_kinds` test helpers in tests/review.rs → T3/T5 tests use `seed`, `restamp`, `task` | consistent; `seed_reviewed`, `editor_script`, `id_of` already exist |
| T3 → T4 | `--awaiting-user` writer → T4 Step 7 fixture edits | consistent (order T3 before T4) |
| T4 → T5 | `review_reason` on TaskSummary/ParkedRow, per-scan views → T5 view tests | consistent; T5 expects most tests green on arrival |
| T4 ↔ sample.rs | `proposal::pending` replaces `pending_proposal` | tasks-22f407 still todo: no concurrent helper on main |
| T5 tests/cli.rs | `repo_with_worktree(env) -> (main, side, dep)`, `env.claim_store(prefix)` | both exist |
| T2–T5 → T7 | `list --needs-review` review_reason, `edit --awaiting-user`, `dep --on`, `claims` → tools/wait-backfill | consistent; copies helpers from tests/review_backfill.rs and tools/review-backfill (exist) |
| T6 | docs only, reads behaviour of T2–T5 | consistent |
| T1 self | worktree, edit 3 rows, ops `just test-fast`, merge | plan says ops has no `setup`; ops justfile now defines one. Ruling 2. |
| T2 self | format test + review unit + 4 CLI tests + move test vs fields/check/transition/tombstone | self-consistent |
| T3 self | tests vs apply/stamp_edited/show rows | self-consistent except ordering above |
| T4 self | unit tests vs state/Deps; Step 6 expects named fixture failures | self-consistent |
| T5 self | RED only on `awaiting_user` row field | self-consistent |
| T7 self | tests vs tool contract (exit codes 0/1/2, snapshot shape) | self-consistent; ENTRY_KEYS includes `unresolved`, which the Interfaces entry list omits but describes below it — not a conflict |

Ruling: Execute Tasks 1–7 and stop before the plan's Rollout — Rollout merges to main, upgrades every host and writes real records across projects, which the plan itself gates on the user — cost if wrong: the user runs Rollout one session later.
Ruling: Task 1 runs ops's `just setup` after `git worktree add` (plan says ops defines none; its justfile now does, and global rules require it) — cost if wrong: a few seconds of setup.
Ruling: Task 3 editor path runs `review::stamp_edited` before any `review::validate` of the edited record (plan places it directly after check_invariants; the plan's own test needs that order) — cost if wrong: none beyond the test the plan wrote.
Ruling: Task 1's ff-merge into ops main and worktree removal are done by the controller after the task review passes, not by the implementer — keeps the outward merge behind review — cost if wrong: none.

## Progress
Baseline: just test-fast → 561 passed, 18 failed, all `record_commit::tests::*` (tasks-eeeeda HOME leak, host git hook). 
Ruling: Treat exactly those 18 record_commit::tests failures as the known baseline in every "just test-fast: PASS" step; any other failure is real — the fix for tasks-eeeeda is out of this plan's scope — cost if wrong: a record_commit regression from this branch could hide among them (this plan does not touch src/record_commit.rs).
Task 1: complete (ops 69cd6f7..906439b, ff-merged to ops main by controller, worktree removed; review clean — task-1-review-report.md)
Task 2: dispatched (BASE 75c519f, implementer a62198005587578cb sonnet)
Ruling: The suite stand-in for "just test-fast" is `just test-one --no-fail-fast` (test-fast stops at the 18 baseline unit failures, so integration binaries never ran in it, nor in the baseline); pass = only the 18 record_commit::tests failures — cost if wrong: none, it runs a superset.
Task 2: minor (deferred): RED observed only for the format test; check/editor/transition/move tests were written with the code and never seen failing
Task 2: minor (deferred): check.rs finding text for awaiting_user-without-reviewed differs from review::not_reviewed's wording (plan-mandated texts)
Task 2: minor (deferred): no editor test for a hand edit of awaiting_user (validate covers it at write time)
Task 2: complete (commits 75c519f..797e9c4, review clean)
Task 3: dispatched (BASE 1b5c79a, implementer a529a1a9fbea8a991 sonnet). Ruling 3 check: review::validate runs at save (repo.rs), after check_invariants — plan's stamp_edited placement suffices.
Ruling: Task 3's vendored adopt carries ops main's other rows that landed since Task 1 (+15/-3, other tools' rows) unedited — adopt vendors the whole table and AGENTS.md forbids hand edits — cost if wrong: an unrelated table change rides in this commit.
Task 3: minor (deferred): show_names_a_reviewed_idea assertion loosened to contains("yes (") (date pinned elsewhere)
Task 3: minor (deferred): show test's awaiting-row assertion is not line-anchored
Task 3: complete (commits 1b5c79a..78c5605, review clean)
Task 4: dispatched (BASE 23cc360, implementer a1ab724033ffffd2d opus — high complexity, multi-file threading)
Ruling: Deps::of_scan propagates resolve_dependency errors (malformed dependency record or other-project config) instead of treating them as open with a warning — spec §4.1's 'unresolvable counts as open' covers Ok(None) (unreachable, broken/cyclic chain), ready already propagates the same Err (list.rs:261), and the user's rules say fail early, no silent fallbacks — cost if wrong: a broken record that only a reviewed idea depends on now fails ready/prime/lanes/tree/sample until repaired (check names it).
Task 4: minor (deferred): prime's doing-row elsewhere_deps uses `?`, so a foreign checkout's view error fails all of prime, unlike claimed_elsewhere / parked recorded rows which warn and fall back (plan-mandated code)
Task 4: minor (deferred): parked sourced_rows builds a second view of `all` in prime/next (spec §4.3 says once per read) — pass deps into parked::rows
Task 4: minor (deferred): list resolves every reviewed idea's edges in the scan, then the unreachable-warning loop resolves listed rows again — cost only
Task 4: complete (commits 23cc360..097db5b, review clean)
Task 5: dispatched (BASE 3679399, implementer aa2cd21da0ae18b85 sonnet)
Task 5: minor (deferred): spec §8 case 6 variant (woken when an edge closed after review, after a plain note answers the proposal) has no test
Task 5: minor (deferred): spec §8 case 12: prime's non-parked rows not asserted for woken/stranded in the view-agreement test
Task 5: minor (deferred): comment in stranded_ideas_and_the_review_time_test ("Closed before the review: stranded") misdescribes the assertions
Task 5: complete (commits 3679399..8294292, review clean)
Task 6: dispatched (BASE 79b57a7, implementer a655cc416a40dbb73 haiku — exact text in plan)
Task 6: minor (deferred): new prose not hard-wrapped like its neighbours: skills/scope/SKILL.md:28, :200; skills/curate/SKILL.md:105; the Amended 2026-10-17 line in docs/specs/2026-10-11-review-state-design.md
Task 6: complete (commits 79b57a7..a12843f, review clean)
Task 7: dispatched (BASE 01c4689, implementer a04d3b2f1fef693b6 sonnet)
Task 7: Ruling: fix the plan-mandated test that asserts nothing (rerun ... reads_no_notes: later note must name an open task) and add review_reason to apply()'s changed detail — spec §7.3 and acceptance 14 are the authority over the plan's code — cost if wrong: one extra field in a one-time tool's report.
Task 7: minor (deferred): corrupt-snapshot test never RED; could also assert sci.json still reads {not json
Task 7: minor (deferred): non-Usage errors (RuntimeError from read_json, UnicodeDecodeError in load) traceback with exit 1, not JSON/exit 2 (inherited from review-backfill)
Task 7: minor (deferred): snapshot save has no fsync of file/dir before rename (fails safe: next run exits 2)
Task 7: minor (deferred): operator-written dep targets only type-checked, not matched against the id regex
Task 7: minor (deferred): no test for a multi-target {"dep": [a, b]} decision or an interrupted edit --awaiting-user
Task 7: fix round 1/5 (1 addressed, 1 open — review_reason changed-detail added in tool but no test assertion, report claimed one; commits 5c29577..3316122; re-reviewer claude-code/claude-sonnet-5-5; new Minor: list --needs-review read per changed entry can raise RuntimeError → crash, retried next run)
Task 7: minor (deferred): changed-entry review_reason lookup re-reads list --needs-review per entry and a failure there crashes the pass (entry retried next run)
Task 7: fix round 2/5 (1 addressed, 0 open — review_reason assertion added; commits e6084e3..5c01453 pre-rebase, 83ad200 post-rebase; re-reviewer claude-code/claude-haiku-5-5)
Task 7: complete (commits 01c4689..5c01453 pre-rebase, review clean after 2 fix rounds)
Setup: rebased onto main 7af1722 before the final review (main moved by two task records only).
Final review: dispatched (base 7af1722, head f57b32e, reviewer a36eda3c6f96bc435 opus; full suite green on this head via just test-one --no-fail-fast)
Final review: Critical 1 (rename rewrite quotes reviewed_at/closed → unparseable records), Important 0, Minor 4; all 18 deferred minors triaged keep-deferred.
Final: minor (deferred): review::apply Review::Set keeps a stale awaiting_user on a re-review (spec-compliant; needs a spec decision)
Final: minor (deferred): plain-note warning and scope skill §1 wording are wrong when the note answers a pending proposal (it does change the wait)
Final: minor (deferred): wait-backfill freeze "changed" detail says "rerun after the next pass" though a rerun never re-freezes
Final: minor (deferred): src/commands/lanes.rs:564-567 doc comment stale under the error-propagation Ruling
Final: corrective round 1 dispatched (FIX_BASE 0cf8455, fixer abd8eab938bacb15f sonnet) for the rename Critical
Final: corrective round 1/5 (1 addressed, 0 open — rename rewrite uses shared TIMESTAMP_KEYS; commit c2b7095; re-reviewer claude-code/claude-sonnet-5-5). Final review clean.
Finish: suite on c2b7095 via just test-one --no-fail-fast: 1529 passed, 0 failed; just check exit 0. Deferred minors filed as tasks-d2cdfe. Integration choice presented to the user (merge = Rollout step 1; skills go live with the merge).
