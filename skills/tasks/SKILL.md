---
name: tasks
description: Use when working in a repository that contains tasks/.config.toml or tasks/*.md task records.
---

# tasks

`tasks` is the repository's task tracker: one markdown file per task under `tasks/`,
managed only through the CLI. Output is JSON unless `--pretty` is given (`check` prints
nothing when it has nothing to report).
Task objects omit unset optional fields and empty collections; treat missing optional
keys as unset and missing task arrays as empty. `false`, `0`, and empty strings remain.
Response containers (`tasks`, `warnings`, etc.) remain present; `next: null` still means
nothing is eligible. `tasks doc <id> --plan|--spec` is the exception: exact UTF-8
body bytes on stdout in every format, with warnings on stderr and no JSON wrapper.
Summary and parked rows expose `step` only when set. `check` distinguishes
`doc_unreachable` (an unavailable reference) from `doc_unchecked` (one summary of
external checks unfinished at the shared budget); warning-only exit 0 does not
mean those documents were verified.

## Session protocol

1. `tasks prime` — lanes (the efforts meant to run side by side, each with its guidance
   and the step it could take now), roadmap (the open goal tree), closeout (goals whose
   work is all done), the ready list, and who is working on what.
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
   A session that cannot meet a shared resource says so: `TASKS_WITHOUT=quiet` (comma-
   separated) in a host's ordinary session environment, or `--without <need>` on
   `ready`/`next`/`prime`/`lanes`, hides tasks that need it: `ready`, `next`, and `prime`
   with one warning counting them, `lanes` under its `without` cause.
   The flag refuses a name no project in scope declares; the variable never errors.
   A TTY session handed the idle host runs without the variable.
   The one exception: an idea `next` hands you because it is parked waiting on the agent,
   which means resume its scoping, never implement it.
   `tasks list` is the wider view: open tasks by priority, or `--sort updated` /
   `--sort created` for the most recently touched or added first (`--reverse` flips it).
   `list` and `ready` also take `-p/--priority`, `--size`, `--complexity`, and `--process`,
   each repeatable: repeats of one of these widen (any of them), and different flags
   narrow (all of them). `--size`, `--complexity`, and `--process` accept `none` for an
   unset field; `--priority` does not, since every task has one. `--tag` stays all-of: a
   task must carry every tag given. `--owner`, `--source`, and `--parent` take one value,
   and `--parallel` is a switch. `--need` is all-of, like `--tag`. `--complexity` is a selection over the effective rating,
   not the session cutoff: a session under a cutoff still picks only through `ready` and
   `next`.
   `--needs-review` keeps rows that still need a look, each with a `review_reason`:
   `unreviewed` ideas, `woken` and `stranded` reviewed ideas (no recorded wait: open
   dependency, pending proposal, `awaiting_user`, or future deferral), and `unassessed`
   leaf todo or blocked tasks with no process. Pretty tables mark those rows with a
   leading `*`; together they are the scope queue, which `/scope` with no ids draws from.
   `note --reviewed --awaiting-user` records that a reviewed idea waits on the user;
   `--no-reviewed` readmits it with the answer.
   `tasks search "<phrase>"` is `list` narrowed to tasks whose title or body contains the
   phrase (literal, case-insensitive; notes are not searched) and takes every `list`
   option, so `tasks search "<phrase>" --all-projects` (open work, shelved aside) is the
   duplicate check before filing.
   Never pick a task with children; those are goals. `ready` already omits them.
   A parent must be in the same project, so a goal whose pieces live in other projects
   holds them as `depends` and is a leaf to the tool: `ready` offers it once its pieces
   close, for the closeout. Rate it as that closeout (usually `--process direct`), which
   also takes it out of the `*` queue.
   A lane (`lane: true`) is a goal too, even before it has children, and every row names
   its lane in `in_lane` (the nearest lane at or above it). A session committed to one
   lane picks with `tasks next --under <lane>`; `--under` also narrows `list` and `ready`
   to descendants at any depth, while `--parent` stays direct children. Without `--under`,
   `next` keeps its priority order. `tasks lanes` answers what can run in parallel: each
   lane is `ready` with its pick, `held` on an exclusive need, `waiting` with its causes,
   `paused`, or `empty`.
   With nothing in hand, `tasks next` prints the most recently parked task waiting on the
   agent, else the first ready task, in full; `tasks next --all-projects` does the same
   across every registered project, and `tasks next
   --project <prefix>` reads one of them.
   `tasks quiet` lists work parked waiting for an idle host across every registered
   project, priority first, as resume briefs with the checkout to open; `-n 1` is the
   top of the queue, and `--project <prefix>`
   or `--group <name>` narrows it. It is the person's bedtime
   view, not a picker: resume an entry by opening a session in the checkout it names
   and running `tasks start <id>` there.
3. Read the task's `process` and follow **Process and workspace** below before
   implementation; state the chosen path and workspace. `tasks start <id>` before
   changing code records you as owner.
   `start` also records a claim outside git, visible from every worktree of the project,
   with the session identity and a liveness handle. A task claimed by another live session
   fails with `claimed`; `tasks start --force <id>` takes it over and records that in the
   task's notes.
   A claim whose pid this command cannot judge (recorded in another PID view, such as
   a sandbox's, or before views were recorded) is held and shows as `unverifiable`.
   Its own view, its owner, a reboot or `start --force` releases it. When you know its
   holder is gone (a finished sandbox, a removed container), `tasks release <id> "<why>"`
   removes it with one note and leaves status, owner and park alone; a live claim is
   refused.
   Before this host adopts a prefix another host renamed, a claim recorded under the old
   prefix is released with `tasks release <new id> "<why>" --adopting <old>`, which
   `tasks rename <old> <new> --adopt` names when it refuses.
   A write refuses with `stale_copy` when another checkout holds a newer copy of the
   record, and the error says what to do: rerun there with the printed `tasks -C <root> …`,
   merge the main checkout's copy into your worktree, or leave the record to the session
   named as working there. There is no override. A write by the claim's holder moves the
   claim to the checkout it lands in, and `show` finds a task that exists only in the
   checkout its claim or park names.
   A write missing that local record keeps `task_not_found`, names the recorded checkout (or says it is unavailable), and offers a `tasks -C` retry only when no other session's claim or park occupies it; no write is routed automatically.
   `ready` and `next` omit live claims and explain each omission in warnings,
   and `ready` omits tasks parked waiting on the user.
   An open task tagged `halt` in the registered checkout stops new lower-priority starts
   in that project, including from older worktrees. `prime`, `ready`, and `next` name
   the halt and show eligible work; an already doing task may resume. To override a
   halt, use `tasks start <id> --force --reason "<one-line explanation>"`. The reason
   is recorded on both the halt and the started task. Start's reason is free text;
   park's `--reason` below is a fixed choice from its own vocabulary.
   A need declared `exclusive` names a host resource (an idle host, a device) and serves
   one session at a time across every project on the host that uses the name. `start`
   records the task's exclusive needs as holds on its claim. `ready`, `next` and `prime`
   hide other sessions' steps that need a held resource, with a warning naming the holder.
   Starting such a step (or `edit --status doing`) fails with `need_held`. Override it
   deliberately with `tasks start <id> --force --reason "<why>"`, which notes the task and
   a same-project holder. Your own session may start more steps that need what it already
   holds. A park or a dead session releases the hold. A long run that keeps the resource
   should heartbeat with `tasks note`, or a pid-less claim lapses with its TTL. Change the
   needs of a claimed task only under your own claim (`start --force` takes it over first).
   Adding an exclusive need that another session holds takes `tasks edit <id> --need <n>
   --force --reason "<why>"`.
   Set `TASKS_SESSION` (and `TASKS_SESSION_PID`, when a long-lived process id is available)
   when concrete native identity is unavailable and several agents share one terminal or
   harness process. Distinct Codex threads already supply separate native identities;
   unnecessary explicit overrides bypass native root association.
   On a host with relay identity enabled (`[identity] relay = true` in
   `~/.config/tasks/config.toml`), a claim is keyed by the relay agent id
   `<harness>:<sessionId>`, so it belongs to the session rather than to the terminal. The
   owner can still `park` and `done` a claim it already holds while the registry is
   unavailable; resuming a parked task needs the registry back, because parking releases
   the claim and resuming is a fresh acquisition. Any relay identity error is resolved by
   setting `TASKS_SESSION` and `TASKS_SESSION_PID`, which sit above the level.
4. `tasks note <id> "<one line>"` whenever scope or understanding changes.
5. `tasks park <id> "<next step>" [--waiting-on user] [--reason <why>]` before ending a
   turn that waits on the user, or whenever you set work down. It records the next step
   and this session in the shared store, releases your claim, and leaves status alone;
   `start` resumes it. A host pointer a live test repointed into the task's worktree (a
   launcher symlink, a service unit, a config include) is restored before the park, or
   the park text names it: the record is where the next session learns of it. Add
   `--reason` when one of these fits, and leave it off otherwise:
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
   `--reason environment` and never raises the rating.
   If any park fails after its note landed (the note is there, but the task was not
   parked), rerun the same command; the rerun adds a second note.
   When a preflight or benchmark refuses on host load (CPU, GPU, load average, a
   competing application), park with
   `tasks park <id> "<the check to rerun, then what follows>" --reason quiet --waiting-on user --minutes <n>`,
   adding `--needs headless` when the desktop session itself is the load and must be
   stopped first (`idle`, the default, means the desktop may stay up but nothing else
   runs). `--minutes` is the expected wall-clock length once started and is required;
   the queue is read before bed. The next step lists the run's phases and their minutes
   (for example build, run, analysis), plus any refusal or hang an earlier attempt of the
   same recipe hit. The person then knows what can go wrong before they give up the
   machine. `quiet` is not `environment` (a missing tool or a restart) and not
   `decision` (a session the person must attend).
   Every attempt at a quiet run ends with one note, refused attempts included:
   `tasks note <id> "run: <actual> min (est <n>, <needs>); <phase> <m>, …; <outcome>[: <cause>]"`.
   `<outcome>` is one of `passed`, `failed` (a check failed), `refused` (a preflight or
   settle gate stopped it; time the attempt up to the refusal), `hung` (killed after it
   stopped making progress), or `aborted` (stopped for any other reason). The agent that
   resumes the task writes it. For a run the person started from a TTY, or one launched
   detached, take the times from the run's own output. Example:
   `run: 69 min (est 40, idle); preflight 1, build 4, smoke 64; hung: trace export`.
   Tools that compare estimates with actuals parse this form, so keep it exact.
   `prime` lists parked work first with where it was left; a park this host kept after
   the record closed elsewhere is a warning naming the command that clears it (`tasks done
   <id>`, or `tasks drop <id> "<why>"`), not a row. `ready` omits work waiting on
   the user; `list --parked` is the picker's feed. `list --parked --waiting-on user
   --all-projects` is the person's queue: everything parked on them, with each next step.
   `--headers` labels the columns of any pretty table.
6. `tasks done <id> "<what landed>"` on the task's branch before it merges (under `[git] commit`
   the CLI commits the record itself; otherwise commit it with the code). If dependencies are
   still open, do not `--force` unless the dependency is genuinely irrelevant; say why in the message.
   To reopen a done or dropped task (a correction handed back after close), run
   `tasks edit <id> --status todo`, then `tasks start <id>`; `start` alone refuses and names
   that edit.
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
   A clean check prints nothing and exits 0; findings print as JSON, and errors exit 1.
8. When a goal appears under `closeout`, confirm it is met and `tasks done <id> "<verdict>"`,
   or add the children still missing.

Never edit `tasks/*.md` directly. `tasks edit <id> --title/--body/-p/--size/--complexity/--no-complexity/--process/--no-process/--lane/--no-lane/--tag/--depends/--no-depends/--spec/--no-spec/--plan/--no-plan/--step/--no-step/--parent/--no-parent/--source/--no-source/--agent/--no-agent/--every/--no-every/--defer/--no-defer/--need/--rm-need/--no-needs`
updates fields; `tasks edit <id>` with no flags opens `$EDITOR` and validates the result.
Notes stay append-only there: the one note change it accepts is removing trailing spaces
and tabs, which every new note already has stripped.
`--tag` adds a tag and leaves the rest alone, so triage keeps the tags a task arrived with;
`--rm-tag <tag>` removes one and `--no-tags` clears them all. `--depends` likewise adds;
`--no-depends` clears, and with `--depends` replaces the list. When the project keeps a
tag dictionary (`[tags]` in `tasks/.config.toml`), `tasks tags` shows each tag's meaning:
prefer a defined tag, and add an entry when a new tag is worth keeping — `check` warns
on open tasks carrying an undefined one.

A step that uses a shared resource records it with `--need <name>` (repeatable), drawn
from the project's `[needs]` vocabulary in `tasks/.config.toml`:
`[needs.quiet]` with `meaning = "<one line>"` and, for a resource only one session can use
at a time, `exclusive = true`. Name an exclusive need after the host resource it stands
for: exclusive names are one namespace across every project on the host. Prefer a need to
an ad hoc `needs-*` tag. `add`/`edit` refuse an undeclared name (`unknown_need`), and
`check` errors on a record naming one; add the vocabulary entry first. `edit --need`
appends, `--rm-need` removes, and `--no-needs` clears. Change status and needs in
separate operations: `edit --status` refuses the needs flags, and an editor save may not
change both.

Project identity colors are assigned by an optional top-level `color = "#RRGGBB"` in
`tasks/.config.toml`, before any table headers. Use exactly six hex digits (either case);
invalid values fail with a typed config error. Change the value to recolor or remove the
key to restore existing styling. Init/reinitialization and prefix rename preserve it.
Pretty output paints a project's name in that RGB: prefixes in
`projects`, `groups` and `where`; task ids and titles in table views, and in `show`/`next`
(the overview line included) by the project each id belongs to. Ids take a lighter shade
of the color than titles (barely lighter for a near-white color). `claims` stays uncolored. Unassigned or unreachable projects keep existing styling;
other columns keep their roles. Existing `--color`, `TASKS_COLOR`, `NO_COLOR`, and auto
controls apply; stored IDs are unchanged. JSON carries the color as lowercase
`color: "#rrggbb"` on `projects` rows, `where`'s `project`, and resolved `resolve` rows
(omitted when unset or unreachable, and by `resolve` with a warning when a config cannot
be read): read it there, never from config files.

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
Malformed local configuration still fails. `feedback` works from anywhere: outside every
project it reports as `from:unregistered`.

A linked project keeps its records outside its code checkout: its `tasks/.config.toml`
carries `[checkout] path`, and every command run anywhere in that checkout or its
worktrees opens the linked project (`tasks init --prefix <p> --checkout <path>` in the
overlay directory makes one). Claims and parks name the code worktree, so resume there;
records, specs and plans are read and written in the project root, never in the code
tree. `tasks where [PATH]` prints which project a path opens (`linked` says whether
through the link) and whether its repository keeps records; two projects declaring one
checkout is `checkout_conflict`. The rules that tie a record to the code read through the
overlay there: records commit in `<prefix>/tasks/` in the overlay repository after the
code commit (under `[git] commit` the CLI makes that commit), and the record committed
before `git worktree add` is committed there; the worktree itself is added in
the code checkout, and `tasks start <id>` in it moves the claim. Task ids never appear in
the code repository: its commits, branches, pull requests and docs cite the repository's
own issue key when there is one, which the task records with `--source`.

The read commands say where to look instead of inferring it from an id: `list`, `ready`,
`next`, `prime`, `lanes`, `tree`, `tags`, and `sample` each take `--project <prefix>` for one
registered project, `--group <name>` for the members of a project group, or
`--all-projects` for every reachable one. Each works from anywhere, including
outside every project. `--project` reads that project's *registered* root, so from a
worktree it is how you ask for the main checkout. `tree <id>` is the exception that needs
no flag: like `show`, `dep`, and `note`, it routes by the id's prefix, so
`tasks tree <other-prefix>-<hex>` reads that subtree from wherever you are. Passing
`--project` alongside an id names the scope explicitly and wins over the prefix.

A project group is a named set of registered projects, declared in this host's registry
and not synced. Each host declares its own groups.

- **Managing groups.** `tasks group set <name> <prefix>...` creates or replaces a group,
  and a retired prefix resolves to its live name. `tasks group rm <name>` deletes a group.
  `tasks groups` lists each group with whether each member is reachable.
- **Names.** Names use the tag grammar (lowercase letters, digits, `-`). A name cannot be
  a live or retired prefix, or a prefix an unfinished rename reserves. Registering a
  prefix (`init`, and `init --force` re-pointing one) and `rename` refuse a prefix that
  names a group. Groups may overlap.
- **Membership changes.** `rename` carries membership to the new prefix. `unregister`
  drops the prefix from every group, and deletes, with a warning, any group it leaves
  empty.
- **Reading a group.** Under `--group`, `prime` adds `group` and its `prefix` is null.
  Warnings about unreachable projects or unknown halt state name members only. A group
  whose members are all unreachable gives those warnings and empty results.
- **Errors and limits.** A misspelled name is `unknown_group`. `tasks claims` always reads
  every store.

## Process and workspace

When the project's agent instructions adopt this policy, the recorded `process`
decides whether brainstorming runs, ahead of generic Superpowers triggers:

- `direct`: the scoped task or reviewed plan settles the outcome, approach, and
  verification. Execute without brainstorming or new design/plan documents.
- `planned`: prepare a written design spec for user review, then an implementation
  plan for user review, before implementation. Reuse existing artifacts after
  verifying their contents and review state; links alone never prove approval.
  Brainstorming's own sizing step takes the written-design size, and skipping its
  questions does not waive the spec. When its conversation shows the work is smaller,
  record the evidence and change the process to `direct` before skipping the design,
  the mirror of the direct-to-planned rule below.
- Missing (JSON key omitted): unassessed. Read the full task and relevant code, choose with
  `tasks edit <id> --process direct|planned`, and note the reason before implementation.
  Do not silently default or infer a choice from size, priority, complexity, parents,
  or document links. Ideas still require scoping.

If direct work exposes an unresolved design decision or grows beyond the scoped
task, record the evidence and change it to planned before continuing implementation.
Bounded choices covered by the task do not require that change. Both paths retain
applicable debugging, testing, verification, and code-review skills.

Both code paths use an isolated task worktree: commit the task record before creating
one with `git worktree add` (automatic under `[git] commit`: `add` and `start` commit it)
under `.worktrees/`, or reuse it on resume. Then run `just setup` when defined; otherwise run only the root guide's explicit setup command. Do not
guess an installer. After creating a worktree for a task you have started, run
`tasks start <id>` there before any other `tasks` command for it: the claim moves there,
and a later write from the main checkout refuses instead of forking the record.
Planned work creates the worktree before drafting the spec.
Run the worktree's code for a live test by explicit path or an environment override,
never by repointing a shared launcher or symlink on the host; a pointer that must be
repointed is noted when it happens (what, from, to, how to restore) and restored before
`park` or `done`, and a worktree is not removed while a host pointer resolves into it.
Read-only investigation and task-record maintenance alone need no new worktree. Explicit
user instructions to work in place win. This field does not override higher-priority
instructions; other projects must adopt the policy in their agent instructions before
relying on direct to waive mandatory brainstorming. The CLI never launches skills or
creates worktrees, and does not gate selection or `start` on process.

A record-only commit still runs content checks, and those read the working tree and
the host's tasks registry, not the commit, so it can be refused for content it does
not contain. Never bypass the hook or carry the record by patch; name the check and
paths and ask the user, parking `--waiting-on user --reason decision` until they answer:
- Failure committed in HEAD: on an explicit work-in-place instruction, repair it in
  place, commit the repair by pathspec (`git commit -m "…" -- <paths>`) so the staged
  record stays out, then commit the record and create the worktree.
- Unrelated uncommitted edits (someone's draft): with their consent, `git stash push --
  <paths>`, commit the record, `git stash pop`; confirm the commit holds only the record,
  since a hook may regenerate and stage a guide the stash left clean. Never edit or
  commit their draft.
- A cross-project reference whose project is unregistered or unreachable on this host:
  register or sync that project, then commit.

`add --process` and `edit --process` accept `direct` or `planned`;
`edit --no-process` clears the choice. `check` warns `process_missing` only on
doing records, including goals and plan steps. An unassessed todo is not a finding;
there is no bulk backfill requirement. Parked `phase` is a link-derived resume hint:
`process: planned` with `phase: implementing` on a link-less todo still requires
the document reviews.

Codex claim `root_session`, when present, is `codex:<root>` grouping metadata,
never an immediate parent or ownership authority. Same-owner refresh without root
data retains it; conflicting known roots refuse until the owner restores exports
or releases with `park` before intentional reassociation. Takeovers record the
new owner's root. Older claim writers can discard it. Relay worker acquisition
uses the worker's own row, selected only by its `CODEX_THREAD_ID`, and refuses a
worker relay has not registered; Codex proof-only continuation needs
a matching concrete thread even when process ancestry agrees. A legitimate owner
missing that thread can recover with the exact held `TASKS_SESSION` from `show`;
never fabricate a thread or use the command PID as a liveness handle.

## Worker handoff

A worker releases its claims before ending its assignment: `done` if responsible
for closing completed work, otherwise `park --waiting-on agent` naming the
controller's next action and authoritative worktree. Verify release before
reporting completion. The controller runs `start` before review edits or closure.
An agent-waiting park is visible to every agent and is not reserved for the
controller; its next-step text is a convention, not an ownership restriction.
If a worker exited without release, the controller may use explicit `start
--force --reason` recovery only after its own wait/completion result for that
specific worker establishes the assignment has stopped, with no pending tool,
check or background process. A timeout, silence, the root session's state, and
the shared harness PID are not that evidence. If completion reports work still
running or the state is uncertain, reconcile it with the worker and keep waiting;
do not take over. Inspect the worker's result and claim before takeover. The
worker may be resumable later; do not resume it on the task after taking over
without a new ownership handoff. No worker closes a controller's live claim,
and no controller closes a worker's live claim merely by being its parent.

## Recording work

### Record commits

Set `[git] commit = true` in `tasks/.config.toml` to opt in; absent or false keeps manual commits.
Each write commits the files it wrote (records, attachments and configuration), plus whatever the repository's own pre-commit hook stages, once per git working tree. Staging another git process makes while the commit runs can land in it.
`commit_unavailable:` means git is missing or the root is outside git.
`commit_deferred:` means staged changes, detached HEAD, or an in-progress git operation prevented the commit; clear the cause first.
`commit_failed:` means a git operation or hook failed; resolve the cause, using the record-commit routes above for a refusing hook.
A successful write still exits 0 when its commit is skipped or fails; read its warnings.
`cleanup needs attention:` means cleanup failed or the index could not be checked; inspect the named paths and index before retrying.
`tasks commit [--project <p>]` recovers skipped commits or sweeps every uncommitted file under `tasks/` into one commit, regardless of the switch; it exits 1 when it cannot commit.

### Lifecycle provenance

Start/resume, park and close notes automatically carry optional `harness_session`
and `harness_session_source` fields in JSON, persisted as an indented `provenance:`
JSON continuation under the note bullet. Sources are `CLAUDE_CODE_SESSION_ID`
(`claude-code:<id>`), `CODEX_SESSION_ID` or `CODEX_THREAD_ID` (`codex:<id>`);
Codex selects `CODEX_THREAD_ID` whenever present, even for equal IDs, otherwise
`CODEX_SESSION_ID`. Unequal valid IDs are supported; historical source values
remain unchanged and the serialized pair keeps its existing shape. Missing/empty native input
omits both fields; invalid/conflicting input omits them and warns without preventing
the lifecycle operation. A qualified `TASKS_SESSION` conflicting with the native
key warns too; it never supplies or replaces the native source.

This is independent of claim identity and liveness, which read the same Codex
variables on their own: a Codex claim is keyed by the thread id, carries no pid, and
lives by the TTL. Do not set `TASKS_SESSION` merely for obs.
Generated lifecycle markers are `started`, `resumed`, `done`, `dropped`,
`parked (waiting on …): <next step>`, and `completed; next due <YYYY-MM-DD>`.
Close-message notes are stamped but are not lifecycle markers; plain `tasks note`,
feedback, shelf notes and takeover commentary remain unstamped.
`tasks note <id> "<text>" --stamp` stamps one note the same way (use it for gate notes);
its text is never interpreted, so it is not a marker either. Consumers use
generated text, not the presence of fields, to identify transitions.
User text can equal a marker; the pair-only schema cannot disambiguate that collision.

Binaries older than the provenance writer reject the whole stamped file, so every
host reading synced task files needs a current `tasks`; the README has the complete
contract.

### Task operations

An id copied from a sentence or a list may carry trailing punctuation: every positional
task id and id-valued flag (`--parent`, `--depends`, `dep --on/--rm`, `feedback
--recur`) strips a trailing run of ASCII periods and commas before validation. Embedded
or leading punctuation, other punctuation, and whitespace are still rejected, and
records and output stay canonical.

Inside a project, every one of those inputs also accepts the bare six-hex suffix:
`tasks show 4f2a9c` is `tasks show sci-4f2a9c` when the cwd (or `-C`) is in `sci`. The
suffix always means the project you stand in, even when `--project` or another id sends
the command elsewhere, and a suffix missing locally is not found rather than searched for.
Outside every project it is `invalid_id`; use the full id there and for other projects.

- An unscoped thought: `tasks add "<title>" --status idea -b "<why>"`. Ideas never appear in `ready`.
- A thought to revisit later: `tasks add "<title>" --status idea --defer 60d` (or a date,
  `--defer 2026-11-10`); every status change clears the date, so scope it with
  `edit --status todo` first and defer it in a second command when both are wanted.
- Deliberate idea review: `/scope [<id>... | --tag <tag>] [--project <prefix>]`. The same
  skill re-scopes a `todo` or `blocked` task named by id: re-check its premises, then narrow it.
- Not now, but kept: `tasks shelve <id> "<what would bring it back>"`. Shelved work is open
  (it still blocks dependents and holds its goal open) but hidden from `list`, `ready`, and
  `prime`'s roadmap and ready sections. A surviving shelved park overlay remains visible in
  `prime` and `list --parked` for cleanup. `check` warns when open work depends on it.
  `edit --status shelved` refuses; only `shelve` writes the shelf. `tasks unshelve <id>`
  returns it to `idea`.
- A scoped task: `tasks add "<title>" -p <0-4> --size <xs|s|m|l|xl> --complexity <low|mid|high> --process <direct|planned> --tag <group> [--need <name>]... [--defer <date|Nd|Nw>] [--source <ref>] [--agent <harness>/<model>] [--spec <name>] [--plan <name> --step "<heading>"]`.
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
- Move the same work to another project: `tasks move <id> --to <prefix>` from the
  source's registered checkout. The task must be open, unparented, childless and
  unclaimed; refusals name the edit or park that settles them, and destination
  validation still applies. It preserves fields, notes and live attachments, and carries
  this host's park and escalation. The source becomes a read-only `moved` tombstone;
  writes and new references refuse with `moved` and name the new id. Existing
  dependents follow the pointer; their owners swap references with the command in
  `check`'s `moved_dep` warning. Under `[git] commit` the move commits each opted-in root;
  otherwise commit each root its `paths` lists. Rerun the same command after interruption;
  `--explain` diagnoses without writes or locks. While pending, writes to both ids and both projects'
  lifecycle commands refuse with `move_pending`. Recovery covers process interruption,
  not power loss; the journal and carried park are host-local. Update every host's
  binary before moving synced records: older binaries cannot parse the new fields.
  For counterpart findings or manual undo, see the
  [recovery limits](../../docs/specs/2026-10-08-task-move-design.md#44-recovery-limits).
- A screenshot or file for a task: `tasks attach <id> <path>` (or `-` with `--name`, or
  `--clipboard`), with `--caption "<what it shows>"`. `tasks show <id>` prints each
  file's absolute path; read the image from there. `tasks detach <id> <name> "<why>"`
  removes one. Never copy files into `tasks/files/` by hand: the record's notes are the
  ledger, and `check` fails on a file no ledger lists. `feedback` takes no attachments.
- Decomposing: `tasks add "<piece>" --parent <goal>` for each part; `tasks dep` only
  for ordering between the pieces. `tasks dep <id> --on <new> --rm <old>` swaps a
  dependency in one save; any failure in the batch changes nothing. A goal that is committed work is a `todo` with a
  body, however large; `idea` is for uncommitted thoughts. `done` refuses while any
  descendant is open (`--force` overrides); `drop` refuses while any descendant is
  open and has no override — drop or reparent the subtree first
  (`tasks drop <child> "<why>"` / `tasks edit <child> --no-parent`).
- An effort meant to run beside the project's other efforts:
  `tasks add "<effort>" --lane -p <0-4> -b "<why it exists and its first milestone>"`,
  then decompose it with `--parent` like any goal. Lead the body with that paragraph:
  `prime` and `tasks lanes` show it as the lane's guidance, skipping headings above it.
  Make a lane only for a standalone effort; a sub-effort inside a lane is an ordinary
  child goal, and a write that nests a lane inside a lane is refused (`nested_lane`).
  A lane's priority ranks it in the lanes view and decides which lane wins a contested
  exclusive need; it never reorders `ready` or `next`. Pause a lane with
  `tasks block <lane> "<why>"`: its subtree leaves `ready`, `next`, and `prime`'s ready
  list until `tasks unblock <lane>`. `ready` and `next` warn about the paused work;
  `prime` shows the lane as `paused` in its lanes block. `start` on a task inside a paused
  lane still works. Never pause with `shelve`, which hides the lane from every view and
  needs its subtree shelved first. `check` warns about a lane with no steps yet.
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
- Id collision after a merge (git add/add conflict on the same `tasks/<id>.md`): keep one
  file, rename the other to a fresh id, fix its `id` field, then run `tasks check` and
  repair any `depends` it reports. If `tasks/files/<id>/` exists, split it by ledger
  before `git add` resolves the conflict:
  1. For each name live only in the loser's notes, move `tasks/files/<id>/<name>` to
     `tasks/files/<new>/<name>`.
  2. For a name both records attach, the record at `git show :2:tasks/<id>.md` came from
     the `HEAD` side, and the one at `:3:` from the merged side. Write each side's
     `git show :<stage>:tasks/files/<id>/<name>` into its own record's directory.
  3. Before staging, confirm `git hash-object <file>` equals
     `git rev-parse :<stage>:tasks/files/<id>/<name>`.
  4. Identical bytes (stage 0 only) are copied to both directories. If the merge is
     already committed, use `<merge>^1:` and `<merge>^2:` in place of `:2:` and `:3:`.

  `check` catches a file left under the wrong id, but not swapped bytes under a shared
  name.
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
A live id and its retired spelling are one task: `dep --on` of a dependency already
stored under either spelling changes nothing and warns, and `dep --rm` with either
spelling removes it. Never repoint a stored reference with `--on` then `--rm`; in two
calls that drops the dependency, and in one call it is refused.

Tools map ids and paths through renames and moves with
`tasks resolve <id|prefix|path>...` (or `--stdin`), never by parsing `projects.toml`
by hand. Before moving a checkout, run `tasks init --prefix <p> --force` there
on every host while the old root and storage still exist, so storage is recorded.

Start with clean `tasks/`, no live claims, and at most one git worktree. A pending rename
freezes every writer to that project, including `start`, feedback, `init --force`, and
`unregister`; reads remain available. `tasks rename <old> <new> --explain` reports the
observed recovery verdict without writes, locks, or authorization checks. Re-run the same
command without `--explain` to resume; live claims and extra worktrees still block recovery.

When another host has already renamed a synced checkout, run
`tasks rename <old> <new> --adopt` from the renamed checkout before starting work there.
It changes this host's registry and claim state, not the synced task files, and old ids
continue to resolve. A live claim in either store blocks adoption until its owner stops
or it becomes stale. Re-run after interruption; `--explain --adopt` diagnoses without
writes. Rehearsals must set both `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to scratch
directories; isolating only the registry can move live parks.

Recovery covers process interruption, not power loss. Outside git, once a source was
removed, only forward recovery is available. `git checkout .` alone is not an undo: new
filenames and the registry survive it. Follow the
[manual rollback procedure](../../docs/specs/2026-09-08-prefix-rename-design.md#56-undo-and-manual-recovery)
when rollback is needed: restore and verify each source before deleting its destination,
keep the destination and stop if restoration fails, restore config and registry (including
all moved aliases), and remove the inventory last.

## Retiring a project

A project that is over declares it: settle every open task (close or drop it, or move it
to the successor with `tasks move <id> --to <successor>`; re-file with
`--source <old id>` and drop the source when the task should change), then
`tasks retire [--successor <prefix>] --reason "<one line>"`
in its registered checkout. Every write to it is refused afterwards with `retired_project`,
from every checkout; its ids keep resolving, `--all-projects` and `--group` reads leave it
out, and `--project` reads warn. `tasks unretire` there undoes it. The project lifecycle
runbook in ops has the whole procedure.

## Document links and reads

`--spec` and `--plan` accept local paths/bare names or a host-configured external
reference `<scheme>:<opaque remainder>`. Successful changed-link writes store the
provider's canonical reference. Unrelated edits preserve existing external links
without fetching; changing the plan or step requires step verification.
Use `tasks doc <id> --plan` or `--spec` to read the selected body through the same
resolver, including linked projects and worktree/main-checkout fallback.
Use `tasks list --plan <stored reference> --status done` to find completed steps
across parents; list/search compare the stored value exactly and never fetch.

`show` and `next` warn and omit `step_found` for an unavailable open external
plan; absence means unknown, not a missing heading. No step means no external
fetch. A read that must never run a provider command passes `--no-fetch` to
`show`, `next` or `doc`: `step_found` is then omitted for any external plan, and
`doc` fails an external link as `doc_not_found`; local documents still read. `check` prioritizes stepped open plans within a shared five-second budget
and reports unfinished references once as `doc_unchecked`. Local missing files
and steps remain errors; malformed host configuration remains fatal. See the
[README's external document contract](../../README.md#external-documents) for
fetch configuration, limits and binary-before-config rollout order.

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

## Feedback

A report about a project's tooling goes to the project that owns it, from any checkout or
from outside every project, in one command:

    tasks feedback --project <owner> "<one line about the tool>" --category <friction|gap|idea|positive> [-b "<command, error kind, what you expected>"]

File it at the moment it happens: the tool got in the way (`friction`), could not do
something you needed (`gap`), gave you an idea (`idea`), or worked notably well
(`positive`). `--project` is required and names a registered project whose
`tasks/.config.toml` has a `[feedback]` table; any other is refused with the list of
projects that accept, each with its scope. Your agent instructions may say when to
report and list the owners; the refusal lists them either way. The report lands as an
`idea` in the owner's registered checkout (committed there when the owner sets
`[git] commit`), tagged `feedback`, the category,
and `from:<your prefix>` (`from:unregistered` outside every project; a linked checkout
reports its project's prefix). Describe the tool, not the project: no repository names, file
paths, people, or project content. Do not
manually commit in the owner's repository and do not triage your own report. If `ambiguous` comes
back, rerun with `--recur <id>` to join the listed entry or `--new` to insist. `--body`
detail is kept verbatim on a new entry; recurrence appends it as a single-line note, so
on recurrence it must be one line. Keep the
returned id in a note if the outcome matters to your task; `tasks show <id>` works from
any registered project.

### In a project that accepts feedback

A `[feedback]` table with a one-line `scope` opts the project in:

    [feedback]
    scope = "<what this project owns, for someone deciding where a report goes>"

Ideas tagged `feedback` with no triage note are unreviewed reports: read each, and redact
anything that describes a project rather than the tool with `tasks edit` (history keeps
the original). Ideas tagged `feedback` are the triage queue; scope, drop, or promote them like any other idea and
record the outcome in a note. Drop a misfiled report with a note naming the owner it
belongs to. `check` holds `feedback`, the categories, and any `from:<prefix>` or
`from:unregistered` as defined
on feedback records, whatever the tag dictionary says.
