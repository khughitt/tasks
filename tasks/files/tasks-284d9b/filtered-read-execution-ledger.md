# SDD ledger — plan: docs/plans/2026-10-25-filtered-read-routing-plan.md

Approved plan: 3a9752f, Keith accepted for inline execution with three reviewed minor recommendations.
Spec: a92a91a. Workspace: .worktrees/tasks-284d9b; parent tasks-284d9b owns retained resources.
Task 1: tasks-3b3f0b; Task 2: tasks-92cf3a.
Pre-flight: Task 1 produces the positional-ID routing policy; Task 2 consumes that policy via existing local_or_foreign, no new interface mismatch.

Ruling: accept plan-review Minor 1; locate local project before loading the registry, propagate non-NoProject errors, then parse and compare filters before routing. Why: match positional-ID error ordering and spec rule 2. Cost: dual-corruption detail names local config instead of the registry as drafted. Revision: approved 3a9752f adjusted during execution.
Ruling: accept plan-review Minor 2; smoke the confined installed binary from a temporary outside cwd against main-visible IDs plus source error/recovery. Why: exercise the new route rather than only the existing local path. Cost: registered main may have fewer descendants than the branch; compare with main instead of branch-only children. Revision: Task 1 Step 8 adjustment to 3a9752f.
Ruling: accept plan-review Minor 3; add the worktree-only leaf via tasks add with record commits disabled, and enroll the new linked fixture cases in the hostile-parent wrapper. Why: no Git write is needed in a linked worktree; existing strict main-repository assertions remain valid. Cost: fixture checks uncommitted worktree records only, matching the intended precedence case. Revision: Task 1 Step 3 adjustment to 3a9752f.

Setup: existing worktree isolation and hydration verified. tasks-e147b9 rechecked: still todo with no reproduced finding, so no discovery policy change.
Plan review numbering: incoming claude-code/claude-opus-5-5 acceptance is round 2; controller self-review already recorded round 1.
Todo: Task 1 in progress; Task 2 pending.

Baseline: just test exited 0; 1685 passed, 0 failed, 0 ignored across 14 binaries.
Task 1 RED: 6 CLI tests failed on missing routing/guidance; 2 linked tests failed on outside/foreign routing after correcting a TOML fixture insertion. Marker control already passed.
Task 1 GREEN: 6 CLI + 2 linked cases passed. Corrected two test expectations to existing contracts (parked list uses tasks; local config errors name relative tasks/.config.toml).
Controls: bare_suffixes_ 2, project_scope_ 4, positional outside routing 1, marker 1, hostile Git fixture wrapper 1 all passed.
Ruling: retain the positional resolver unchanged and validate a separate original context only when both ID filters are present. Why: one-ID reads reuse the existing path directly and the small filtered opener avoids a broader resolver refactor. Cost: combined filters perform a second discovery/registry lookup before opening their target. Revision: Task 1 routing commit.
Task 1: complete (commits fa14ae5..634cf8d, tests: sh -c 'just test-one --test cli filtered_read_ && just test-one --test linked filtered_read_' → test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out; finished in 0.94s)

Task 1: release install and live smoke passed; outside route matches registered main (0 descendants), source-only no_project guidance and all-projects recovery (0 matches, 0 warnings). Fast 1689 passed, 0 failed, 4 ignored; hook checks passed after fixing four cloned-ref lint findings in tests. Child tasks-3b3f0b done.
Task 2: BASE f4ac367; new foreign completion test and updated tree test RED (empty candidates), explicit/local boundary control already green. After using local_or_foreign in shared completion, both new cases and all 16 completion_ cases GREEN. Fast 1691 passed, 0 failed, 4 ignored. No new routing policy or deviations.
Task 2: complete (commits f4ac367..d8c147a, tests: just test-one --test cli completion_ → test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 684 filtered out; finished in 1.30s)

Final release refreshed after Task 2. Installed outside-ID route matches main and outside typed-prefix completion offers tasks-284d9b.
Final review: codex/gpt-6-astra accepted frozen bde3079 against 998e6a4 with 0 Critical, 0 Important, 0 Minor; full returned report in final-review-report.md, attached to parent. No corrective round needed and no minor (deferred) findings.
Final: Ruling: leave the empty-ancestor Git-marker attachment incident to tasks-e147b9. Why: the approved spec excludes that investigation and preserves marker errors. Cost: the original scratch-directory incident remains unresolved until its owning task completes. Revision: bde3079 final review, declined-to-judge item 1.
Final: Ruling: preserve existing group/all-projects registry_warnings discovery. Why: this change retains explicit-scope behavior and does not revise warning collection. Cost: those wide reads may still fail on malformed local cwd configuration. Revision: bde3079 final review, declined-to-judge item 2.
Integration: local merge into main and reinstall authorized by Keith's accepted plan; no push or PR. Test logs and extracted briefs are regenerable execution output; durable verification summary, full review reports and ledgers will be attached before removal. Both implementation children are done; parent owns integration and cleanup.
