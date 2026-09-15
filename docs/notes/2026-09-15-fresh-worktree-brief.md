# Fresh worktree brief

## Problem

A task created in the registered checkout can be absent from a newly created worktree, and that worktree can lack dependencies required by its commit hooks. Work should be ready to start and commit without manually moving task files or discovering a package install interactively.

## Current behaviour and evidence

`tasks add` and `tasks feedback` create an untracked record in the current checkout. Git worktrees begin from committed content, so the record is absent in a worktree created afterwards. `start` warns only after it writes an uncommitted record while multiple worktrees exist (`src/commands/status.rs`).

The task skill prescribes `just setup` when that recipe exists. It provides no declared fallback for projects without a justfile. The report in `tasks-5e6971` observed a commit hook waiting for its package runner to install dependencies in such a worktree. Nexcode is a concrete example: its root guide says `npm install` after pull, while its `commit-msg` hook runs `npx commitlint`, which offers an interactive install when `node_modules` is absent. The feedback design already documents the visibility constraint in `docs/specs/2026-09-03-feedback-design.md`.

## Constraints

Task files are committed with the code change and the CLI must not silently copy or execute arbitrary project commands. Existing projects use different build tools, hooks, and agent instructions. The current `tasks/.config.toml` is CLI-owned configuration for task metadata and document roots.

## Alternatives

1. Require the task record to be committed before `git worktree add`, then run the root guide's explicit setup command after the existing `just setup` convention. This is explicit, aligns both worktrees, and uses the project's already-reviewed ownership of its setup command.
2. Teach `tasks start` or `add` to copy task records into a worktree. Rejected for now: it creates divergent uncommitted records and cannot safely decide which checkout owns the task.
3. Keep only the `just setup` convention. Rejected: projects without Just have no supported bootstrap path.

The recommendation is option 1. The task CLI remains a record manager: it neither copies an uncommitted record nor gains an arbitrary-command field in `tasks/.config.toml`. The workflow fails early when neither `just setup` nor a documented root setup command exists; it never guesses an installer.

## Unanswered questions

- How should a root guide express that a setup command must be non-interactive?
- Which global and shipped task-workflow instructions must state the ordering consistently?

## Proposed decomposition

- `tasks-5a46b9` and `tasks-5e6971` can become direct documentation work: commit the task record before branching, then use `just setup` or the root guide's explicit setup command.
- `tasks-63742b` established that recommendation from the task, global, and Nexcode instructions. No new task configuration contract is needed.
