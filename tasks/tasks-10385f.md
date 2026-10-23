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
updated: 2026-10-23T11:17:58Z
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
