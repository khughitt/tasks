---
id: tasks-bacba3
title: Adopt the ops act design's front door
status: done
priority: 2
size: s
complexity: low
process: direct
owner: chore/act-front-door
created: 2026-09-28T10:28:33Z
updated: 2026-09-29T19:05:07Z
started: 2026-09-29T18:58:18Z
completed: 2026-09-29T19:05:07Z
depends: []
tags: [testing]
source: ops-a5a7ef
agent: claude-code/claude-opus-5-5
---

ops docs/specs/2026-09-28-test-ci-act-design.md §7.3. Copy templates/justfile's test-one, the docs-only pre-commit path and the CI-aware pre-push gate (ci_suite_refs, ci_remote, push_fast_cmd, hook-pre-push-fast) with both templates/githooks; set ci_suite_refs from what CI runs the full suite for and note it; AGENTS.md Gates line per templates/AGENTS.md. Move `test-fast [<name>]`'s filter to `test-one`; `test-fast` takes no arguments afterwards. No alias.

## Notes

- 2026-09-29T18:58:18Z (chore/act-front-door): started
- 2026-09-29T18:58:18Z (chore/act-front-door): claimed by Codex adopt_tasks worker; direct process in .worktrees/act-front-door; no setup recipe is defined
- 2026-09-29T18:58:41Z (chore/act-front-door): resumed
- 2026-09-29T18:58:41Z (chore/act-front-door): took over session sid:1714679 (owner chore/act-front-door, host titan, pid 1714679, worktree /mnt/ssd3/work/tasks/.worktrees/act-front-door, since 2026-09-29T18:58:18Z, age 23s, stale: pid 1714679 is gone)
- 2026-09-29T18:58:41Z (chore/act-front-door): claimed by Codex adopt_tasks worker, pid 3949212; CI has no workflow, so ci_suite_refs is empty and every push retains the full local gate
- 2026-09-29T19:02:42Z (chore/act-front-door): Focused hook tests passed after red-green verification; adopted pre-push Git-local environment cleanup so suite sandboxes use their own repositories (shared template feedback ops-9ef532)
- 2026-09-29T19:05:07Z (chore/act-front-door): Coordinator reviewed final diff and approved; just test-fast passed 743 tests with 1 intentional ignored test; just check passed; tasks check has 0 errors and the unchanged closed-task retired_prefix warning on tasks-dc599b
- 2026-09-29T19:05:07Z (chore/act-front-door): done
- 2026-09-29T19:05:07Z (chore/act-front-door): Adopted focused test-one, zero-argument test-fast, conservative docs-only commit checks and CI-aware push hooks; no CI means full local push gate
