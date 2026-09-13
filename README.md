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

       Adopt the process and workspace policy in the tasks skill: the task's process
       field, not generic Superpowers triggers, decides whether brainstorming runs.
       Direct executes scoped work without brainstorming or new design/plan documents;
       planned requires user review of a written spec, then a written implementation
       plan, before implementation. Assess missing process explicitly before coding.
       Both code paths use an isolated task worktree, reusing one on resume or creating
       one with git worktree add under .worktrees/; run just setup when defined.
       Planned work creates it before drafting the spec. Explicit user overrides win.

4. Work:

       tasks prime                      # counts, ready list, who is doing what
       tasks ready                      # what can be worked on now
       tasks tree                       # the goal hierarchy
       tasks start <id>                 # claim it
       tasks note <id> "<one line>"     # when scope or understanding changes
       tasks shelve <id> "<wake condition>" # keep open work out of active views
       tasks unshelve <id>              # bring it back as an idea
       tasks done <id> "<what landed>"  # in the same commit as the code
       tasks check                      # before every commit
       tasks feedback "<about the tool>" --category friction   # file friction upstream

Never edit `tasks/*.md` by hand. `tasks --help` lists every command; add `--pretty` to any
command for human-readable output.

`start` also writes a per-project claim outside git, so every worktree sees the session and
its liveness. `ready` and `next` omit live claims with an explanatory warning. Set
`TASKS_SESSION` per agent when agents share a terminal or harness process; use
`tasks start --force <id>` for an explicit, recorded takeover.
`TASKS_MODEL` per harness process records which model completed each task: a fresh
`done` stamps the record's `model` field from it (and clears the stamp when it is
unset); correct a wrong stamp with `tasks edit --model`/`--no-model`.
`TASKS_AGENT` per harness process records which harness and model filed each task
(`<harness>/<model>`, or the harness alone): `add` and `feedback` stamp the record's
`agent` field from it, `add --agent` overrides it, and `tasks edit --agent`/`--no-agent`
corrects a stamp. The harness owns exporting it: Claude Code is wired through the ops
`claude-provenance` hook (session start and model switch, so the stamp follows
`/model`), Codex exports the harness alone, and other harnesses pass `--agent` when
they know their ids. Design:
`docs/specs/2026-09-13-creation-provenance-design.md`.
`TASKS_MAX_COMPLEXITY` per harness process is the envelope a session picks within:
`ready`, `next`, and `prime` hide tasks rated above it and unassessed tasks, and say how
many. `--max-complexity` on `ready`/`next` overrides it for one call. Design:
`docs/specs/2026-09-12-task-complexity-design.md`.
`park` sets a task down with its next step in the same store, who it waits on, and
optionally why:

| reason        | The work stopped because…                                                        |
|---------------|----------------------------------------------------------------------------------|
| `review`      | an artifact the user must inspect and judge: art sheets, screenshots, a document read |
| `decision`    | a decision only the user can make — scope, taste, priority                       |
| `approval`    | the agent holds a recommendation and wants it confirmed                          |
| `environment` | the checkout or machine cannot run the work — missing deps, a restart, a TTY     |
| `dependency`  | another task or project must land first                                          |
| `session`     | the session ended before the work did — context exhausted, time, crash           |
| `capability`  | the work needs more reasoning than this session can supply; `--complexity` raises the rating |
| `quiet`       | the host is in use; the work is prepared, unattended, and needs only an idle machine — `--minutes <n>` (required) and `--needs idle\|headless` record the recipe |

`start` resumes it, and `prime` lists parked work first. Every record also carries two
stamps written only by status changes: `started`, the first time work began, and
`completed`, the latest completion (cleared by a reopen). Design:
`docs/specs/2026-09-11-park-reason-and-stamps-design.md`.
`tasks quiet` lists quiet parks across every registered project as resume briefs (design:
`docs/specs/2026-09-13-quiet-queue-design.md`).

Statuses are `idea`, `todo`, `doing`, `blocked`, `shelved`, `done`, and `dropped`.
`shelved` is open but hidden; use `tasks shelve <id> "<wake condition>"` and
`tasks unshelve <id>` to return it as an idea.

## Install

From a checkout:

    cargo install --path .

Without a checkout (binary only; the skill still needs the `skills/tasks` directory
from a clone):

    cargo install --git https://github.com/khughitt/tasks

## Use

    cd <repo>
    tasks init --prefix sci          # creates tasks/ and the doc roots; registers the project
    tasks init --prefix sci --force  # re-point the prefix here after moving the repo
    tasks unregister sci             # drop a stale prefix and its aliases; files are untouched
    tasks rename dot dots            # rename a registered prefix; old ids still resolve
    tasks rename dot dots --explain  # diagnose an interruption without locks or writes
    tasks add "Bank the ledger" -p 1 --size m --complexity low --process direct --tag ledger --agent codex/gpt-6
    tasks add "Emit rows" --parent sci-4f2a9c
    tasks edit <id> --process planned # choose a workflow explicitly
    tasks edit <id> --no-process     # clear it to unassessed
    tasks add "Curation sweep" --every 30d  # days or weeks since each completion
    tasks list --periodic            # recurring tasks, soonest due first
    tasks edit <sweep-id> --every 2w # change the cadence; --no-every clears it and its anchor
    tasks add "Reply to Dana" --source "mail:<42@example.org>"  # where it came from; never interpreted
    tasks add "Reply to Dana" --source "mail:<42@example.org>"  # again: reuses the id, writes nothing
    tasks list --source "mail:<42@example.org>"  # what came from this reference (exact match)
    tasks ready                      # what can be worked on now (JSON)
    tasks ready --parallel -n 3      # up to 3 candidates marked safe to dispatch together
    tasks sample -n 3                # random open tasks for a curation pass (see skills/curate)
    tasks tree                       # the goal hierarchy
    tasks next                       # parked work waiting on you, else the first ready task
    tasks park <id> "next step" --reason review   # set it down; tasks list --parked to see what is parked
    tasks park <id> "rerun the preflight" --reason quiet --waiting-on user --minutes 50  # needs an idle host
    tasks quiet                      # what could run tonight, across every project; -n 1 for the top
    tasks shelve <id> "when the dependency lands" # keep open work out of active views
    tasks unshelve <id>             # return shelved work to idea
    tasks next --all-projects        # the same across every registered project
    tasks prime --project fam        # read another registered project; also list, ready,
                                     #   next, tree, tags, sample. Needs no local project.
    tasks projects                   # the registry: reachable? counts?
    tasks add "Piece" --project fam  # create in another registered project
    tasks note fam-0c3d7e "…"        # id-taking commands follow the prefix to its project
    tasks edit sci-4f2a9c --tag cli --rm-tag triage  # --tag adds; --rm-tag/--no-tags remove
    tasks --pretty ready             # same, as a table (or export TASKS_FORMAT=pretty)
    tasks --pretty --color auto ready # color when stdout is a terminal
    tasks start sci-4f2a9c
    tasks note sci-4f2a9c "spec §4 no longer holds"
    tasks done sci-91be03 "rows emitted"
    tasks done sci-4f2a9c "landed in 1a2b3c"  # open-work rule: closes once its child is closed
    tasks check                      # validate files, links, plan steps, dependencies

Commands targeting an existing id, including `show`, `tree <id>`, `note`, and `start`,
work from outside every project: the prefix selects the registered root. Inside a
project, a matching prefix keeps the current checkout, including a worktree selected
with `-C`. `feedback` still needs a local project for its provenance tag.

A recurring task closes normally: `done` records the completion and anchors its next
cycle. When due, it appears in `ready` with status `done`; use `start` before completing
the next occurrence. Early runs are allowed. `--every` accepts positive whole days or
weeks (for example, `30d` or `2w`), up to 36500 days; goals cannot recur.

Run `tasks --help` for the full command list.

The registry maps live prefixes to project roots and retired prefixes to their current
live name. `tasks rename <old> <new>` updates the project's filenames, ids, local
`depends`/`parent` references, config, and registry. References in other projects and in
prose need no edits: retired names keep resolving **for as long as the project stays
registered**. `unregister` removes that project's aliases too; retired names cannot be
reused while registered.

Rename requires clean `tasks/`, no live claims, and at most one git worktree. An interrupted
rename freezes writes to that project; reads remain available. Re-run the same command to
resume, or use `--explain` to observe its recovery verdict without writing, locking, or
checking authorization. Outside git, rename warns that forward recovery is the only
option after source removal. `git checkout .` alone does not undo a rename; see
[manual recovery](docs/specs/2026-09-08-prefix-rename-design.md#56-undo-and-manual-recovery).

## Choosing a process

Set `--process direct|planned` on `add` or `edit`; `edit --no-process` clears it.
Choose it alongside size and complexity when scoping work, including each plan child.
It is never derived from those fields, parentage, or document links.

| Process | Agent workflow |
|---------|----------------|
| `direct` | Execute the scoped task or reviewed plan without brainstorming or new design/plan documents. |
| `planned` | Review a written design spec with the user, then review a written implementation plan, before implementing. |
| Unassessed | Read the task and relevant code, explicitly record the choice and its reason before implementation. |

Both code paths retain appropriate debugging, tests, and review, and use an isolated
task worktree under the adopted policy. A direct task that uncovers an unresolved
design decision or grows beyond its scope needs a note and reassessment to planned.
Ideas still need scoping. Existing document links do not establish approval.

JSON task, summary, and parked rows expose `process` as a string or null. Pretty
summary and parked rows show a process column (`-` when unassessed); show and next
print `Process: direct`, `Process: planned`, or `Process: unassessed`. The parked
`phase` remains a link-derived resume hint: a todo without document links can show
`phase: implementing` alongside `process: planned`, which still requires both reviews.

`tasks check` warns `process_missing` only for doing records without a choice,
including goals and plan steps. Unassessed todos need no backfill sweep. The CLI
does not change readiness, ordering, or `start` eligibility based on process, and
does not run skills or create worktrees.

Projects must adopt the process policy in their agent instructions (see the install
snippet above) before it can take precedence over generic brainstorming triggers.
This repo has adopted it. The separate global worktree-rule widening landed as
`ai-69ccac` (2026-09-13); it fixes the ran-on-main incident without a CLI change. The
writing-plans integration lives in the tasks skill's child command
(`--complexity <level> --process <value>`), not in the upstream plan-writing skill;
`ai-e8dcc5` confirmed there is no locally owned copy to change. These rollout tasks
are separate from this repo's implementation. Design: `docs/specs/2026-09-13-task-process-design.md`.

## Completions

Bash and zsh complete subcommands, flags, `--status`/`--size`/`--sort`/`--color`
values, registered project prefixes, and task ids — `tasks show sci-4f<TAB>`. The id
candidates come from the project the command would actually act on, so `-C`,
`--project`, and a typed foreign prefix all steer them.

Bash, in `~/.bashrc`:

    source <(TASKS_COMPLETE=bash tasks)

Zsh, in `~/.zshrc`, **after** completion is initialized — the stub calls `compdef`, so
sourcing it before `compinit` fails with `command not found: compdef`:

    autoload -Uz compinit && compinit      # or your framework's own init
    source <(TASKS_COMPLETE=zsh tasks)

Under oh-my-zsh, prezto, or a plugin manager, the same rule applies: the stub goes after
that framework's initialization.

Re-source the stub or open a new shell after upgrading `tasks`; a running shell keeps the
function it loaded at startup. Fish, elvish, and powershell use the same mechanism with
their own syntax. `TASKS_COMPLETE=` or `TASKS_COMPLETE=0` disables completion.

Color is off unless you ask for it. `--color auto|always|never`, or `TASKS_COLOR` with the
same three values, styles `--pretty` output only; JSON never carries escape sequences.
`auto` colors a stream only when that stream is a terminal, so putting `TASKS_COLOR=auto`
in a shell rc leaves piped and agent-run output plain. A non-empty `NO_COLOR` turns off
color selected through the environment, and an explicit `--color` overrides it.

## Agent skill

`skills/tasks/SKILL.md` teaches agents the session protocol. Install it once at user level
so it applies to every project:

    mkdir -p ~/.claude/skills && ln -s "$PWD/skills/tasks" ~/.claude/skills/tasks
    mkdir -p ~/.agents/skills && ln -s "$PWD/skills/tasks" ~/.agents/skills/tasks   # other harnesses
    ln -s "$PWD/skills/curate" ~/.claude/skills/curate
    ln -s "$PWD/skills/curate" ~/.agents/skills/curate   # other harnesses
    ln -s "$PWD/skills/scope" ~/.claude/skills/scope
    ln -s "$PWD/skills/scope" ~/.agents/skills/scope     # other harnesses

or per project, when a project needs to pin its own copy:

    mkdir -p <repo>/.claude/skills && cp -r skills/tasks <repo>/.claude/skills/tasks

`tasks init` warns when neither location has the skill.

`skills/curate/SKILL.md` is the maintenance pass: `/curate` samples open tasks and
refines them within fixed bounds.

`skills/scope/SKILL.md` is the deliberate idea review between capture and maintenance:
`/scope` turns a bounded cluster into supported next actions and handoffs.

## Feedback

When the tool itself gets in the way, cannot do something needed, suggests an improvement,
or works notably well, file it from wherever you are:

    tasks feedback "<one line about the tool>" --category <friction|gap|idea|positive> [-b "<detail>"]

The entry lands as an `idea` in whichever checkout is registered under the `tasks` prefix,
tagged `feedback`, the category, and `from:<your prefix>`. A repeat of the same one-liner
appends a note to the open entry instead of creating a duplicate; `--recur <id>` and
`--new` settle an `ambiguous` result. The command never commits: this repository is public,
so a person here reviews each uncommitted file before it becomes public.

## Adopting in an existing project

1. Keep historical docs in place. By default spec links accept docs/specs/,
   docs/designs/, docs/superpowers/specs/, and docs/superpowers/designs/; plan links
   accept docs/plans/ and docs/superpowers/plans/. If the project keeps them elsewhere,
   say so before `init`:

       mkdir -p tasks && cat > tasks/.config.toml <<'EOF'
       prefix = "sci"
       spec_dirs = ["design", "rfcs"]
       plan_dirs = ["planning"]
       EOF

   A configured list replaces the defaults. The roots are both the validation boundary
   and the search path for bare names, and they are project-level only so `tasks check`
   agrees on every machine.

   The same file may carry the project's tag dictionary, one line per tag with a
   specific meaning:

       [tags]
       testing = "Tests, gates, and CI."
       perf = "Speed or memory."

   `tasks tags` prints the meaning beside each tag, and `tasks check` warns
   `undefined_tag` for an open task carrying a tag the table does not define; a project
   without the table is not held to anything. A new metadata idea starts here as a tag
   plus an entry, and is promoted to a proper field only once it has earned validation
   and a JSON key — then the entry says so and `check` flushes the tag. Design:
   `docs/specs/2026-09-11-tag-dictionary-design.md`.
2. `tasks init --prefix <p>`.
3. Install the skill and mention it in the project's CLAUDE.md / AGENTS.md.
4. Require `tasks prime` at session start and `tasks check` before completion. Add
   `tasks check` to automation once the binary has a pinned install source; never skip it
   conditionally when unavailable.

## Layout

    tasks/.config.toml               prefix = "sci"; optional spec_dirs / plan_dirs / [tags]
    tasks/sci-4f2a9c.md              one task
    ~/.config/tasks/projects.toml    per-machine registry: live prefix -> repo path; retired -> live
