# Fresh worktree brief

## Problem

A task created in the registered checkout can be absent from a newly created worktree, and that worktree can lack dependencies required by its commit hooks. Work should be ready to start and commit without manually moving task files or discovering a package install interactively.

## Current behaviour and evidence

`tasks add` and `tasks feedback` create an untracked record in the current checkout. Git worktrees begin from committed content, so the record is absent in a worktree created afterwards. `start` warns only after it writes an uncommitted record while multiple worktrees exist (`src/commands/status.rs`).

The task skill prescribes `just setup` when that recipe exists. It provides no declared fallback for projects without a justfile. The report in `tasks-5e6971` observed a commit hook waiting for its package runner to install dependencies in such a worktree. The feedback design already documents the visibility constraint in `docs/specs/2026-09-03-feedback-design.md`.

## Constraints

Task files are committed with the code change and the CLI must not silently copy or execute arbitrary project commands. Existing projects use different build tools, hooks, and agent instructions. The current `tasks/.config.toml` is CLI-owned configuration for task metadata and document roots.

## Alternatives

1. Require the task record to be committed before `git worktree add`, then run a project-declared bootstrap command. This is explicit and aligns both worktrees; it needs a portable declaration and a non-interactive failure contract.
2. Teach `tasks start` or `add` to copy task records into a worktree. Rejected for now: it creates divergent uncommitted records and cannot safely decide which checkout owns the task.
3. Keep only the `just setup` convention. Rejected: projects without Just have no supported bootstrap path.

The lean is option 1, with the declaration located in the project instructions unless evidence shows `tasks/.config.toml` is the established cross-project contract.

## Unanswered questions

- Which project-owned file can declare a bootstrap command without making the task CLI execute arbitrary commands?
- How should the workflow fail when a bootstrap command would prompt or is absent?
- Can the task creation flow commit the record before worktree creation without violating the caller's intended commit boundary?

## Proposed decomposition

- `tasks-5a46b9` and `tasks-5e6971` remain ideas pending `tasks-63742b`'s recommendation.
- `tasks-63742b` will survey current project setup conventions and recommend a portable, non-interactive workflow. If it requires a new task configuration contract, follow with a reviewed design task.
