# tasks

A fast, file-based task tracker for software projects, built for coding agents and the
humans supervising them. One markdown file per task, checked into the project; a single
Rust binary as the only writer. JSON output by default.

Design: [`docs/specs/2026-08-29-tasks-design.md`](docs/specs/2026-08-29-tasks-design.md).
Implementation plan: [`docs/plans/2026-08-29-tasks.md`](docs/plans/2026-08-29-tasks.md).

## For agents

If you have been asked to install `tasks` or adopt it in a project, do these in order.
Each step is idempotent.

1. Install the binary and the skill (once per machine; needs a Rust toolchain):

       git clone https://github.com/khughitt/tasks ~/.local/src/tasks
       cargo install --path ~/.local/src/tasks
       mkdir -p ~/.claude/skills && ln -sfn ~/.local/src/tasks/skills/tasks ~/.claude/skills/tasks

   Other harnesses look in `~/.agents/skills`; symlink there as well if relevant.

2. Initialise the project (once per repository):

       cd <repo>
       tasks init --prefix <p>          # p: a letter then 1-7 letters/digits, e.g. sci

   If the project keeps specs or plans outside `docs/specs/` and `docs/plans/`, see
   "Adopting in an existing project" below before running `init`.

3. Add this to the project's `AGENTS.md` or `CLAUDE.md`:

       Work is tracked with `tasks` (see ~/.claude/skills/tasks/SKILL.md). Start every
       session with `tasks prime`, `tasks start <id>` before changing code, `tasks done <id>`
       in the same commit as the code, and `tasks check` before every commit.

4. Work:

       tasks prime                      # counts, ready list, who is doing what
       tasks ready                      # what can be worked on now
       tasks tree                       # the goal hierarchy
       tasks start <id>                 # claim it
       tasks note <id> "<one line>"     # when scope or understanding changes
       tasks done <id> "<what landed>"  # in the same commit as the code
       tasks check                      # before every commit

Never edit `tasks/*.md` by hand. `tasks --help` lists every command; add `--pretty` to any
command for human-readable output.

## Install

From a checkout:

    cargo install --path .

Without a checkout (binary only; the skill still needs the `skills/tasks` directory
from a clone):

    cargo install --git https://github.com/khughitt/tasks

## Use

    cd <repo>
    tasks init --prefix sci          # creates tasks/ and the doc roots; registers the project
    tasks add "Bank the ledger" -p 1 --size m --tag ledger
    tasks add "Emit rows" --parent sci-4f2a9c
    tasks ready                      # what can be worked on now (JSON)
    tasks tree                       # the goal hierarchy
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

1. Keep historical docs in place. By default spec links accept `docs/specs/`,
   `docs/designs/`, `docs/superpowers/specs/`, and `docs/superpowers/designs/`; plan links
   accept `docs/plans/`. If the project keeps them elsewhere, say so before `init`:

       mkdir -p tasks && cat > tasks/.config.toml <<'EOF'
       prefix = "sci"
       spec_dirs = ["design", "rfcs"]
       plan_dirs = ["planning"]
       EOF

   A configured list replaces the defaults. The roots are both the validation boundary
   and the search path for bare names, and they are project-level only so `tasks check`
   agrees on every machine.
2. `tasks init --prefix <p>`.
3. Install the skill and mention it in the project's CLAUDE.md / AGENTS.md.
4. Require `tasks prime` at session start and `tasks check` before completion. Add
   `tasks check` to automation once the binary has a pinned install source; never skip it
   conditionally when unavailable.

## Layout

    tasks/.config.toml               prefix = "sci"; optional spec_dirs / plan_dirs
    tasks/sci-4f2a9c.md              one task
    ~/.config/tasks/projects.toml    per-machine registry: prefix -> repo path
