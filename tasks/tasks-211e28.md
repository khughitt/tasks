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
updated: 2026-10-22T21:51:12Z
started: 2026-10-22T21:07:05Z
depends: []
tags: []
source: tasks-dc16eb
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-22-records-home-design.md
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
