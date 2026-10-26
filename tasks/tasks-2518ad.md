---
id: tasks-2518ad
title: Move integration fixtures inherit host Git hooks and fail during seed commits
status: done
priority: 2
size: s
complexity: low
process: direct
owner: fix/move-fixture-git-isolation
created: 2026-10-25T22:41:10Z
updated: 2026-10-26T15:01:18Z
started: 2026-10-26T14:56:04Z
completed: 2026-10-26T15:01:18Z
closed: 2026-10-26T15:01:18Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
origin: agent
---

The git fixture helper sets author and committer identity but inherits HOME, XDG configuration and Git routing variables. Three move integration tests fail at their seed commit because the host identity hook resolves a temporary fixture against the live registry. The focused target reproduced 87 passes and 3 failures. Build subprocess environments explicitly, isolate global/system Git config and unset inherited Git routing before fixture writes; do not bypass the host hook as a remedy.

## Notes

- 2026-10-25T22:41:10Z (feedback): created
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T02:04:07Z (feedback): feedback from tasks: Move integration fixtures inherit host Git hooks and fail during seed commits
  provenance: {"harness_session":"codex:01a175ed-7097-7802-a439-c32c236ee97d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T02:04:07Z (feedback): detail from tasks: Full baseline reproduced the same three seed-commit failures: 1740 tests passed and 3 failed when fixture Git inherited the host identity hook; isolated global and system Git configuration is being used for the comparison. No fixture remediation is included in the current cleanup.
  provenance: {"harness_session":"codex:01a175ed-7097-7802-a439-c32c236ee97d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T14:56:01Z (main): Scoped direct: root cause confirmed 2026-10-26 — tests/move.rs's private git helper inherits global git config, whose hook.identity-check runs the installed tasks resolver against the live registry; fixtures named 'dots' collide since dotfiles moved its records to the records root (dotfiles 62b206d, 2026-10-25 13:16, after the last push). Fix: route the helper through common::git_fixture with a fixture-owned HOME; common::git stays with tasks-047ad0.
- 2026-10-26T14:56:04Z (main): started
  provenance: {"harness_session":"claude-code:b9e6f0fa-77a8-4b9d-9a43-7099682b41cf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T14:56:20Z (fix/move-fixture-git-isolation): resumed
  provenance: {"harness_session":"claude-code:b9e6f0fa-77a8-4b9d-9a43-7099682b41cf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T15:01:18Z (fix/move-fixture-git-isolation): done
  provenance: {"harness_session":"claude-code:b9e6f0fa-77a8-4b9d-9a43-7099682b41cf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T15:01:18Z (fix/move-fixture-git-isolation): tests/move.rs's git helper now runs through common::git_fixture with an owned HOME and asserts repository ownership at init; move_git_fixtures_ignore_hostile_environment covers inherited selectors and a hostile global hook. common::git remains with tasks-047ad0.
  provenance: {"harness_session":"claude-code:b9e6f0fa-77a8-4b9d-9a43-7099682b41cf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
