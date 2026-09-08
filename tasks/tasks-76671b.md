---
id: tasks-76671b
title: "note/start write the record in whichever checkout they run from, so a worktree and its main checkout diverge silently"
status: done
priority: 2
size: s
owner: feat/worktree-divergence-warning
created: 2026-09-07T12:51:11Z
updated: 2026-09-08T02:02:11Z
depends: []
tags: [feedback, gap, "from:forge", cli, correctness]
---

Warn when another checkout of this project holds a newer copy of the record being written.

Mechanism: in save() (src/commands/mod.rs), before the updated bump on line 402, the task still carries the stamp it was loaded with. Compare that against every sibling worktree's copy. New Project::sibling_task_copies() in src/repo.rs, modelled on uncommitted_task_files: git worktree list --porcelain -z, LC_ALL=C, Ok(None) only for the two documented skips (no git binary, not a repository), every other git failure a typed error. Map paths through rev-parse --show-toplevel so a project nested below the repo root resolves. Parse each sibling's bytes with parse_task so the stamp is frontmatter-only and validated; a missing sibling file is skipped, an unreadable or malformed one becomes a warning.

Warn only when a sibling's stamp is newer than the one we loaded. A worktree merely behind is the normal state and stays silent. Never blocks a write; a git failure is a warning, as in warn_if_uncommitted_with_worktrees.

edit: restore original.updated onto the parsed record before save (src/commands/edit.rs:169), so a hand-edited updated: line cannot suppress the warning. No observable change, since save overwrites it anyway.

Out of scope, deliberately: feedback's guarded_update (src/commands/feedback.rs:216) bumps updated and calls write_task directly, bypassing save. Its read-verify-retry loop guards within a checkout, and it writes to the upstream feedback repo, which is not a worktree-branching workflow. Covering it would mean hoisting the check into write_task and dragging in create_task.

Known limits, to be documented in the code, not fixed: (1) updated is second-precision, so two writes to one record in the same second in two checkouts compare equal and slip through; (2) after the first warned write our stamp is now(), which beats the sibling's, so the warning fires once per divergence and goes quiet while the divergence stands, until the sibling writes again. Fixing (2) means content comparison, which is the noisy option rejected in design.

Leaves the existing start-time warn_if_uncommitted_with_worktrees alone: it is shape-based and would double up.

Catches the reported sequence at done in the worktree: it loaded a copy stamped T1, the main checkout's copy is T2, theirs is newer, warn. start and note in main run before the worktree exists and are correctly silent.

## Notes

- 2026-09-08T02:02:11Z (feat/worktree-divergence-warning): Every write of an existing record now warns when another worktree holds a copy with a newer updated stamp: Project::sibling_task_copies (git worktree list --porcelain -z, read at the project's offset below the repo top level, parsed with parse_task) plus a check in save() reading the pre-bump stamp as its baseline. edit restores original.updated so a hand-edited stamp cannot suppress it. A checkout merely behind stays silent; unreadable copies warn; git failure warns without refusing. feedback's guarded_update stays out of scope. Documented in the work-claims design doc with both accepted limits.
