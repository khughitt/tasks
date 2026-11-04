# Using tasks

Type: guide; document types 1.
This guide covers choosing and recording work, with the [task model](task-model.md), [configuration](configuration.md)
and [ownership protocol](ownership.md) for the detailed contracts.

Tasks stores work as Markdown records and prints JSON by default. Start with the
[README](../README.md) for installation; use `tasks --help` and
`tasks <command> --help` for the complete command and flag catalogue.

## Record work and choose a next action

Run `tasks init --prefix demo` in a project, then:

```sh
tasks add "Investigate the failing import" --status idea --origin user
tasks list
tasks ready
tasks next
tasks prime
```

Use the id returned by `add` in later commands. A prefix contains two to eight
lowercase letters/digits and starts with a letter; its suffix is six lowercase
hex digits. Positional ids accept trailing periods/commas copied from prose.
Stored ids and references are strict. Full ids can route to registered projects;
suffixes and unique title/id shorthand use the command's scope.

Scope an idea before committing to work: give it a concrete outcome, priority
(`0` highest through `4`), size (`xs` through `xl`), complexity (`low`, `mid`,
`high`) and process (`direct` or `planned`). These assessments are independent;
missing values mean unassessed. `add` accepts initial `idea` or `todo` status;
use lifecycle commands for later transitions. `edit` changes fields or opens an
editor when no field arguments are supplied. Prefer the CLI to editing records.

```sh
tasks edit demo-abcdef --status todo --size s --complexity low --process direct
tasks start demo-abcdef
tasks note demo-abcdef "Reproduced with an empty input"
tasks done demo-abcdef "Verified the corrected behavior"
```

`start` establishes a host-local work claim and records an advisory owner.
`park` records an actionable continuation and releases the active claim without
changing status. Read [ownership](ownership.md) before takeover, release or
automation. `block` records an explicit obstacle; dependencies alone do not
change a task's status. `drop` closes abandoned work. Reopen a done or dropped
nonrecurring task with `edit --status todo` before starting it again; a recurring
done task can be started directly for its next occurrence. A moved record is a
redirect to its successor, not editable work.

`done` refuses open dependencies or descendants unless explicitly forced.
`drop` refuses open descendants and has no force override. A `halt` tag on open
work prevents lower-priority starts in that project without an explicit override;
`prime` reports halts. See the [task model](task-model.md#lifecycle).

## Select work without losing context

`ready` selects due `todo` leaves and due completed recurrences whose dependencies
are satisfied, excluding
held work, busy lanes and unavailable exclusive resources. A closed dependency
is satisfied; an unresolved dependency is a warning and cannot establish
readiness. `next` first resumes the most recent eligible agent-waiting park, then
falls back to ready order, under the applicable picker gates. A parked idea can
be returned to resume scoping; it is not scoped implementation work. With no
eligible choice the result is `next: null`. Priority, size and creation/id tie
breakers keep ordinary ready selection deterministic.
`prime` combines actionable work, claims, parks, review state, lanes and roadmap.

Repeated values of one filter widen selection; different filters narrow it.
Repeated `--tag` and `--need` require all named values. An explicit status filter
replaces a read command's default pool. `none` selects an absent size, complexity,
process or origin. `--under` selects a parent's descendants. Literal source,
plan and reference strings do not change project routing.

`TASKS_MAX_COMPLEXITY` hides higher-rated and unassessed tasks from `ready`,
`next` and `prime`; `--max-complexity` overrides a valid environment setting for
one supported call. `TASKS_WITHOUT` is a comma-separated list of needs the
session cannot meet. `--without` adds to it and must name a need declared in
scope; unknown environment names have no effect. Reports include hidden counts.
These are selection constraints, not permission to perform a host operation.

Use `list`, `search`, `show`, `tree`, `dep`, `tags` and `sample` to inspect and
organize records. `graph --format mermaid` or `dot` renders dependency edges in
work order. Read commands do not acquire claims or perform lifecycle transitions.

## Goals, lanes and dependencies

`parent` expresses same-project containment. A task with children is a goal;
it does not become ready itself. `depends` expresses execution order and may
cross projects. Neither relation implies the other, and cycles are refused.
`goals` reports user or mixed-origin goals and their descendants; dependency
counts are reported separately and do not redefine the goal's intended scope.

A lane is a goal marked `--lane`: its descendant tree is one stream of work.
Lanes cannot nest. A live claim in a lane makes sibling work in that lane busy.
`--parallel` marks work assessed safe alongside other parallel work; it does
not bypass dependencies, claims, lanes or resource requirements.
`lanes` explains the derived state and causes for each lane.

Needs come from the project's declared vocabulary. Exclusive needs share a
host-wide namespace: matching names in different projects refer to the same
resource. Claims reserve exclusive needs; advisory needs do not serialize work.
See [configuration](configuration.md#project-settings).

## Review, waits and recurring work

`reviewed` says an idea has been considered. Derived `needs_review` reasons are
`unreviewed`, `woken`, `stranded` and `unassessed`. Reviewed ideas with a recorded
user wait, future deferral, pending proposal, or open/unresolved dependency wait
quietly. Once waits clear, new closure/deferral evidence can wake them; an idea
with no supported wait or wake evidence is stranded. Unassessed `todo`/`blocked`
leaves need review when process is absent, except lanes. Use the scope skill for
the review workflow rather than treating the mark as an execution decision.

`shelve <id> "<wake condition>" --defer 30d [--wake-on <id>]...` sets aside open
work with a required future review checkpoint and optional wake targets. Shelves
remain visible in ordinary list/search, while execution pickers and roadmap omit
them. Any currently closed wake target or due date wakes review; missing targets
still warn. Without a valid checkpoint or with an unresolved target, a shelf is
stranded unless due/closed evidence already wakes it. Reads never unshelve it;
`unshelve` returns it to an unreviewed idea. Wake edges are independent of execution
dependencies. Shelve open descendants before shelving a goal. Renewing a shelf
replaces its whole checkpoint/target pair; edit cannot erase its required checkpoint.

`--defer YYYY-MM-DD` or a positive `Nd`/`Nw` interval postpones ordinary work until
the start of that day in UTC. New dates must be after today. Status changes clear
deferral. `--every Nd|Nw` defines recurring work, up to 36,500 days; goals cannot
recur. Outside a shelf, recurrence and defer cannot coexist. A shelved goal or
recurrence can carry a review checkpoint, which does not change execution cadence.
Completion leaves a recurring record `done` and anchors its next due boundary;
when due it appears in ready with that same id and status. Start before completing
the next occurrence; early runs are permitted. `last_done` survives reopening and
`due` is derived at read time, never an automatic status rewrite.

## Blockers and coverage

`blockers` ranks open tasks by transitive impact, direct impact, priority, creation
and id (default limit 20). It follows dependencies inside scope; foreign boundary
edges are reported without expanding another project implicitly. Removing a
dependency obstruction does not prove the dependent is otherwise ready.

Read `coverage.complete` and warnings before trusting ranks or counts. Partial
scans yield lower bounds and possibly different ordering; unresolved edges never
mean completion. User attention reflects recorded user waits or `awaiting_user`,
not an inferred reason for an old task. Park evidence is host-local, and reported
age is not a measurement of how long a person has been blocked.

## Route across projects

The default scope is the current project, including a recognized worktree or
linked checkout. `--project`, `--group` and `--all-projects` are mutually exclusive
explicit scopes. An explicit scope wins over an id's prefix for scoped reads;
otherwise a foreign full id can route through the registry. Id-directed writes
lock and mutate the target project. A malformed local config is an error, not a
reason to retry against some other root. Foreign reads normally see the registered
main checkout, which can differ from an unmerged worktree.

`projects --paths`, `where`, `root <full-id>` and `resolve` explain roots and paths.
`root` requires a real full task id, not a bare project prefix. An explicitly named
unreachable project fails; wide scans warn and skip missing projects but still
fail malformed configuration. [Project operations](project-operations.md) covers
records layouts, routing repairs, moves, renames and retirement.

## Output, errors and bounded summaries

JSON is the automation contract; `--pretty` is for readers. Unset optionals and
empty task collections may be omitted, while response containers and warnings
remain. Do not parse pretty tables. A successful command exits 0, application
errors exit 1 and command-line usage errors exit 2. Errors have typed codes;
inspect warnings even on success. See [task model](task-model.md#json-and-errors).

`prime --summary` returns complete halt/warning evidence, counts and bounded
rows in at most 12,288 UTF-8 bytes. Initial row caps are five each for ready,
doing, lanes and roadmap, and three each for parks and goal closeout. To fit,
whole rows are shed in roadmap, lanes, closeout, ready, parked, doing order.
Omission metadata includes counts and the first omitted item; rows are not
silently clipped. If required metadata alone exceeds the bound, the command
fails explicitly. For full detail execute the returned argv as an argument
array and capture it to persistent task evidence; an incomplete summary is
never evidence that no work remains.

## Feedback, stamps and presentation

`feedback --project <prefix> "<title>" --category friction|gap|idea|positive`
records a tool observation. Existing matching feedback may recur; use `--new`
for a distinct issue and `-b` for context. Source is opaque evidence, while
`--origin user|agent|mixed` records whose ask it is. Unknown origin stays absent;
the CLI does not infer it from author, parent or session. Correct it explicitly
with a note citing evidence. [Task model](task-model.md#origin-and-provenance)
defines field versus occurrence precedence and provenance limits.

Creation can stamp `--agent`/`TASKS_AGENT`; completion can stamp `TASKS_MODEL`.
`note --stamp` adds verified native harness provenance when available. A session
claim is not proof of native provenance. Failure to establish provenance is
reported rather than guessed.

For shell completion source the stub printed by `TASKS_COMPLETE=<shell> tasks`
in that shell's rc; zsh needs `compinit` first. Completion walks the live parser
and offers scope-aware task candidates; it is best effort and does not change
records. Unset the variable or use `0` to disable its activation.

Color is opt-in for pretty output and absent from JSON. Set `--color`,
`TASKS_COLOR`, `TASKS_THEME` and optional `TASKS_PALETTE` as described in
[configuration](configuration.md#environment-and-precedence). Pretty `show`
wraps at the terminal width up to 100 columns; piped output is unwrapped.
