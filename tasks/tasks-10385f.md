---
id: tasks-10385f
title: "Keep inherited GIT_DIR, GIT_INDEX_FILE and GIT_WORK_TREE out of the git the binary runs for a detached project's records repository"
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: fix/tasks-10385f-git-selectors
created: 2026-10-23T01:04:39Z
updated: 2026-10-23T11:33:04Z
started: 2026-10-23T11:17:47Z
depends: []
tags: []
source: tasks-211e28
agent: claude-code/claude-fable-5-1
---

Raised by the final review of docs/plans/2026-10-22-records-detached.md (declined to judge, pre-existing for linked projects, more likely now). Project::git runs in records with current_dir, GitTree::of and record_commit::discover use -C; a pre-commit hook exports GIT_DIR and GIT_INDEX_FILE for the code repository, and those win over the directory, so under the code repository's hook the binary's git queries about the records repository (uncommitted_task_files, worktree_records, sibling copies, the record commit) read the wrong repository. Spec docs/specs/2026-10-22-records-home-design.md §8 step 4 relies on the code repository's pre-commit tasks check during the removal commit, so this gates the ops pilot migration (ops-423f23). Decide which calls must honour the hook's variables (link::config_in_index reads the code tree's index on purpose) and clear the inherited selectors for every call aimed at the records repository; prove it with a test that exports GIT_DIR and GIT_INDEX_FILE pointing at a foreign repository while a detached project's commands run from its code checkout. Related: tasks-79a907 covers the same selectors in the record-commit fixture tests.

## Notes

- 2026-10-23T11:17:47Z (main): started
  provenance: {"harness_session":"codex:01a16873-670b-76a1-af8d-052e18860638","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-23T11:17:58Z (fix/tasks-10385f-git-selectors): resumed
  provenance: {"harness_session":"codex:01a16873-670b-76a1-af8d-052e18860638","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-23T11:20:48Z (fix/tasks-10385f-git-selectors): Root cause confirmed in the path-targeted Git constructors: inherited repository/index selectors override current_dir and -C. Use one shared path-targeted constructor for discovery, record status/worktree checks and commits; preserve the invoking index only in link::config_in_index. Update the two existing GIT_DIR-override expectations to the path-targeted contract and cover detached operations plus the alternate-index migration guard.
- 2026-10-23T11:20:59Z (fix/tasks-10385f-git-selectors): Workspace hydrated with cargo install --locked --path . --root .local --target-dir target (confined installation). Baseline: just test exited 0; 2469 tests passed, 0 failed, 0 ignored.
- 2026-10-23T11:21:07Z (fix/tasks-10385f-git-selectors): Correction to the preceding baseline note: the log totals 1658 tests across 14 test binaries, all passed (not 2469); just test exited 0.
- 2026-10-23T11:26:50Z (fix/tasks-10385f-git-selectors): Verification: regression RED reproduced wrong-repository panics, missed dirty records, incorrect path discovery and bypassed stale-copy rejection; focused detached suite now 45 passed. just test-fast: 1658 passed, 4 existing ignored, exit 0. just check and pre-commit passed. Implementation frozen for independent review at 92b1c96.
- 2026-10-23T11:29:43Z (fix/tasks-10385f-git-selectors): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-23T11:30:53Z (fix/tasks-10385f-git-selectors): Post-review boundary probe found that config_in_index retains unrelated repository selectors when GIT_INDEX_FILE is absent: a detached check incorrectly returns records_moved for a foreign indexed config. New regression migration_guard_uses_the_code_repository_without_an_explicit_hook_index fails with that error. Narrow the exception to the explicit hook index only; the task remains direct and within its selector-isolation scope.
- 2026-10-23T11:31:25Z (fix/tasks-10385f-git-selectors): attached: git-selectors-round-1-report.md (2088 bytes): Independent implementation review of 92b1c96 by codex/gpt-6.1-sol; first revision accepted, followed by the controller’s narrower migration-index fix.
- 2026-10-23T11:31:44Z (fix/tasks-10385f-git-selectors): Ruling on review round 1, revision 92b1c96: broader Git object-store/configuration environment hardening is deferred. Why: outside this task’s four repository/index selectors. Cost: those other overrides can still affect Git behavior. Older linked and record-commit fixture environment inheritance remains separate. Why: unchanged here, with record-commit fixture hardening tracked by tasks-79a907. Cost: those existing helpers retain their inherited-environment exposure.
- 2026-10-23T11:33:04Z (fix/tasks-10385f-git-selectors): Correction verified at 6e1492a: config_in_index now selects the code repository by path and restores only an explicit GIT_INDEX_FILE. Guard regression proved RED then GREEN; detached suite 46 passed. just test-fast exited 0: 1659 passed, 4 existing ignored. Pre-commit passed formatting, clippy, hygiene and tasks check. Fresh scoped re-review requested.
