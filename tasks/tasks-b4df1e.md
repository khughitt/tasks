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
updated: 2026-10-27T00:57:44Z
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
- 2026-10-27T00:51:43Z (chore/tasks-b4df1e-setup): Scope: direct, two-file change (justfile and AGENTS.md Gates), following the source task’s declared work-link target plus cargo fetch --locked sequence. Hydrated .worktrees/tasks-b4df1e-setup with that sequence; existing just setup is absent. Baseline command: just test; running before edits.
- 2026-10-27T00:55:15Z (chore/tasks-b4df1e-setup): Verification: full baseline just test passed 1745 tests; after recipe/doc change, just setup succeeded twice, Cargo.lock stayed unchanged, just test-fast passed 1741 tests (4 ignored), and just check passed. Code commit 1936e51. Independent read-only review dispatched. A second pristine worktree will verify setup before any build output exists.
- 2026-10-27T00:57:42Z (chore/tasks-b4df1e-setup): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-27T00:57:44Z (chore/tasks-b4df1e-setup): Fresh-worktree acceptance: .worktrees/tasks-b4df1e-verify started at 1936e51 with target absent; just setup succeeded twice, Cargo.lock unchanged; just test passed 1745 tests; subsequent just test-fast passed 1741 tests (4 ignored). Review accepted with no findings or declined-to-judge behaviors. No CLI or Rust source changed; no reinstall needed. Proceeding with local integration and checked cleanup.
