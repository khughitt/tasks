---
id: tasks-047ad0
title: Isolate the shared Git helper used by CLI integration fixtures
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: fix/tasks-047ad0
created: 2026-10-25T12:30:00Z
updated: 2026-10-26T15:22:29Z
started: 2026-10-26T15:12:35Z
completed: 2026-10-26T15:21:17Z
closed: 2026-10-26T15:21:17Z
depends: []
tags: [testing]
source: "tasks-284d9b:shared-git-fixture-follow-up"
agent: codex
---

Why: while implementing tasks-284d9b, code inspection at a8bbd4d found tests/common/mod.rs::git still launches std::process::Command::new("git") with the inherited environment, overriding only author/committer identity. GIT_DIR, GIT_COMMON_DIR, GIT_WORK_TREE and GIT_INDEX_FILE can redirect fixture writes, and host HOME/XDG config or credentials can reach subprocesses. This is a concrete code finding, not a reproduced host incident. The helper has 113 call sites in tests/cli.rs and is also used in tests/refs.rs. Earlier tasks-171b8b isolated only linked.rs and record_commit.rs fixtures; its accepted boundary is recorded in [ref:prior-review]. Existing tasks-79a907 covers a different, already-isolated record-commit unit-test runner.

Scope: isolate the shared common::git helper and adjust only callers that need explicit fixture context or variables. Reuse tests/common/mod.rs::isolated_env and tests/common/git_fixture.rs patterns: key-by-key environment, temporary fixture-owned HOME, GIT_CONFIG_GLOBAL=/dev/null or a fixture file, GIT_CONFIG_NOSYSTEM=1, fixed fake identities, and only explicitly required extra variables. Never copy/filter the inherited environment, mutate process-wide variables, or use the production Git constructor as fixture isolation. Preserve existing behavioral assertions; no production or routing changes and no repository-wide rewrite of independent helpers.

Verification: first add a hostile-parent regression that runs representative common-helper initialization, staging, commit and worktree operations in a child whose inherited Git selectors point only at owned temporary foreign state. Before RED, the child environment is explicit and fake HOME/global config are temporary. Prove the old helper changes or misroutes owned foreign state, then prove the isolated helper succeeds while foreign HEAD/index/fake global config remain byte-identical and the intended fixture changes. Assert the fixture top level and Git administrative-directory ownership before the first write; if a caller writes through a linked worktree, validate its top level and that its main Git directory lies inside the owned fixture rather than assuming the Git directory is below the worktree itself. Never demonstrate the defect against real host repositories/configuration. Run the focused regression through just test-one, then just test-fast and just check after the normal full baseline in the implementation worktree.

Done: common::git cannot inherit repository selectors or host environment/config, all affected callers retain their assertions, and the hostile-parent regression plus the normal checks pass.

## References

- prior-review (input, required, kind=review): tasks/files/tasks-171b8b/fixture-isolation-review-report.md — earlier hardening deliberately excluded other integration Git helpers

## Notes

- 2026-10-25T12:31:46Z (main): concerns: tasks-171b8b extension — Extend the completed linked/record-commit fixture hardening to the shared CLI/refs Git helper that its review deliberately excluded.
- 2026-10-25T12:31:50Z (main): Duplicate check: open fixture/environment tasks in this project and all-project searches for the shared helper found no matching task. tasks-79a907 covers the isolated record-commit unit runner; ops-5dc3c0 covers separate Python fixture helpers, so neither duplicates this Rust helper fix. No hostile-environment reproduction has been run yet; only code inspection was needed to file the bounded follow-up.
- 2026-10-26T15:01:29Z (main): tasks-2518ad (2b6625e) moved tests/move.rs off its private leaky git helper and made git_fixture::hostile_parent's fake global config carry a failing config-based pre-commit hook, so a hostile-parent regression for common::git also catches inherited host hooks.
- 2026-10-26T15:12:35Z (main): started
  provenance: {"harness_session":"codex:01a178c4-420d-75e3-a07d-83976bde0d40","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T15:12:54Z (fix/tasks-047ad0): resumed
  provenance: {"harness_session":"codex:01a178c4-420d-75e3-a07d-83976bde0d40","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T15:14:00Z (fix/tasks-047ad0): Direct scope confirmed: common::git plus regression in tests/cli.rs and linked-worktree ownership assertion in the one common-helper caller that commits through side. Keep its existing signature, reuse git_fixture::command with per-invocation temporary HOME, and validate init before later fixture writes. No production, CLI, routing, dependency, or independent-helper changes. Worktree hydration completed with README cargo install --locked --path . confined to worktree target and install root; installed output now at target/hydrated-install.
- 2026-10-26T15:15:47Z (fix/tasks-047ad0): Baseline: just test in .worktrees/tasks-047ad0 passed all 1744 tests, 0 failed, 0 ignored. Log retained under target/tasks-047ad0-baseline.log until attached at closeout. Regression break: inherited Git selectors redirect common::git initialization away from the intended fixture; inherited global hooks can also prevent commits.
- 2026-10-26T15:16:45Z (fix/tasks-047ad0): RED: just test-one --test cli common_git_fixtures_ignore_hostile_environment exited 101 with the old common::git: hostile selectors redirected init to the owned foreign .git, leaving the intended fixture uninitialized; the isolated boundary probe failed with not a git repository before staging. Parent checks confirmed foreign HEAD/index/fake global config unchanged. Only explicit fake child environment and owned temporary repositories were used; log target/tasks-047ad0-red.log.
- 2026-10-26T15:18:00Z (fix/tasks-047ad0): attached: fixture-isolation-verification.txt (5253 bytes): Verification excerpts: 1744-test baseline, expected RED misrouting, GREEN regression, 1741-test fast suite (4 ignored), and successful just check; build chatter omitted.
- 2026-10-26T15:20:54Z (fix/tasks-047ad0): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-26T15:20:57Z (fix/tasks-047ad0): Ruling: exclude production Git isolation and CLI behavior from this review at d082ffee729a780303f3a22bcc6c12128dc9a6e1. Why: unchanged and explicitly outside tasks-047ad0. Cost: production isolation receives no assurance from this fixture-only fix. Ruling: exclude independent Git helpers elsewhere at the same revision. Why: this task and the prior review bound the work to common::git and connected callers. Cost: those independent helpers may still inherit environment and need separate work.
- 2026-10-26T15:21:00Z (fix/tasks-047ad0): attached: fixture-isolation-review-report.md (2462 bytes): Full read-only whole-branch review by codex/gpt-6.1-sol, frozen d082ffee729a780303f3a22bcc6c12128dc9a6e1: Ship, no findings; 6 independent tests passed.
- 2026-10-26T15:21:14Z (fix/tasks-047ad0): retained: {"host":"titan","kind":"worktree","path":".worktrees/tasks-047ad0","state":"kept","verification":"verified","purpose":"Reviewed implementation awaiting immediate local integration and checked removal; target contains only regenerable build/install outputs and redundant logs, with verification excerpts and full review preserved as verified task attachments","next_owner":"session:codex:01a178c4-420d-75e3-a07d-83976bde0d40","cleanup_trigger":"After local merge into main in this turn, recheck pointers, Git state and ignored files, run tt-report, and remove the task worktree"}
- 2026-10-26T15:21:17Z (fix/tasks-047ad0): done
  provenance: {"harness_session":"codex:01a178c4-420d-75e3-a07d-83976bde0d40","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T15:21:17Z (fix/tasks-047ad0): Isolated common::git with temporary HOME and key-built environment; checked init/linked ownership; hostile-parent RED to GREEN verified; baseline 1744 passed, fast 1741 passed (4 ignored), just check passed, review Ship with 6 independent tests and no findings.
  provenance: {"harness_session":"codex:01a178c4-420d-75e3-a07d-83976bde0d40","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T15:22:29Z (main): retained: {"host":"titan","kind":"worktree","path":".worktrees/tasks-047ad0","state":"released","verification":"verified","purpose":"Merged locally into main at 0f312a9; preserved verified task attachments; discarded regenerable target build/install outputs and redundant logs; after clean Git/submodule/ignored-file checks, tt-report and no-host-pointer verification, removed worktree and merged branch","next_owner":"none","cleanup_trigger":"none"}
