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
Starting a goal with local descendants in `todo`, `blocked`, or `doing` also
suggests `tasks ready --under <goal-id>` to find eligible work. The claim is on
the goal you started.
`park` records an actionable continuation and releases the active claim without
changing status. Read [ownership](ownership.md) before takeover, release or
automation. `block` records an explicit obstacle; dependencies alone do not
change a task's status. `drop` closes abandoned work. Reopen a done or dropped
nonrecurring task with `edit --status todo` before starting it again; a recurring
done task can be started directly for its next occurrence. A moved record is a
redirect to its successor, not editable work.

For a goal, `prime` offers closeout when its direct dependencies and effective
members are closed. Both the offer and `done` follow children and foreign pieces
through done subgoals and stop at dropped ones. If a foreign piece is offline,
the warning `G has unresolved goal members: H -> B` names the goal G and
the declaration to check. Restore access or deliberately waive it with
`done --force`. A membership cycle warning gives its path, for example
`G has a membership cycle: X -> Y -> X`. The result names
the waived work. Cycles and read failures still require repair. The writer
rechecks even after an offer. See [goal completion](task-model.md#goal-completion).
`drop` refuses open descendants and has no force override. A `halt` tag on open
work prevents lower-priority starts in that project without an explicit override;
`prime` reports halts. See the [task model](task-model.md#lifecycle).
Session integrations can use the [hook subscribers](hooks.md) to report held claims
and registered halts. They are read-only protocol commands; configuring a route
requires its own qualification and activation.

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
`show` lists the tasks that depend on the shown one as `dependents` (pretty
`dependent` rows), open and closed. Plain `show` finds those in its own project;
`show --all-projects` also searches every reachable registered project, such as a
hub goal elsewhere that depends on a cross-project piece. `next` uses its own scope.
`goals` reports user or mixed-origin goals and their members across registered
projects (`--project` and `--group` narrow which goals are listed; `--all-projects`
is the default; done goals are left out unless `--closed` adds those closed in the
last 14 days). A goal's members are its descendants plus its declared `pieces` and
their members, in any project ([task model](task-model.md#goal-pieces)); they are
read through the registry whatever the scope, so narrowing never changes a row's
numbers. Dependency counts are reported separately and do not redefine the goal's
intended scope. Declare a foreign task part of a goal with `tasks edit <goal> --piece
<id>` (or `add --piece`), which also adds the dependency; `--rm-piece` unmarks one and
keeps the dependency, `--no-pieces` unmarks all, and `dep --rm` removes both.

The default `goals --sort project` groups canonical project prefixes A–Z, then
shows newest activity (the newest update on the goal or any member).
`--sort activity` instead ranks all goals by activity; `--sort priority` ranks P0
first, then activity. Open ties use work total descending and id ascending.
`--reverse` reverses every key, including ties, within each open/closed section;
it never moves closed goals ahead of open ones. Under `--closed`, closed rows use
closure dates instead of activity and tie on id, without a descendant-total key.
Sorting applies to JSON array order too; no JSON fields or counts change.

Pretty progress uses separate aligned columns: `work` is done/total committed
descendants and declared pieces, `+N` counts ideas, `deps` is satisfied/total distinct
prerequisites outside the membership, and `?N` counts unresolved prerequisites,
including declared pieces no record answers (`unres` with headers).
Work includes done, todo, doing and blocked members, excluding the goal itself,
ideas, shelved, dropped and moved tasks. Dependency satisfied count is total minus
open minus unresolved; unknown targets never count as satisfied. Fully satisfied
sets remain visible as N/N, and rows without prerequisites show a dash. JSON keeps
its existing open/total/unresolved fields. Optional counter columns appear only
when a category exists anywhere in the displayed open and closed report.

The selected sort and counter legend appear before data, including with headers
off, and a footer notes that parks and claims are host-local evidence from the
running host. Long titles end in `…` to fit the available space, capped at 80
display cells. Attention (`stalled` in yellow, `blocked` in red, `you` in bold
red), then secondary counters, then core metadata move into labelled row
details as space tightens; their legend clauses remain present. `COLUMNS` overrides
the attached terminal width and must be a positive integer. Pipes with no known
width keep one row per goal with the title cap; JSON and `tasks show <id>` preserve
full titles. Project colors and `--color always` do not change width budgets.

Pretty `prime` (across `closeout`, `lanes`, `roadmap`, `ready` and `doing`) and
`tree` also reserve a left-aligned `work` column for goals of every origin,
including childless lanes, childless declared-piece hubs and closed ancestors of
open work; leaf rows leave `work` blank, and `list`, `ready`, `search`, `parked`
and standalone `lanes` keep their existing columns. Each goal cell uses the same
full membership before display filtering (hidden completed children and foreign
declared-piece subtrees still count, and selecting `tree <id>` keeps the goal's
full counts while starting at depth zero): `4/7`, `4/7 +3`, or `-` / `- +3` when
committed work is zero, plus `?N` (`- ?1`, `- +3 ?1`) for distinct unresolved
declared-piece targets encountered during the membership walk. Unlike `goals`,
`prime` and `tree` do not walk ordinary `depends` for `work`, so `?N` reflects
missing declared pieces only; `doing` goals read from another claimed checkout use
that checkout's scan. Tree containment stays local to each project (foreign pieces
contribute to `work` without becoming local child nodes), drawn with three-cell
guides (`├─ `, `└─ `, `│  `, `   `) decided after display filtering and blank ID
cells on wrapped continuations. When any displayed goal is present, a wrapping
legend (`work: done/total local descendants and declared pieces`, with
`; tree: local containment` in `prime` and `; +N: ideas` and
`; ?N: unresolved declared pieces` when displayed) appears after `prime`'s top
summary block or before `tree`'s table;
empty trees stay empty and leaves-only views omit the legend. Work metadata is
computed only for resolved pretty output (`--pretty` or `TASKS_FORMAT=pretty`);
JSON shapes and `prime --summary` are unchanged.

Pretty task tables (`list`, `ready`, `tree`, `prime`'s rows) mark each row's type
in one column using `≡` for a lane, `◆` for any other goal (a record with children
or declared pieces), and `p` for a recurring record. `tree` places this column at
the far left and leaves its heading blank. The column appears only when some row
needs it.


`related` expresses non-blocking informational links and may name foreign task
ids. It does not affect readiness, review, lanes or blocker ranking. Pretty
`show` and `next` list one row per canonical task ID under `links`. Retired
prefix spellings group under the canonical ID even when the target is unreachable.
A move tombstone and its successor keep separate rows.

Each row shows its roles, ID, status, updated date, and full title. Roles and
rows follow this order, with the highest-precedence role setting the row position
and ties keeping the order within that role. The order is `parent`, `piece`,
`depends on`, `dependent`, `wake on`, `follows`, `followed by`, `child`,
`source`, and `related`. A piece covers its dependency role. A row shows
`related` only when it has no other role. Long role lists and titles wrap under
their own columns.

Resolved rows use the target's live status and recency-colored `updated` date
(`YYYY-MM-DD`). An unresolved row shows `?` for status and `(unresolved)` for
its title, with a blank date. JSON keeps separate relationship fields and their
existing ID spellings.

`follows` means: this task continues work arising from
one or more earlier tasks. `source` remains its filing origin, and `related` an
informational citation. Follows does not require an earlier task to close or change
readiness, review or completion. Existing source/related links are not converted.
Show and next display outgoing follows links to foreign, moved and unresolved
targets. They also display direct incoming `followed by` links, including closed follow-ups.
Plain show searches its local scan, and `show --all-projects` every reachable
registered project; next uses its existing group/all-project scope.
No incoming row means none found in that readable scope. After a move, the view
matches the focal work’s verified move history; old/new ids agree only over the
same complete scan and history. Incomplete history warns once and keeps direct
matches. A moved follower appears only through its in-scope live successor.
After a cross-project move, use `show --all-projects` so followers in the old
and new projects are both in scope.

Set it explicitly with repeatable `add --follows REF` or `edit --follows REF`.
Remove one with `edit ID --rm-follows REF`, or all with `edit ID --no-follows`.
The same target cannot be added and removed in one write; clear conflicts with both.
New self, missing or moved links refuse atomically (use a moved task's live successor).
Editor changes follow the same validation. An unchanged unreachable list survives
unrelated saves. Source/title reuse of add writes nothing; edit the returned task
if it needs follows. Body/source/concerns text never infers follows.

A new task filed from a brief can keep `--source docs/notes/brief.md` and name
multiple predecessors with `--follows`. A normal plan constituent uses `--parent`,
not follows merely because it came from a design. Existing source/related values
are not retyped or backfilled.

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
quietly; a re-review (`--reviewed`) clears `awaiting_user` unless `--awaiting-user`
is given again. Once waits clear, new closure/deferral evidence can wake them; an idea
with no supported wait or wake evidence is stranded. Unassessed `todo`/`blocked`
leaves need review when process is absent, except lanes. Use the scope skill for
the review workflow rather than treating the mark as an execution decision.

Plain notes preserve the review marker and its timestamp. `note` reminds you
when the idea explicitly awaits the user. Use `note --no-reviewed` to record
new evidence or an answer and return the idea to the review queue.

`shelve <id> "<wake condition>" --defer 30d [--wake-on <id>]...` sets aside open
work with a required future review checkpoint and optional wake targets. Shelves
remain open and visible in ordinary list/search, while execution pickers and roadmap omit
them. A shelved dependency still blocks open dependents and triggers `check`'s
`shelved_dep` warning: replace the shelf with a dependency on the awaited task when
one exists, unshelve or drop the edge when the wait is no longer intended, or retain
both the shelf and the warning when the recorded wake condition is an intentional
wait no task can stand for (a note does not suppress the warning). Any currently closed wake target or due date wakes review; missing targets
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

`blockers` answers which open prerequisites to tackle for the most downstream
impact (default limit 20). Its rows are root blockers: a blocker that waits on another
ranked row counts through it, since every task waiting on it also waits on that row.
JSON lists such rows after the roots with `counted_through`; pretty output leaves them
out and counts them in its footer. A cycle member is never folded. Rows rank by
committed work waiting at any depth (`todo`, `doing`, `blocked`; ideas are counted
apart), then the committed work it alone still holds (`clears`), the highest priority
waiting, its own priority, creation and id. It follows dependencies inside scope;
foreign boundary edges are reported without expanding another project implicitly.
Removing a dependency obstruction does not prove the dependent is otherwise ready.

Pretty output is one labelled line per root, whatever `--headers` says: `state` names
what stands between the row and someone finishing it, most pressing first (`cycle`,
`user`, `shelved`, `idea`, `deferred`, `waits` on its own open or unresolved
dependencies, `blocked`, `claimed`, `parked`, then `doing` or `todo`). Titles clip to a
known terminal width and stay whole when piped. The footer counts the roots and folded
rows, explains the columns and states the scope and coverage; warnings it restates
(history, a cut scope) stay in JSON and out of the pretty warnings section. Park step
text and claims are in `show`.

Read `coverage.complete` and warnings before trusting ranks or counts. Partial
scans yield lower bounds and possibly different ordering; unresolved edges never
mean completion. User attention reflects recorded user waits or `awaiting_user`,
not an inferred reason for an old task. Park evidence is host-local, and reported
age is not a measurement of how long a person has been blocked.

## Route across projects

The default scope is the current project, including a recognized worktree or
linked checkout (`quiet` and `goals` instead read all reachable registered
projects unless `--project` or `--group` narrows them). `--project`, `--group`
and `--all-projects` are mutually exclusive explicit scopes. An explicit scope wins over an id's prefix for scoped reads;
otherwise a foreign full id can route through the registry. Id-directed writes
lock and mutate the target project. A malformed local config is an error, not a
reason to retry against some other root. Foreign reads normally see the registered
records home, which can differ from an unmerged in-tree or overlay worktree.
Publish an upstream record and its completion there before relying on a foreign
dependency; detached code worktrees already share records. See
[record publication](project-operations.md#publish-records-for-cross-project-dependencies)
for safe Git integration and the separate plan-review gate.

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
records a tool observation in any registered, reachable, unretired project. Existing matching feedback may recur; use `--new`
for a distinct issue and `-b` for context. Source is opaque evidence, while
`--origin user|agent|mixed` records whose ask it is. Unknown origin stays absent;
the CLI does not infer it from author, parent or session. Correct it explicitly
with a note citing evidence. [Task model](task-model.md#origin-and-provenance)
defines field versus occurrence precedence and provenance limits.

Creation can stamp `--agent`/`TASKS_AGENT`; completion can stamp `TASKS_MODEL`.
Inside a Codex tool call (`CODEX_THREAD_ID` set and nonempty, with no explicit
`--agent`), `tasks` asks `relay model --harness codex --json` once per process
(2 s deadline, 64 KiB stdout cap, stderr discarded) instead of trusting
inherited `TASKS_AGENT`/`TASKS_MODEL`: a verified answer records
`codex/<model>[@<effort>]` on creation and `<model>` on completion;
`not-under-codex` naming another harness `H` reads `TASKS_AGENT`/`TASKS_MODEL`
when `TASKS_AGENT`'s harness prefix matches `H`, and otherwise records bare `H`
with warning `model unverified: other-harness`; any other outcome records bare
`codex` and no completion `model`, warning `model unverified: no-harness`,
`model unverified: <reason>` (including `unbound-call` and `earlier-turn`),
`model resolver timeout`, `model resolver missing`, or `model resolver failed`
without failing the write. Known limits of the resolver route: user TUI shell
escapes take the latest turn's model; a shell kept from an earlier turn stamps
bare `codex` (`earlier-turn`); other harnesses launched under Codex require a
matching `TASKS_AGENT`; and sandbox reparenting resolves through call binding
(`unbound-call`/`earlier-turn` when unbound).
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
In multi-task views, colored titles with more recent direct or neighboring activity
move through a shared lightness range. On the builtin dark palette, lightness is
`0.60 + 0.30a` for activity `a` from 0 to 1. Higher activity normally makes titles
lighter on dark backgrounds and darker on light backgrounds, keeping at least
3.5:1 contrast. The project hue is preserved, with reduced chroma where needed;
unusual backgrounds may limit the range. IDs keep their fixed project shade,
independently of activity, and active titles can be lighter than IDs. In `prime`
and `tree` pretty views, goal IDs add bold emphasis (retaining the fixed project ID
shade when assigned, or bold alone without a project color; standalone `lanes` and
nested lane pick/active references stay non-bold), and tree guide segments step in
OKLab by column depth (capped at depth 6) from a muted project shade (or muted
`fg`/`bg` mix when unassigned) toward readable foreground with at least 3:1
contrast against `bg` (dim chrome without a resolved palette; plain connectors when
color is disabled).
Activity affects presentation only; priorities and work selection keep their order.
