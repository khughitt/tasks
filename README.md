# tasks

A fast, file-based task tracker for software projects, built for coding agents and the
humans supervising them. One markdown file per task, checked into the project; a single
Rust binary as the only writer. JSON output by default.

Design: [`docs/specs/2026-08-29-tasks-design.md`](docs/specs/2026-08-29-tasks-design.md).
Implementation plan: [`docs/plans/2026-08-29-tasks.md`](docs/plans/2026-08-29-tasks.md).

## Install

    cargo install --path .

## Use

    cd <repo>
    tasks init --prefix sci          # creates tasks/, docs/specs/, docs/plans/; registers the project
    tasks add "Bank the ledger" -p 1 --size m --tag ledger
    tasks ready                      # what can be worked on now (JSON)
    tasks --pretty ready             # same, as a table (or export TASKS_FORMAT=pretty)
    tasks start sci-4f2a9c
    tasks note sci-4f2a9c "spec §4 no longer holds"
    tasks done sci-4f2a9c "landed in 1a2b3c"
    tasks check                      # validate files, links, plan steps, dependencies

Run `tasks --help` for the full command list.

## Agent skill

`skills/tasks/SKILL.md` teaches agents the session protocol. Install it once at user level
so it applies to every project:

    mkdir -p ~/.claude/skills && ln -s "$PWD/skills/tasks" ~/.claude/skills/tasks
    mkdir -p ~/.agents/skills && ln -s "$PWD/skills/tasks" ~/.agents/skills/tasks   # other harnesses

or per project, when a project needs to pin its own copy:

    mkdir -p <repo>/.claude/skills && cp -r skills/tasks <repo>/.claude/skills/tasks

`tasks init` warns when neither location has the skill.

## Adopting in an existing project

1. Keep historical docs in place. Move only active specs/plans that need structured task
   links into `docs/specs/` / `docs/plans/`, fixing links; new docs use those directories.
2. `tasks init --prefix <p>`.
3. Install the skill and mention it in the project's CLAUDE.md / AGENTS.md.
4. Require `tasks prime` at session start and `tasks check` before completion. Add
   `tasks check` to automation once the binary has a pinned install source; never skip it
   conditionally when unavailable.

## Layout

    tasks/.config.toml               prefix = "sci"
    tasks/sci-4f2a9c.md              one task
    ~/.config/tasks/projects.toml    per-machine registry: prefix -> repo path
