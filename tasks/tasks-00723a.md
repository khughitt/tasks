---
id: tasks-00723a
title: "Agent-docs wording in the tasks, scope and curate skills: documents under the records root"
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: docs/tasks-00723a
created: 2026-10-10T23:02:26Z
updated: 2026-10-25T12:03:34Z
started: 2026-10-25T11:44:49Z
depends: []
tags: []
source: "hq:docs/specs/2026-10-10-agent-docs-collection-design.md"
agent: claude-code/claude-fable-5-1
---

Re-scoped 2026-10-22 by tasks-dc16eb (tasks:docs/specs/2026-10-22-records-home-design.md §7.1, §8). The skills keep --spec docs/specs/<name>.md and --plan as they are (they resolve under records); the scope skill's brief lands under <records>/<prefix>/docs/notes/; the curate skill's fixable link may point under records; the plan-snapshot and mindful-op wording is dropped. Lands with the tasks CLI piece.

## Notes

- 2026-10-22T20:56:05Z (records-home): Re-scoped 2026-10-22: mindful capture and plan-snapshot wording superseded by tasks-dc16eb's design.
- 2026-10-23T01:04:43Z (main): Scope note 2026-10-22 from tasks-211e28 (landed, main 85b90f3): the tasks skill should also name tasks init --prefix <p> --records <records-root>, the [records] registry table, the three locations and kind in projects and where (an unreachable projects row carries records but no checkout or kind), records_moved, records_unpaired and records_in_tree with their remedies, that rename moves the records directory and leaves it for the user to commit, and that rename --adopt pairs a directory another host renamed. README.md Linked and detached projects has the wording to draw from.
- 2026-10-25T11:44:49Z (main): started
  provenance: {"harness_session":"codex:01a172e0-035d-7420-9c13-60fe7e644221","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T11:45:19Z (docs/tasks-00723a): resumed
  provenance: {"harness_session":"codex:01a172e0-035d-7420-9c13-60fe7e644221","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T11:46:13Z (docs/tasks-00723a): Direct documentation scope: update shipped tasks/scope/curate skills for landed detached records support; preserve stored docs paths, distinguish checkout evidence from records writes, cover init/JSON/errors/rename, and remove config-walk assumptions. Source: approved records-home design c03869d and original human turn recovered from its session. Workspace .worktrees/tasks-00723a; hydrate with confined README cargo install, then full-suite baseline before edits.
- 2026-10-25T11:48:29Z (docs/tasks-00723a): Worktree hydration succeeded with confined cargo install --locked --path . --root .local and CARGO_TARGET_DIR=target. Baseline just test exited 0; full result counts recorded next. Reference baseline found detached init/locations/errors absent and unscoped scope/curate unable to locate detached code worktrees. Updating existing reference wording, not adding new workflow policy.
- 2026-10-25T11:49:53Z (docs/tasks-00723a): Baseline just test: 1685 passed, 0 failed, 0 ignored across 14 test targets. Reference-only documentation update reaches skills/tasks/SKILL.md, skills/scope/SKILL.md and skills/curate/SKILL.md; no parser, callers, fixtures or JSON shapes changed.
- 2026-10-25T11:53:27Z (docs/tasks-00723a): review: impl round 1 — verdict: revise; findings: Important 1, Minor 1; reviewer: codex/gpt-6.1-sol
- 2026-10-25T11:54:15Z (docs/tasks-00723a): Reference review round 1 corrections: broaden skill activation to config-free detached/linked checkouts and use init --records --force when recording detached checkout locations. Fast-suite attempt exposed existing doc_fetch 150ms-deadline fixture failures: timeout_cleans_up_child_and_pipe_holder and escaped_pipe_holder_cannot_extend_deadline both lacked pidfiles; full baseline had passed. Investigating scheduling contention with focused rerun; no Rust changes.
- 2026-10-25T11:55:27Z (docs/tasks-00723a): The doc_fetch unit tests passed on focused serial rerun with unchanged Rust code. The fast-suite failure coincided with host load 135.31 on 32 CPUs and missing pidfiles after 150ms child-start deadlines. Filed tooling friction tasks-16ebe8; rerunning the complete non-ignored suite with RUST_TEST_THREADS=1 (no selection or deadline changes).
- 2026-10-25T12:00:03Z (docs/tasks-00723a): review: impl round 2 — verdict: revise; findings: Important 2; reviewer: codex/gpt-6.1-sol
- 2026-10-25T12:01:30Z (docs/tasks-00723a): Corrective round 1 for final review: unscoped code-worktree passes keep that worktree as tasks -C context, preserving detached retained-doc resolution; scoped passes read code/history from each selected project's checkout. Complete serial just test-fast exited 0: 1681 passed, 0 failed, 4 ignored across 14 targets. Existing focused retained-doc resolver case passed 1/1 after wording corrections. No Rust changes or new workflow policy.
- 2026-10-25T12:03:34Z (docs/tasks-00723a): review: impl round 3 — verdict: revise; findings: Important 1; reviewer: codex/gpt-6.1-sol
