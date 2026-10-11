---
id: tasks-bcfe85
title: "Approve installation, verify both hosts and close the task"
status: done
priority: 1
size: s
complexity: mid
process: direct
owner: feat/tasks-91f63b-external-docs
created: 2026-10-11T13:33:18Z
updated: 2026-10-11T16:52:42Z
started: 2026-10-11T16:02:28Z
completed: 2026-10-11T16:52:42Z
depends: [tasks-b75aee]
parent: tasks-91f63b
tags: []
agent: codex
spec: docs/specs/2026-10-11-external-document-references-design.md
plan: docs/plans/2026-10-11-external-document-references.md
step: "Task 7: Approve installation, verify both hosts and close the task"
---

## Notes

- 2026-10-11T16:02:28Z (feat/tasks-91f63b-external-docs): started
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T16:04:51Z (feat/tasks-91f63b-external-docs): parked (waiting on user, review): Keith: approve the pending Titan+Europa live-install request; then Codex installs reviewed fef13d3 artifact /tmp/tasks-doc-install.tntNMX/bin/tasks (sha256 b150cb229f76819a38b5d709d5baeefc776cd1be678131e986db225829b29fdb), backs up each ~/.cargo/bin/tasks, runs isolated host smokes, hands proof to ops-721511, and closes/integrates the task.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T16:45:08Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T16:46:01Z (feat/tasks-91f63b-external-docs): Keith approved the concrete Titan+Europa binary replacement. Reviewed artifact fef13d3 SHA-256 b150cb229f76819a38b5d709d5baeefc776cd1be678131e986db225829b29fdb; backing up each ~/.cargo/bin/tasks as tasks.before-tasks-91f63b-20261011. Restore with install -m 755 backup ~/.cargo/bin/tasks (remove [docs.schemes] first if ops has since configured it). No host pointer enters the worktree.
- 2026-10-11T16:50:05Z (feat/tasks-91f63b-external-docs): Both hosts now resolve tasks to standalone ~/.cargo/bin/tasks with reviewed fef13d3 SHA-256 b150cb229f76819a38b5d709d5baeefc776cd1be678131e986db225829b29fdb. Version/help plus isolated export/filter and no-scheme read/edit/show/next/check passed. Prior binaries retained beside executable as tasks.before-tasks-91f63b-20261011; no scheme table or real external link written.
- 2026-10-11T16:52:42Z (feat/tasks-91f63b-external-docs): done
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T16:52:42Z (feat/tasks-91f63b-external-docs): Approved reviewed artifact installed on titan and europa at ~/.cargo/bin/tasks; same SHA-256 verified, export/filter/no-scheme and provider-outage smokes passed, backups retained, clean corpus preflights. Installation proof and binary/config/real-wrapper pilot gates handed to ops-721511; external workflow activation remains there.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
