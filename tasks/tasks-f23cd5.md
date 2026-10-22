---
id: tasks-f23cd5
title: "record_commit unit tests commit through the production git runner under the host's global git config, so config-based hooks (hook.<name>.command) still run in test repos"
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: main
created: 2026-10-18T14:59:28Z
updated: 2026-10-22T23:37:44Z
started: 2026-10-22T23:37:44Z
depends: []
tags: [testing]
source: tasks-eeeeda
agent: claude-code/claude-opus-5-5
---

tasks-eeeeda stopped the HOME leak and made the test-only git helper in src/record_commit.rs ignore the global config, but the commits under test go through the production git() runner (commit_paths_in), which reads the developer's global config. Config-based hooks are not disabled by core.hooksPath, so a host hook (e.g. an identity check) runs on every fixture commit: today it passes, but the suite's outcome still depends on the host. Fix: let the tests pass git environment (GIT_CONFIG_GLOBAL=/dev/null, GIT_CONFIG_NOSYSTEM=1) into the runner, by injection, not a process-wide set_var, and assert a hostile global hook cannot fail a fixture commit (reproduce with a GIT_CONFIG_GLOBAL fixture whose pre-commit hook exits 1). Related: ops-646c0e (same gap in ops), lore-e1c2db (a global hermetic-git rule).

## Notes

- 2026-10-18T14:59:30Z (main): concerns: tasks-eeeeda extension — the HOME-leak fix left production record commits in unit tests reading the host's global git config
- 2026-10-22T23:37:44Z (main): started
  provenance: {"harness_session":"codex:01a165e8-fe11-7e63-b157-1faff4921001","harness_session_source":"CODEX_THREAD_ID"}
