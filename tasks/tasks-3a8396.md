---
id: tasks-3a8396
title: Address the blockers report's deferred review minors
status: doing
priority: 3
size: s
complexity: low
process: direct
owner: fix/tasks-3a8396
created: 2026-10-26T01:55:16Z
updated: 2026-10-26T02:18:07Z
started: 2026-10-26T01:59:24Z
depends: []
tags: [cli]
source: tasks-e637a1
agent: claude-code/claude-opus-5-5
origin: agent
---

Why: the final review of tasks blockers (tasks-e637a1, merged 074672f) left eight Minor findings deferred; they are recorded only in tasks-e637a1's execution-ledger.md and review report attachments. Separately, [ref:spec] still says "Status: proposed for user review" although it was approved and shipped.

Done:
- blockers --help states that blocking-since and confirmed-pass metrics are unavailable (spec: stated in warnings and help); check parser_surface_equals_table compares only the summary.
- unresolved diagnostics keep every stored spelling of a missing reference (spec: retain original reference IDs) while counts stay per canonical id; update the single-diagnostic expectation in blockers_identity_and_lifecycle with it.
- one scan-first lookup helper shared by commands/blockers.rs and list::resolve_dependency, ideally map-backed.
- isolated_worktree, write_claim and own_pid_view move into tests/common instead of being copied between tests/cli.rs and tests/blockers.rs.
- pretty assertions tied to their row, the footer check asserts "history: blocking since" rather than "unknown", and the 120-column width check also runs on the fuller label fixture.
- the spec's status line records approval and shipment (decision record, not rewritten as reference).
Verification: just test-one --test blockers and blockers_ green, just test-fast; JSON shapes unchanged except added diagnostic rows.

## References

- spec (context): docs/specs/2026-10-25-blocker-ranking-design.md — approved design whose status line still reads proposed

## Notes

- 2026-10-26T01:55:16Z (add): created
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
- 2026-10-26T01:59:24Z (main): started
  provenance: {"harness_session":"codex:01a175ed-7097-7802-a439-c32c236ee97d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T02:00:07Z (fix/tasks-3a8396): resumed
  provenance: {"harness_session":"codex:01a175ed-7097-7802-a439-c32c236ee97d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T02:01:46Z (fix/tasks-3a8396): Process direct; workspace .worktrees/tasks-3a8396 on fix/tasks-3a8396. Verified source final-review-report.md and rereview-round-1-report.md against code: eight minors map to help, diagnostic spellings, scan-first lookup, shared fixture helpers and row/width/footer assertions; spec still carries proposed status. Surface audit reads get_about only, so long_about needs no vocabulary change. Share the existing linear scan lookup rather than add an index or change all picker callers; canonical counts remain unchanged, only diagnostic rows expand. Confined README cargo install completed; full just test baseline running before edits.
- 2026-10-26T02:03:56Z (fix/tasks-3a8396): Baseline normal just test: exit 101, 1740 passed/3 failed/0 ignored. Exact known tasks-2518ad failures: correction_i1_move_stamp_keeps_identical_sibling_stale, move_from_a_source_worktree_refuses, move_explain_classifies_source_worktree_refusal; all at tests/move.rs:1717 seed git commit refused by inherited host git-identity-check (records_moved). No code edits. Isolated GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 just test running to establish clean full baseline.
- 2026-10-26T02:05:39Z (fix/tasks-3a8396): Clean full baseline: GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 just test exit 0, 1743 passed/0 failed/0 ignored. Normal baseline limitation reproduced and recurred on tasks-2518ad. First regression changes assert unavailable history metrics in rendered blockers --help and both canonical/alias missing-reference diagnostic rows while keeping own unresolved count 1.
- 2026-10-26T02:10:32Z (fix/tasks-3a8396): Implementation covers all eight deferred minors plus approved/shipped spec status. RED: blockers_identity_and_lifecycle failed on missing canonical-spelling diagnostic; blockers_cli_surface_and_limits failed on missing history help. GREEN: blockers_ 10 passed; --test blockers 4 passed; dependency_resol 3 passed; parser_surface_equals_table 1 passed. Row-association mutation moved needs onto another row and failed the intended assertion; renderer restored byte-identically. Final isolated just test-fast exit 0, 1739 passed/4 ignored. JSON keys unchanged; only unresolved diagnostic rows expand. Shared scan_lookup intentionally keeps current linear complexity; map indexing is optional and outside this cleanup.
- 2026-10-26T02:17:29Z (fix/tasks-3a8396): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-26T02:17:32Z (fix/tasks-3a8396): attached: cleanup-review-report.md (3128 bytes): Full independent whole-branch review of 8cd0814 by codex/gpt-6.1-sol: accepted, no findings; saved validation evidence inspected, suites not rerun by reviewer
- 2026-10-26T02:17:34Z (fix/tasks-3a8396): attached: review-ledger.md (1198 bytes): Controller dispositions R1-R4 for review exclusions at 8cd0814, each with reason and cost; no deferred minors or corrective rounds
- 2026-10-26T02:18:07Z (fix/tasks-3a8396): retained: {"host":"titan","kind":"worktree","path":".worktrees/tasks-3a8396","state":"kept","verification":"verified","purpose":"Reviewed cleanup 8cd0814 ready for immediate local merge, CLI reinstall and removal in this turn. Only ignored target/ (regenerable builds, confined install and repeatable test/mutation logs) and .superpowers/ (review report and ledger attached byte-identically) require disposition. No submodules or host pointers; tt-report ran. Attached review files verified SHA-256 0ba1d5d653220fc6224aa27c6cf2faf56f027c0530e22fa294c844151bb3f747 and 565eada224e7073fdae55d4c0f194e1e080181027458b53152f2ff3265449826. No off-tree evidence or task scratch directory created.","next_owner":"session:codex:01a175ed-7097-7802-a439-c32c236ee97d","cleanup_trigger":"After authorized local integration and reinstall, confirm merged clean branch, attachments, ignored-file dispositions and no host pointers; remove this task worktree and branch"}
