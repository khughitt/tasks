---
id: tasks-b4df1e
title: Add a just setup recipe so a new worktree has a declared hydration route
status: doing
priority: 3
size: xs
complexity: low
process: direct
owner: chore/tasks-b4df1e-setup
created: 2026-10-27T00:34:40Z
updated: 2026-10-27T00:51:29Z
started: 2026-10-27T00:51:03Z
depends: []
parent: tasks-6ed79b
tags: [docs]
source: tasks-82be9d
agent: claude-code
origin: agent
---

The justfile defines no setup recipe, and the guides name no setup command, so each session hydrating a new worktree must decide for itself how. During tasks-82be9d that meant running cargo fetch --locked on its own judgment. The ops template (templates/justfile setup_cmd) starts with just work-link for generated directories, then installs. Here that is work-link target (the main checkout's target/ is already a work-link symlink), then cargo fetch --locked. Done when just setup exists, is idempotent, runs through tools/tt, and is named in AGENTS.md Gates; verify in a fresh worktree, followed by just test-fast.

## Notes

- 2026-10-27T00:34:40Z (add): created
  provenance: {"harness_session":"claude-code:c4bc7ff8-9393-47cf-a05c-ff61c4b7df6b","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
- 2026-10-27T00:51:03Z (main): started
  provenance: {"harness_session":"codex:01a17ad5-cdd2-7952-9bf4-bff2e7cfd4f2","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-27T00:51:29Z (chore/tasks-b4df1e-setup): resumed
  provenance: {"harness_session":"codex:01a17ad5-cdd2-7952-9bf4-bff2e7cfd4f2","harness_session_source":"CODEX_THREAD_ID"}
