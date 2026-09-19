---
name: tasks
description: Use when working in a repository that contains tasks/.config.toml or tasks/*.md task records.
---

# tasks

`tasks` is the repository's task tracker: one markdown file per task under `tasks/`,
managed only through the CLI. Output is JSON unless `--pretty` is given.
Task objects omit unset optional fields and empty collections; treat missing optional
keys as unset and missing task arrays as empty. `false`, `0`, and empty strings remain.
Response containers (`tasks`, `warnings`, etc.) remain present; `next: null` still means
nothing is eligible.

## Session protocol

1. `tasks prime` — roadmap (the open goal tree), closeout (goals whose work is all
   done), the ready list, and who is working on what.
2. Pick from `tasks ready` (sorted by priority, then size). Never pick an `idea`; scope it first.
   Use the `scope` skill for a deliberate idea review.
   A `shelved` task is out of active work: `list --status shelved` sees it, and
   `tasks unshelve <id>` brings it back as an idea.
   A deferred task (`defer: <date>`) is hidden from `ready`, `next`, `quiet`, and `sample`
   until its date; `ready` and `next` say in one warning how many they hid, `prime`'s
   `deferred:` line counts what is waiting and what has come due, and `tasks list --deferred`
   lists them. `start` on a deferred task works and spends the deferral.
   A session under a cutoff (`TASKS_MAX_COMPLEXITY=<low|mid|high>` set by its harness, or
   `--max-complexity <level>` on `ready`/`next`) picks only through `ready` and `next`, which
   hide tasks rated above the level and unassessed tasks and say in warnings how many they
   hid. Do not take work from `prime`'s parked or roadmap sections or from `list --parked`,
   and close goals only when `prime`'s closeout offers them. The variable is the harness
   form; the flag is for a person at a terminal.
   The one exception: an idea `next` hands you because it is parked waiting on the agent,
   which means resume its scoping, never implement it.
   `tasks list` is the wider view: open tasks by priority, or `--sort updated` /
   `--sort created` for the most recently touched or added first (`--reverse` flips it).
   Never pick a task with children; those are goals. `ready` already omits them.
   With nothing in hand, `tasks next` prints the most recently parked task waiting on the
   agent, else the first ready task, in full; `tasks next --all-projects` does the same
   across every registered project, and `tasks next
   --project <prefix>` reads one of them.
   `tasks quiet` lists work parked waiting for an idle host across every registered
   project, priority first, as resume briefs with the checkout to open; `-n 1` is the
   top of the queue and `--project <prefix>` narrows it. It is the person's bedtime
   view, not a picker: resume an entry by opening a session in the checkout it names
   and running `tasks start <id>` there.
3. Read the task's `process` and follow **Process and workspace** below before
   implementation; state the chosen path and workspace. `tasks start <id>` before
   changing code records you as owner.
   `start` also records a claim outside git, visible from every worktree of the project,
   with the session identity and a liveness handle. A task claimed by another live session
   fails with `claimed`; `tasks start --force <id>` takes it over and records that in the
   task's notes. `ready` and `next` omit live claims and explain each omission in warnings,
   and `ready` omits tasks parked waiting on the user.
   Set `TASKS_SESSION` (and `TASKS_SESSION_PID`, when a long-lived process id is available)
   when several agents share one terminal or harness process; otherwise agents that resolve
   to the same session id are indistinguishable to the claim store.
4. `tasks note <id> "<one line>"` whenever scope or understanding changes.
5. `tasks park <id> "<next step>" [--waiting-on user] [--reason <why>]` before ending a
   turn that waits on the user, or whenever you set work down. It records the next step
   and this session in the shared store, releases your claim, and leaves status alone;
   `start` resumes it. Add `--reason` when one of these fits, and leave it off otherwise:
   `review` (the user must inspect and judge an artifact), `decision` (only the user can
   decide), `approval` (you hold a recommendation and want it confirmed), `environment`
   (the checkout or machine cannot run the work), `dependency` (another task or project
   must land first), `session` (the session is ending before the work is), `capability`
   (the work needs more reasoning than this session can supply), `quiet` (the host is in use; the work is prepared and unattended and needs only an idle machine).
   Escalate on an observable trigger, not a feeling: the implementation needs a decision
   the spec or plan leaves unresolved; investigation reveals interacting behaviour outside
   the assessed scope; a bounded attempt makes no progress or has no way to establish
   correctness. Record the evidence in a note, then
   `tasks park <id> "<where it stopped and why>" --reason capability --complexity <level>`:
   the level must be at least the task's effective rating (its record, or an existing
   escalation, whichever is higher) and above your cutoff, and it is
   written to the record and to the shared store so no checkout's picker offers it under
   that cutoff again. When no level above the cutoff exists, `--waiting-on user` instead,
   so a person can decompose or reassign it. An environment or credential failure is
   `--reason environment` and never raises the rating. If the command reports that the
   escalation was not recorded, rerun it as it was.
   When a preflight or benchmark refuses on host load (CPU, GPU, load average, a
   competing application), park with
   `tasks park <id> "<the check to rerun, then what follows>" --reason quiet --waiting-on user --minutes <n>`,
   adding `--needs headless` when the desktop session itself is the load and must be
   stopped first (`idle`, the default, means the desktop may stay up but nothing else
   runs). `--minutes` is the expected wall-clock length once started and is required;
   the queue is read before bed. `quiet` is not `environment` (a missing tool or a
   restart) and not `decision` (a session the person must attend).
   `prime` lists parked work first with where it was left; `ready` omits work waiting on
   the user; `list --parked` is the picker's feed.
6. `tasks done <id> "<what landed>"` in the same commit as the code. If dependencies are
   still open, do not `--force` unless the dependency is genuinely irrelevant; say why in the message.
   `done` refuses while any descendant is open (`--force` overrides); `drop` refuses while any
   descendant is open and has no override — drop or reparent the subtree first
   (`tasks drop <child> "<why>"` / `tasks edit <child> --no-parent`).
   When the harness exports `TASKS_MODEL` (the model id it is running), every fresh
   completion stamps the record's `model` field with it — latest-completion attribution,
   cleared by a recompletion without the variable; `tasks edit --model/--no-model`
   corrects it.
   Every task an agent files carries `agent`, the harness and model that created it
   (`<harness>/<model>`, or the harness alone), from `TASKS_AGENT` when the harness
   exports it. When it is unset and you know your harness and model ids, pass
   `--agent <harness>/<model>` on `add`; when you know only the harness, pass that;
   when unsure, pass nothing — never guess. `feedback` has no flag: supply the variable
   on that invocation, `TASKS_AGENT=<harness>/<model> tasks feedback …`. Under Claude
   Code the model half is the enclosing session's model. `tasks edit --agent/--no-agent`
   corrects a stamp; an edit never reads the variable.
   Recurring sweeps use `--every 30d` (positive whole days or weeks, up to 36500 days;
   goals cannot recur). `done` closes normally, anchors the next cycle, and writes an
   occurrence note. A due recurrence appears in `ready` still marked `done`; use `start`
   before closing its next occurrence. Early runs are allowed. `list --periodic` shows
   what is coming up; `edit --no-every` stops recurrence and clears its anchor.
7. `tasks check` before committing. A failing check means a task and its plan/spec drifted apart; fix both.
   `tasks check -q` prints nothing when there are no errors and no warnings and is
   otherwise identical; hooks use it so a clean commit is silent.
8. When a goal appears under `closeout`, confirm it is met and `tasks done <id> "<verdict>"`,
   or add the children still missing.

Never edit `tasks/*.md` directly. `tasks edit <id> --title/--body/-p/--size/--complexity/--no-complexity/--process/--no-process/--tag/--depends/--spec/--plan/--step/--parent/--no-parent/--source/--no-source/--agent/--no-agent/--every/--no-every/--defer/--no-defer`
updates fields; `tasks edit <id>` with no flags opens `$EDITOR` and validates the result.
`--tag` adds a tag and leaves the rest alone, so triage keeps the tags a task arrived with;
`--rm-tag <tag>` removes one and `--no-tags` clears them all. When the project keeps a
tag dictionary (`[tags]` in `tasks/.config.toml`), `tasks tags` shows each tag's meaning:
prefer a defined tag, and add an entry when a new tag is worth keeping — `check` warns
on open tasks carrying an undefined one.
`--source <ref>` records where a task came from (a URL, a message id, a note); tasks never interprets it.
`tasks list --source <ref>` finds everything filed from one reference, matched exactly.
A sourced `add` is idempotent: when the project already holds a task with that same source
*and* title, in any status, the existing id comes back as `action: "reused"` and nothing is
written, so refiling a batch from one origin never duplicates. The other flags on a reused
call are ignored, not merged — `tasks edit` that id if the record should change.

Every write command that takes an id writes to the project that id's prefix names, so
`tasks note`, `tasks dep`, `tasks edit`, and the status commands work on a task in another
registered project without leaving the current one, or from outside every project.
`show` and bare `tree <id>` follow the same rule. A prefix matching the current project
always means this checkout; without a local project, the registry supplies the root.
Malformed local configuration still fails. `feedback` needs a local project for its `from:` tag.

The read commands say where to look instead of inferring it from an id: `list`, `ready`,
`next`, `prime`, `tree`, `tags`, and `sample` each take `--project <prefix>` for one registered
project or `--all-projects` for every reachable one. Either works from anywhere, including
outside every project. `--project` reads that project's *registered* root, so from a
worktree it is how you ask for the main checkout. `tree <id>` is the exception that needs
no flag: like `show`, `dep`, and `note`, it routes by the id's prefix, so
`tasks tree <other-prefix>-<hex>` reads that subtree from wherever you are. Passing
`--project` alongside an id names the scope explicitly and wins over the prefix.

## Process and workspace

When the project's agent instructions adopt this policy, the recorded `process`
decides whether brainstorming runs, ahead of generic Superpowers triggers:

- `direct`: the scoped task or reviewed plan settles the outcome, approach, and
  verification. Execute without brainstorming or new design/plan documents.
- `planned`: prepare a written design spec for user review, then an implementation
  plan for user review, before implementation. Reuse existing artifacts after
  verifying their contents and review state; links alone never prove approval.
- Missing (JSON key omitted): unassessed. Read the full task and relevant code, choose with
  `tasks edit <id> --process direct|planned`, and note the reason before implementation.
  Do not silently default or infer a choice from size, priority, complexity, parents,
  or document links. Ideas still require scoping.

If direct work exposes an unresolved design decision or grows beyond the scoped
task, record the evidence and change it to planned before continuing implementation.
Bounded choices covered by the task do not require that change. Both paths retain
applicable debugging, testing, verification, and code-review skills.

Both code paths use an isolated task worktree: commit the task record before creating
one with `git worktree add` under `.worktrees/`, or reuse it on resume. Then run `just
setup` when defined; otherwise run only the root guide's explicit setup command. Do not
guess an installer. Planned work creates the worktree before drafting the spec.
Read-only investigation and task-record maintenance alone need no new worktree. Explicit
user instructions to work in place win. This field does not override higher-priority
instructions; other projects must adopt the policy in their agent instructions before
relying on direct to waive mandatory brainstorming. The CLI never launches skills or
creates worktrees, and does not gate selection or `start` on process.

`add --process` and `edit --process` accept `direct` or `planned`;
`edit --no-process` clears the choice. `check` warns `process_missing` only on
doing records, including goals and plan steps. An unassessed todo is not a finding;
there is no bulk backfill requirement. Parked `phase` is a link-derived resume hint:
`process: planned` with `phase: implementing` on a link-less todo still requires
the document reviews.

## Recording work

### Lifecycle provenance

Start/resume, park and close notes automatically carry optional `harness_session`
and `harness_session_source` fields in JSON, persisted as an indented `provenance:`
JSON continuation under the note bullet. Sources are `CLAUDE_CODE_SESSION_ID`
(`claude-code:<id>`), `CODEX_SESSION_ID` or `CODEX_THREAD_ID` (`codex:<id>`);
agreeing Codex variables prefer `CODEX_SESSION_ID`. Missing/empty native input
omits both fields; invalid/conflicting input omits them and warns without preventing
the lifecycle operation. A qualified `TASKS_SESSION` conflicting with the native
key warns too; it never supplies or replaces the native source.

This is independent of claim identity and liveness: Codex without a claim override
still claims as `sid:<pid>`. Do not set `TASKS_SESSION` merely for obs.
Generated lifecycle markers are `started`, `resumed`, `done`, `dropped`,
`parked (waiting on …): <next step>`, and `completed; next due <YYYY-MM-DD>`.
Close-message notes are stamped but are not lifecycle markers; plain `tasks note`,
feedback, shelf notes and takeover commentary remain unstamped. Consumers use
generated text, not the presence of fields, to identify transitions.
User text can equal a marker; the pair-only schema cannot disambiguate that collision.

Binaries older than the provenance writer reject the whole stamped file, so every
host reading synced task files needs a current `tasks`; the README has the complete
contract.

### Task operations

- An unscoped thought: `tasks add "<title>" --status idea -b "<why>"`. Ideas never appear in `ready`.
- A thought to revisit later: `tasks add "<title>" --status idea --defer 60d` (or a date,
  `--defer 2026-11-10`); every status change clears the date, so scope it with
  `edit --status todo` first and defer it in a second command when both are wanted.
- Deliberate idea review: `/scope [<id>... | --tag <tag>] [--project <prefix>]`.
- Not now, but kept: `tasks shelve <id> "<what would bring it back>"`. Shelved work is open
  (it still blocks dependents and holds its goal open) but hidden from `list`, `ready`, and
  `prime`'s roadmap and ready sections. A surviving shelved park overlay remains visible in
  `prime` and `list --parked` for cleanup. `check` warns when open work depends on it.
  `edit --status shelved` refuses; only `shelve` writes the shelf. `tasks unshelve <id>`
  returns it to `idea`.
- A scoped task: `tasks add "<title>" -p <0-4> --size <xs|s|m|l|xl> --complexity <low|mid|high> --process <direct|planned> --tag <group> [--defer <date|Nd|Nw>] [--source <ref>] [--agent <harness>/<model>] [--spec <name>] [--plan <name> --step "<heading>"]`.
  `complexity` is the reasoning and judgment the task demands given its current spec,
  plan, and context — `low`: the approach is established, the relevant context is
  identified, and correctness has a clear check; `mid`: bounded investigation or
  implementation choices remain, scope and acceptance criteria are clear; `high`:
  substantial discovery, subtle reasoning about interacting behaviour, or an unresolved
  architectural judgment. Rate it when scoping, next to priority and size; rate ready
  work first. A precisely specified concurrent algorithm can still be `high`; touching
  many files does not make a task `high`. `edit --complexity` or `--no-complexity` is an
  explicit reassessment and clears any escalation.
  Choose process separately using **Process and workspace**: a small risky change
  can need planning; a large mechanical change can be direct. Record it at scoping
  alongside size and complexity, including on each child; it is never inherited.
- Decomposing: `tasks add "<piece>" --parent <goal>` for each part; `tasks dep` only
  for ordering between the pieces. A goal that is committed work is a `todo` with a
  body, however large; `idea` is for uncommitted thoughts. `done` refuses while any
  descendant is open (`--force` overrides); `drop` refuses while any descendant is
  open and has no override — drop or reparent the subtree first
  (`tasks drop <child> "<why>"` / `tasks edit <child> --no-parent`).
- Dispatching several agents at once: mark each self-contained task with
  `tasks edit <id> --parallel`, then `tasks ready --parallel -n <N>` for the set to hand
  out. The marker asserts only that marked tasks do not collide with *each other* — it
  says nothing about unmarked tasks or about work already in flight, so read `prime`'s
  `doing` list before dispatching. Re-examine a task's marker whenever its scope changes;
  a stale marker is a wrong assertion. `--no-parallel` clears it.
- Blocking on another project: `tasks dep <id> --on <prefix>-<hex>`; the other project must be registered (`tasks init` there).
- Work spanning projects: a goal in the hub project, then one
  `tasks add "<piece>" --project <prefix>` per affected project and one
  `tasks dep <goal> --on <piece>` each. The goal returns to `ready` when the last piece
  closes; verify and `tasks done` it then. `tasks root <id>` prints where a piece lives.
- Id collision after a merge (git add/add conflict on the same `tasks/<id>.md`): keep one file, rename the other to a fresh id, fix its `id` field, then run `tasks check` and repair any `depends` it reports.
- `tasks tree [<id>]` shows the hierarchy; `tasks edit <id> --parent <goal>` / `--no-parent` moves a task.
- Tab completion for ids and flags: `source <(TASKS_COMPLETE=bash tasks)` in `~/.bashrc`,
  or the same with `zsh` in `~/.zshrc` after `compinit`. See the README.
- Throwaway projects get a throwaway registry: `tasks init` registers globally in
  `~/.config/tasks/projects.toml`, and that entry outlives the scratch directory it
  names. For a demo, a smoke test, or anything under a temp dir, run
  `XDG_CONFIG_HOME=$(mktemp -d) tasks init --prefix <p>` so the registration dies with
  it. If you forget, `tasks unregister <prefix>` removes the entry; project files are
  untouched.
- Curating the corpus (random open tasks, one bounded maintenance pass each): the
  `curate` skill, shipped beside this one. Never part of the session protocol.

## Prefix renames and recovery

`tasks rename <old> <new>` renames a registered prefix and the project's own ids and local
references. The retired prefix keeps resolving **for as long as the project stays
registered**, so references in other projects and in prose need no edit. `unregister`
drops the project's aliases with it. Retired names are reserved, and completion offers
only live names; `check` can warn about stored retired references without rewriting them.

Start with clean `tasks/`, no live claims, and at most one git worktree. A pending rename
freezes every writer to that project, including `start`, feedback, `init --force`, and
`unregister`; reads remain available. `tasks rename <old> <new> --explain` reports the
observed recovery verdict without writes, locks, or authorization checks. Re-run the same
command without `--explain` to resume; live claims and extra worktrees still block recovery.

Recovery covers process interruption, not power loss. Outside git, once a source was
removed, only forward recovery is available. `git checkout .` alone is not an undo: new
filenames and the registry survive it. Follow the
[manual rollback procedure](../../docs/specs/2026-09-08-prefix-rename-design.md#56-undo-and-manual-recovery)
when rollback is needed: restore and verify each source before deleting its destination,
keep the destination and stop if restoration fails, restore config and registry (including
all moved aliases), and remove the inventory last.

## With superpowers

- **brainstorming**, when selected by planned process, runs against an existing task
  and attaches with `tasks edit <id> --spec <topic>`; deliverables become children with
  `--parent <id> --spec <topic>`. When a scope brief files a design task,
  brainstorming attaches there and finishes its draft design.
- **writing-plans** attaches with `tasks edit <id> --plan <topic>` and adds one child
  per `### Task N:` heading:

      tasks add "<title>" --parent <id> --plan <topic> --step "Task N: <title>" --complexity <level> --process <value>

  Choose both fields explicitly on every step child. Use `--process direct` when
  the reviewed plan settles the work; a child that still needs design is planned.
  A plan is evidence for a lower complexity rating, not a guarantee; `check` warns
  on an open step without a rating and a doing record without process.
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
