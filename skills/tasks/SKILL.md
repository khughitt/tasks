---
name: tasks
description: Use when working in a repository that contains tasks/.config.toml or tasks/*.md task records.
---

# tasks

`tasks` is the repository's task tracker: one markdown file per task under `tasks/`,
managed only through the CLI. Output is JSON unless `--pretty` is given.

## Session protocol

1. `tasks prime` — counts, the ready list, and who is working on what.
2. Pick from `tasks ready` (sorted by priority, then size). Never pick an `idea`; scope it first.
3. `tasks start <id>` before changing code. It records you as owner.
4. `tasks note <id> "<one line>"` whenever scope or understanding changes.
5. `tasks done <id> "<what landed>"` in the same commit as the code. If dependencies are
   still open, do not `--force` unless the dependency is genuinely irrelevant; say why in the message.
6. `tasks check` before committing. A failing check means a task and its plan/spec drifted apart; fix both.

Never edit `tasks/*.md` directly. `tasks edit <id> --title/--body/-p/--size/--tag/--depends/--spec/--plan/--step`
updates fields; `tasks edit <id>` with no flags opens `$EDITOR` and validates the result.

## Recording work

- An unscoped thought: `tasks add "<title>" --status idea -b "<why>"`. Ideas never appear in `ready`.
- A scoped task: `tasks add "<title>" -p <0-4> --size <xs|s|m|l|xl> --tag <group> [--spec <name>] [--plan <name> --step "<heading>"]`.
- Splitting: `tasks add` each piece, then `tasks dep <original> --on <piece>...` (or `tasks drop <original> "split into …"`), and `tasks note <original> "split into <ids>"`.
- Blocking on another project: `tasks dep <id> --on <prefix>-<hex>`; the other project must be registered (`tasks init` there).
- Id collision after a merge (git add/add conflict on the same `tasks/<id>.md`): keep one file, rename the other to a fresh id, fix its `id` field, then run `tasks check` and repair any `depends` it reports.

## With superpowers

- **brainstorming** writes specs under `docs/{specs,designs}/` or
  `docs/superpowers/{specs,designs}/`. After approval: one task per deliverable,
  `--spec <topic>`.
- **writing-plans** writes the plan to `docs/plans/YYYY-MM-DD-<topic>.md`. One task per `### Task N:` heading:
  `tasks add "<heading>" --plan <topic> --step "Task N: <title>" --depends <previous-task-id>`.
- **executing-plans / subagent-driven-development**: `tasks start` a step before implementing, `tasks done` when its commit lands.
- Plan headings are the drift contract: renaming or removing a heading under an open task fails `tasks check`. Update the task in the same change.
