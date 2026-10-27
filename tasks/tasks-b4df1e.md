---
id: tasks-b4df1e
title: Add a just setup recipe so a new worktree has a declared hydration route
status: done
priority: 3
size: xs
complexity: low
process: direct
owner: chore/tasks-b4df1e-setup
created: 2026-10-27T00:34:40Z
updated: 2026-10-27T00:58:31Z
started: 2026-10-27T00:51:03Z
completed: 2026-10-27T00:58:31Z
closed: 2026-10-27T00:58:31Z
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
- 2026-10-27T00:58:26Z (chore/tasks-b4df1e-setup): retained: {"host":"titan","kind":"worktree","path":".worktrees/tasks-b4df1e-setup","state":"kept","verification":"verified","purpose":"Immediate local integration of reviewed commit 1936e51, then remove the clean worktree; only ignored entry target/ is regenerable Cargo build output and may be discarded; tt-report ran and no host pointers were found","next_owner":"session:codex:01a17ad5-cdd2-7952-9bf4-bff2e7cfd4f2","cleanup_trigger":"After local fast-forward merge, recheck clean state and remove this worktree in this turn"}
- 2026-10-27T00:58:28Z (chore/tasks-b4df1e-setup): retained: {"host":"titan","kind":"worktree","path":".worktrees/tasks-b4df1e-verify","state":"kept","verification":"verified","purpose":"Completed pristine-worktree setup and test verification at reviewed commit 1936e51; remove after local integration; only ignored entry target/ is regenerable Cargo build output and may be discarded; tt-report ran and no host pointers were found","next_owner":"session:codex:01a17ad5-cdd2-7952-9bf4-bff2e7cfd4f2","cleanup_trigger":"After local fast-forward merge, recheck clean state and remove this worktree in this turn"}
- 2026-10-27T00:58:31Z (chore/tasks-b4df1e-setup): done
  provenance: {"harness_session":"codex:01a17ad5-cdd2-7952-9bf4-bff2e7cfd4f2","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-27T00:58:31Z (chore/tasks-b4df1e-setup): Added timed just setup using the ops work-link pattern and cargo fetch --locked; documented AGENTS.md Gates. Verified idempotent in a pristine worktree, full suite 1745 and fast suite 1741 passing, just check passing; independent review accepted without findings. Integrating locally and removing the checked worktrees in this session.
  provenance: {"harness_session":"codex:01a17ad5-cdd2-7952-9bf4-bff2e7cfd4f2","harness_session_source":"CODEX_THREAD_ID"}
