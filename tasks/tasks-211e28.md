---
id: tasks-211e28
title: "Detached records: three locations per project, [records] registry table, init --records, rename, and the typed refusals"
status: doing
priority: 1
size: l
complexity: high
process: planned
owner: records-detached
created: 2026-10-22T20:54:24Z
updated: 2026-10-23T00:48:12Z
started: 2026-10-22T21:07:05Z
depends: []
tags: []
source: tasks-dc16eb
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-22-records-home-design.md
plan: docs/plans/2026-10-22-records-detached.md
---

Implement the tasks CLI part of the approved design (tasks:docs/specs/2026-10-22-records-home-design.md §3, §5.1–§5.4, §12 cases 1–6): Project carries root, checkout, records and kind (in-tree|detached|linked); the records config declares [checkout] path and [checkout] kind = "detached"; the registry gains [records] <prefix>; locate opens a detached project from its code checkout and worktrees through the registry and refuses an in-tree config whose prefix is paired elsewhere (records_moved), a registered root with no config and no [records] row (records_unpaired), and init --records while the config is in the working tree or index (records_in_tree; a staged deletion is allowed). init --records takes the records root and derives <records>/<prefix>; rename moves that directory and the [records] key. projects, where and resolve rows gain records and kind, checkout is always present, resolve gains via records. Path references resolve records then root. The no-write test gains the detached case. CLI rows land on ops cli.toml first. Release order: the ops linked.py adapter piece must be installed on both hosts before this lands, since it rejects the new where shape.

## Notes

- 2026-10-22T21:07:05Z (main): started
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T21:07:23Z (records-detached): resumed
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T21:25:53Z (records-detached): Plan drafted 2026-10-22 (docs/plans/2026-10-22-records-detached.md, revision 1, 31e29ee): 10 tasks after the ops CLI row; seven decisions beyond the spec listed at its top for the reviewer (root from the config, fs::rename for the records directory, reachable-row-only output keys, where's refusal inside git only, toml_edit for [checkout], plain init refuses records_moved, InitOut.records). tasks docs (§5.5) stays tasks-a8e28e and skill wording tasks-00723a.
- 2026-10-22T21:25:54Z (records-detached): parked (waiting on user, review): User reviews the implementation plan at commit 31e29ee on branch records-detached (.worktrees/records-detached/docs/plans/2026-10-22-records-detached.md; read it with git show 31e29ee:docs/plans/2026-10-22-records-detached.md) and rules on its seven listed decisions; then the agent revises as a new commit, or links the plan (tasks edit tasks-211e28 --plan 2026-10-22-records-detached), files the step children per its Tracking section, runs the just test baseline, and executes subagent-driven
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T21:38:43Z (records-detached): resumed
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T21:38:45Z (records-detached): review: plan round 1 — verdict: revise; findings: P1 2, P2 6; reviewer: codex/gpt-6-astra
- 2026-10-22T21:43:48Z (records-detached): plan round 1 fixes (revision 2, 2d70542): rename keeps identity (root) and current records directory apart in Invocation, Inventory (records_from/records_to), Snapshot (RecordsDir, records_old/new) and classify; fixtures build git env with env_clear and assert the fixture repo before writes, TestEnv drops GIT_* except GIT_CEILING_DIRECTORIES; effective_retirement, reject_retired, check and retire::authority compare config_dir with records; init --records on an existing config sets [git] commit = true and carries validate_checkout's overlap checks; projects rows always carry records, checkout/kind when the config was read (decision 3 revised); Task 2 spec test commits the spec before edit; just test after Task 9 and on the branch head; D4 and D6 costs documented.
- 2026-10-22T21:43:50Z (records-detached): parked (waiting on user, review): User reviews plan revision 2 at commit 2d70542 on branch records-detached (.worktrees/records-detached/docs/plans/2026-10-22-records-detached.md; read it with git show 2d70542:docs/plans/2026-10-22-records-detached.md); then the agent revises again as a new commit, or links the plan (tasks edit tasks-211e28 --plan 2026-10-22-records-detached), files the step children per its Tracking section, runs the just test baseline, and executes subagent-driven
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T21:51:12Z (records-detached): resumed
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T21:51:14Z (records-detached): review: plan round 2 — verdict: revise; findings: P1 1, P2 1, minor 2; reviewer: codex/gpt-6-astra
- 2026-10-22T21:52:57Z (records-detached): plan round 2 fixes (revision 3, bea738c): TestEnv cmd/raw and the fixture git helper start from env_clear with PATH, HOME, USER, LC_ALL, GIT_CONFIG_GLOBAL=/dev/null, GIT_CONFIG_NOSYSTEM, GIT_AUTHOR_*/GIT_COMMITTER_*, and GIT_CEILING_DIRECTORIES/TMPDIR copied, via one shared isolate(); rename invocation looks records up by canonical_prefix(source) with a replay/explain-after-completion test; retirement test no longer expects retire from the code checkout to refuse; decision 3 amends spec §5.2 in Task 10; stale rename-root wording removed.
- 2026-10-22T21:52:59Z (records-detached): parked (waiting on user, review): User reviews plan revision 3 at commit bea738c on branch records-detached (.worktrees/records-detached/docs/plans/2026-10-22-records-detached.md; read it with git show bea738c:docs/plans/2026-10-22-records-detached.md); then the agent revises again as a new commit, or links the plan (tasks edit tasks-211e28 --plan 2026-10-22-records-detached), files the step children per its Tracking section, runs the just test baseline, and executes subagent-driven
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T22:11:05Z (records-detached): resumed
  provenance: {"harness_session":"claude-code:ccb226ca-8960-426d-9504-c547cf14a4ac","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-22T22:11:07Z (records-detached): review: plan round 3 — verdict: accept; findings: none; reviewer: codex/gpt-6-astra
- 2026-10-22T22:11:08Z (records-detached): Plan approved by the user 2026-10-22 at revision 3 (bea738c). Linking the plan, filing step children, running the just test baseline, then subagent-driven execution.
- 2026-10-22T22:13:28Z (records-detached): baseline: just test in .worktrees/records-detached at 3030425 before any code change: 1609 passed, 0 failed, 13 test binaries, ignored included, rc 0. Step children filed: tasks-466f18 (T1) … tasks-a3c8a8 (T10), chained by dependency.
- 2026-10-23T00:03:13Z (records-detached): Execution 2026-10-22: all ten plan tasks landed on records-detached at 4eeb456 (16 code/doc commits), each with a fresh task review; three fix rounds (T2 evidence, T3 move guard, T8 lazy resolver walk) and one own-concerns round (T5). just test 1652 passed at Task 9. Final whole-branch review dispatched; ledger at .superpowers/sdd/2026-10-22-records-detached/progress.md in the worktree.
- 2026-10-23T00:48:12Z (records-detached): attached: sdd-ledger.md (21076 bytes): SDD execution ledger: pre-flight scan, baseline, per-task reviews, fix rounds, every ruling and deferred minor
