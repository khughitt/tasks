---
id: tasks-f23cd5
title: "record_commit unit tests commit through the production git runner under the host's global git config, so config-based hooks (hook.<name>.command) still run in test repos"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-18T14:59:28Z
updated: 2026-10-18T14:59:28Z
depends: []
tags: [testing]
source: tasks-eeeeda
agent: claude-code/claude-opus-5-5
---

tasks-eeeeda stopped the HOME leak and made the test-only git helper in src/record_commit.rs ignore the global config, but the commits under test go through the production git() runner (commit_paths_in), which reads the developer's global config. Config-based hooks are not disabled by core.hooksPath, so a host hook (e.g. an identity check) runs on every fixture commit: today it passes, but the suite's outcome still depends on the host. Fix: let the tests pass git environment (GIT_CONFIG_GLOBAL=/dev/null, GIT_CONFIG_NOSYSTEM=1) into the runner, by injection, not a process-wide set_var, and assert a hostile global hook cannot fail a fixture commit (reproduce with a GIT_CONFIG_GLOBAL fixture whose pre-commit hook exits 1). Related: ops-646c0e (same gap in ops), lore-e1c2db (a global hermetic-git rule).
