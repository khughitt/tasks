---
id: tasks-047ad0
title: Isolate the shared Git helper used by CLI integration fixtures
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-25T12:30:00Z
updated: 2026-10-25T12:31:46Z
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
