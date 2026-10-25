---
id: tasks-3b3f0b
title: Route filtered reads through the shared ID resolver
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: design/tasks-284d9b
created: 2026-10-25T11:14:07Z
updated: 2026-10-25T11:56:03Z
started: 2026-10-25T11:38:48Z
completed: 2026-10-25T11:56:03Z
closed: 2026-10-25T11:56:03Z
depends: []
parent: tasks-284d9b
tags: []
agent: codex
spec: docs/specs/2026-10-25-filtered-read-routing-design.md
plan: docs/plans/2026-10-25-filtered-read-routing-plan.md
step: "Task 1: Route filtered reads through the shared ID resolver"
---

Implement the approved filtered-read routing contract with canonical prefix agreement, explicit-scope precedence, matching-worktree preservation and typed no_project recovery guidance. Follow plan Task 1's registry/discovery ordering, routing/source/error matrices, safe linked/worktree fixtures, documentation, focused RED/GREEN and checks. Execute only after the parent plan is approved; use the parent's existing worktree. Parent owns retained resources.

## Notes

- 2026-10-25T11:38:48Z (design/tasks-284d9b): started
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T11:41:21Z (design/tasks-284d9b): Baseline: just test passed, 1685 tests across 14 binaries, 0 failed, 0 ignored (full suite, exhaustive enumeration included). Existing worktree hydrated and hooks installed. tasks-e147b9 remains open with no finding. Accepted plan review adjustments preserve local-before-registry error order and add outside installed-binary smoke plus hostile-parent fixture coverage.
- 2026-10-25T11:48:43Z (design/tasks-284d9b): Routing verification: RED 6 CLI + 2 linked cases on missing outside/foreign routing; GREEN 8/8. Broken-marker and hostile-parent controls 1 each passed; existing bare suffix 2, project scope 4, positional outside route 1 passed. just test-fast passed 1689 tests, 0 failed, 4 ignored across 14 binaries. Positional resolver unchanged; only combined filters prevalidate against original context. Parent owns worktree/evidence.
- 2026-10-25T11:55:58Z (design/tasks-284d9b): Final Task 1 checks passed: fast suite 1689 passed, 0 failed, 4 ignored; configured commit hook hygiene/fmt/Clippy/tasks checks passed after replacing test clones with borrowed slices. Confined installed binary smoke: outside list --under tasks-284d9b equals registered main (0 descendants); source-only returned no_project with guidance; live --all-projects recovery returned 0 matches, 0 warnings; local list/search succeeded. Parent tasks-284d9b owns .worktrees/tasks-284d9b and both .superpowers/sdd evidence directories; this child owns no separate retained resources.
- 2026-10-25T11:56:03Z (design/tasks-284d9b): done
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T11:56:03Z (design/tasks-284d9b): Implemented shared ID-filter routing, typed no_project guidance, docs/help and regression controls in 634cf8d; focused checks and 1689-test fast suite passed. Parent retains worktree and evidence for Task 2 and final review.
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
