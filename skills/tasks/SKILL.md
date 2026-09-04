---
name: tasks
description: Use when working in a repository that contains tasks/.config.toml or tasks/*.md task records.
---

# tasks

`tasks` is the repository's task tracker: one markdown file per task under `tasks/`,
managed only through the CLI. Output is JSON unless `--pretty` is given.

## Session protocol

1. `tasks prime` — roadmap (the open goal tree), closeout (goals whose work is all
   done), the ready list, and who is working on what.
2. Pick from `tasks ready` (sorted by priority, then size). Never pick an `idea`; scope it first.
   Never pick a task with children; those are goals. `ready` already omits them.
3. `tasks start <id>` before changing code. It records you as owner.
4. `tasks note <id> "<one line>"` whenever scope or understanding changes.
5. `tasks done <id> "<what landed>"` in the same commit as the code. If dependencies are
   still open, do not `--force` unless the dependency is genuinely irrelevant; say why in the message.
   `done` refuses while any descendant is open (`--force` overrides); `drop` refuses while any
   descendant is open and has no override — drop or reparent the subtree first
   (`tasks drop <child> "<why>"` / `tasks edit <child> --no-parent`).
6. `tasks check` before committing. A failing check means a task and its plan/spec drifted apart; fix both.
7. When a goal appears under `closeout`, confirm it is met and `tasks done <id> "<verdict>"`,
   or add the children still missing.

Never edit `tasks/*.md` directly. `tasks edit <id> --title/--body/-p/--size/--tag/--depends/--spec/--plan/--step/--parent/--no-parent`
updates fields; `tasks edit <id>` with no flags opens `$EDITOR` and validates the result.

## Recording work

- An unscoped thought: `tasks add "<title>" --status idea -b "<why>"`. Ideas never appear in `ready`.
- A scoped task: `tasks add "<title>" -p <0-4> --size <xs|s|m|l|xl> --tag <group> [--spec <name>] [--plan <name> --step "<heading>"]`.
- Decomposing: `tasks add "<piece>" --parent <goal>` for each part; `tasks dep` only
  for ordering between the pieces. A goal that is committed work is a `todo` with a
  body, however large; `idea` is for uncommitted thoughts. `done` refuses while any
  descendant is open (`--force` overrides); `drop` refuses while any descendant is
  open and has no override — drop or reparent the subtree first
  (`tasks drop <child> "<why>"` / `tasks edit <child> --no-parent`).
- Blocking on another project: `tasks dep <id> --on <prefix>-<hex>`; the other project must be registered (`tasks init` there).
- Id collision after a merge (git add/add conflict on the same `tasks/<id>.md`): keep one file, rename the other to a fresh id, fix its `id` field, then run `tasks check` and repair any `depends` it reports.
- `tasks tree [<id>]` shows the hierarchy; `tasks edit <id> --parent <goal>` / `--no-parent` moves a task.
- Throwaway projects get a throwaway registry: `tasks init` registers globally in
  `~/.config/tasks/projects.toml`, and that entry outlives the scratch directory it
  names. For a demo, a smoke test, or anything under a temp dir, run
  `XDG_CONFIG_HOME=$(mktemp -d) tasks init --prefix <p>` so the registration dies with
  it. If you forget, `tasks unregister <prefix>` removes the entry; project files are
  untouched.

## With superpowers

- **brainstorming** runs against an existing task and attaches with
  `tasks edit <id> --spec <topic>`; deliverables become children with
  `--parent <id> --spec <topic>`.
- **writing-plans** attaches with `tasks edit <id> --plan <topic>` and adds one child
  per `### Task N:` heading with `--parent <id> --plan <topic> --step "Task N: <title>"`;
  `tasks check` warns on any heading left without a task.
- **executing-plans / subagent-driven-development**: `tasks start` a step before implementing, `tasks done` when its commit lands.
- Plan headings are the drift contract: renaming or removing a heading under an open task fails `tasks check`. Update the task in the same change.

## Feedback about the tool

When `tasks` itself gets in the way (`friction`), cannot do something you needed (`gap`),
gives you an idea (`idea`), or works notably well (`positive`), file it at that moment and
carry on:

    tasks feedback "<one line about the tool>" --category <friction|gap|idea|positive> [-b "<command, error kind, what you expected>"]

Describe the tool, not the project: no repository names, file paths, people, or project
content. The upstream repository is public. Do not commit there and do not triage your own
report. If `ambiguous` comes back, rerun with `--recur <id>` to join the listed entry or
`--new` to insist. Keep the returned id in a note if the outcome matters to your task;
`tasks show <id>` works from any registered project.

### In the tasks repository

Uncommitted files under `tasks/` tagged `feedback` are unreviewed reports: read each,
redact anything that describes a project rather than the tool, then commit. Ideas tagged
`feedback` are the triage queue; scope, drop, or promote them like any other idea and record
the outcome in a note.
