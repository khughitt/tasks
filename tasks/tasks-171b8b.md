---
id: tasks-171b8b
title: Isolate Git subprocesses in linked and record-commit integration fixtures
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-23T11:41:03Z
updated: 2026-10-23T11:41:03Z
depends: []
tags: [testing]
source: "tasks-10385f:integration-fixture-isolation"
agent: codex
---

Why: the implementation review of tasks-10385f left older fixture runners outside its production fix; [ref:review] records that boundary. tests/linked.rs::git and tests/record_commit.rs::git still inherit the parent environment, so a hook's Git selectors can redirect fixture operations and inherited credentials/config can reach failing subprocess output. This is distinct from tasks-79a907, which extends the already-isolated src/record_commit.rs unit-test runner's hostile-environment and refused-cleanup regression.

Scope: isolate only the Git subprocess helpers and direct Git subprocesses in these two integration-test modules, using the explicit environment style already in tests/common/mod.rs and tests/detached.rs. Build environments key by key with PATH, a fixture-owned temporary HOME, fixed identity where needed, disabled global/system Git config and only required extra variables; do not copy/filter the parent environment or use the production Git constructor as fixture isolation. Before mutation beyond initialization, assert both the fixture top level and a Git directory inside that fixture. Preserve all existing assertions and fixture behavior; no production changes or repository-wide fixture rewrite.

Verification: first add a regression that runs representative linked-project and record-commit fixture operations in a separately spawned child with GIT_DIR, GIT_COMMON_DIR, GIT_WORK_TREE and GIT_INDEX_FILE pointing exclusively at owned temporary foreign state. Prove the old helper fails, then prove the isolated helper succeeds while the foreign HEAD/index and fake global Git config remain unchanged. The child environment is explicit from its first RED run, and no process-wide environment mutation is allowed. Run focused tests through just test-one, then just test-fast and just check.

Done: the two modules' Git fixtures cannot inherit repository selectors or host credentials/config, the fixture-boundary assertions hold, and the hostile-selector regression passes. Related tasks-79a907 remains a separate unit-test coverage follow-up.

## References

- review (input, required, kind=review): tasks/files/tasks-10385f/git-selectors-round-1-report.md — review exclusion identifying older fixture environment inheritance
