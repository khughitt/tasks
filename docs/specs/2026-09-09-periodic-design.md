# Periodic tasks: a recurrence that is derived, not scheduled

Status: designed (2026-09-09)
Task: tasks-5caeae; first consumer the curate skill
(docs/specs/2026-09-08-task-curation-design.md)

## 1. Problem

Some work never closes for good. A doc and code curation sweep, a dependency review, a
stale-doc check: each is finished for now and due again in a month. The tracker has no way
to say that. Today the choice is to close the task and lose the cadence, or leave it open
forever and have it sit in `ready` as work that cannot be done yet. Neither is true, and
the second is worse: a permanently ready task teaches the reader to skim past `ready`.

What is missing is small. The interval is known, the last completion is known, and the due
date is the sum. Nothing needs to run on a schedule, because nothing needs to happen at
the moment a task falls due -- it only needs to be *seen* differently from that moment on.

## 2. Decision

Two frontmatter fields, `every` and `last_done`, and no new machinery. `done` on a task
carrying `every` is an ordinary close that additionally stamps the anchor. Dueness is
computed at read time from the anchor and the interval, exactly as `phase` is computed
from the spec and plan links: `ready` and `next` treat a due record as eligible work,
`list --periodic` shows the whole series with its dates, and `prime` carries a one-line
count of what is scheduled but not yet due.

Nothing rewrites a record on a timer, on a read, or at any other moment the user did not
ask for. There is no tick command, no daemon, and therefore no missed-tick problem: a
project nobody has opened in six months has a tree that is exactly as truthful as one
primed this morning.

### 2.1 One task, not one per occurrence

A recurrence is one record, reopened, not a template that mints an occurrence per cycle.
The alternative gives each sweep its own notes, its own spec link, and its own closed
record -- real value, at the cost of a second kind of thing in the tree, a growing pile of
closed tasks, and a definition of "the same task" that every view would have to learn.
Series history lives where the rest of a task's history lives: in its notes -- one
appended automatically at each completion (§4.4), so the record carries a dated line per
cycle rather than only the anchor, which every cycle overwrites.

### 2.2 The record, not the store

`park` put its state in the out-of-git claim store, and the reasoning there does not
transfer. A park entry is ephemeral machine state describing a session; it is per machine
by design and losing it costs an overlay. A cadence is durable intent that belongs to the
project, should arrive with a fresh clone, and should be reviewable in a diff. It goes in
frontmatter.

### 2.3 Independent of lifecycle hooks

tasks-d40e8e proposed recurrence as the first consumer of lifecycle hooks: on `done`, a
hook mints the next occurrence. Deriving dueness needs no hook, no event, and no
per-project configuration, so the fork the two tasks shared is resolved in favour of the
model and the two are now independent. Hooks remain worth having for other reasons.

### 2.4 Interval, not calendar

`every: 30d` means thirty days after the sweep actually closed, not the first of the
month. An interval anchored to completion is self-correcting: a sweep run late pushes the
next one out, so the tracker never presents work that is stale-by-schedule the moment it
is finished. A calendar cadence would need a rule for missed periods, and the cadences
this exists to serve ("about monthly") have no calendar significance. `90d` covers
quarterly.

## 3. The fields

| Field       | Value                                                        | Written by                    |
|-------------|--------------------------------------------------------------|-------------------------------|
| `every`     | an interval, `<n>d` or `<n>w`                                 | `add --every`, `edit --every` |
| `last_done` | RFC 3339 UTC; the close that anchors the current cycle        | the completion transition (§4.4) |

In `serialize_task` (`src/format.rs`), `every` sits with the shape fields after `size` and
`parallel`; `last_done` sits with the timestamps immediately after `updated`, as
`Value::Raw` like its neighbours. `quote_timestamps` hardcodes the timestamp keys and
gains `last_done` beside `created` and `updated`.

### 3.1 The interval grammar

`<n><unit>`, where `n` is a positive whole number and `unit` is `d` or `w`. `1d` is 24
hours; `1w` is `7d`. There are no other units: months and years have no fixed length, and
an interval anchored to a completion has no calendar to resolve them against.

Rejected with a validation error naming the value: zero, a negative or fractional `n`, a
missing or unknown unit, and any `n` above 36500.

The cap is a product limit -- an interval of a century is a typo, and failing with a
stated number beats failing with a platform quirk. It is deliberately *not* a
representability argument, which it cannot carry: an anchor near the end of the
representable range overflows with `1d`. Representability is enforced where it actually
lives, in the arithmetic. `due` adds with checked arithmetic (§4.1), and `validate_task`
rejects a record whose `last_done + every` does not fit, so no record that parses can
overflow later.

A malformed `every`, or an anchor and interval that cannot be added, therefore fails in
`parse_task`, which means a hand-edited record surfaces through the lenient scan as an
unparsable file rather than through a `check` finding. `check` covers only what parsing
cannot see (§6).

### 3.2 The anchor

`last_done` is stamped by the completion transition and by nothing else. Three rules make
the edges explicit rather than defensive:

- **Changing the cadence preserves the anchor.** `edit --every 14d` on a task last closed
  20 days ago applies the new interval from that close, which makes the task immediately
  due. This is the useful reading: shortening a cadence should surface the work, not
  restart the clock.
- **`--every` on an already-closed task does not stamp an anchor.** A closed record with a
  cadence and no anchor is due *now* (§4.1). "I want this to recur and I do not know when
  it last happened" should surface the task; its first completion -- `start`, then `done`
  (§4.4) -- anchors the cycle.
- **`--no-every` clears both fields.** Clearing a cadence removes the anchor with it, so
  `last_done` without `every` is an invariant violation rather than harmless residue
  (§6). The durable trail of past completions is the notes, which are untouched.
- **The anchor is not editable.** An editor save may change or clear `every` -- changing a
  cadence is ordinary work -- but one that sets or moves a non-empty `last_done` is
  refused by `check_invariants`, beside `id`, `created`, and the notes. The anchor is
  stamped by a completion and by nothing else (§4.4); *clearing* it stays allowed, because
  clearing the cadence has to clear it.

### 3.3 The flags

| Surface     | Shape                                                                  |
|-------------|------------------------------------------------------------------------|
| `FieldArgs` | `--every <interval>`, on `add` and `edit`                               |
| `EditArgs`  | `--no-every`, `conflicts_with = "every"`; clears `every` and `last_done` |
| `list`      | `--periodic`, `conflicts_with_all = ["sort", "reverse", "parked"]`      |

`--every` offers `7d`, `14d`, `30d`, `90d` as completion candidates from `complete.rs`,
which is best-effort by contract and never errors. The list is a convenience, not the
accepted set: any interval matching §3.1 is valid.

## 4. Dueness

### 4.1 The predicate

```
// None for an open or dropped record and for an unanchored one. Validation (§3.1)
// guarantees the addition cannot overflow on a record that parses.
due(task) = if task.status == Done { task.last_done + task.every } else { None }

is_due(task, now) = task.every.is_some()
                 && task.status == Done
                 && due(task).map_or(task.last_done.is_none(), |d| now >= d)
```

`due` carries the status gate itself so that eligibility, the `list --periodic` ordering,
the JSON field, and the pretty due date are one value rather than four rules that could
drift. §5.4's `null` for an open or dropped record is this `None`, not a separate
convention.

Three consequences, each stated because each is a thing a reader will otherwise assume the
other way:

- **An open periodic task is an ordinary task.** `every` does nothing at all until the
  record closes. There is no new status, and `todo` is never gated on a date.
- **`dropped` ends the recurrence.** Only `Done` yields dueness. `last_done` is preserved
  on a dropped record as history; `due` is `null` for it, as it is for every open record.
  To resurrect a dropped recurrence, `edit --status todo`.
- **`done` keeps meaning done.** The counts in `prime` stay honest, and a periodic child
  never holds its parent goal out of `closeout`, because a closed record is not an open
  descendant.

`now` is captured once per read command and threaded to every call site, so the two
filters in `ready_tasks` and `is_ready` cannot disagree across a tick that lands mid-scan.

### 4.2 Eligibility, in one place

Eligibility for `ready` is currently asserted in three places that must agree: the
dependency-map filter and the main loop in `ready_tasks` (`src/commands/list.rs`), and
`is_ready` (`src/query.rs`), which independently requires `Todo`. They collapse to one
predicate:

```rust
pub fn is_actionable(task: &Task, now: &OffsetDateTime) -> bool {
    task.status == Status::Todo || is_due(task, now)
}
```

`is_ready` takes `now` and calls it in place of its `status == Todo` clause; both filters
in `ready_tasks` call it directly. `is_ready` has one production caller, so the signature
change is contained.

Every other gate is untouched and applies to a due record exactly as to a `todo` one: open
dependencies hold it, children exclude it, another session's live claim omits it with the
takeover hint, and a park waiting on the user omits it with the parked warning.

### 4.3 Reopening

`can_transition` (`src/model.rs`) permits a closed record to reopen only to `todo`, so
`start` on a closed recurrence would fail. One task-aware allowance is added in
`commands::transition`, the choke point that `start`, `done`, `drop`, `edit --status` and
the editor save all funnel through:

> `Done -> Doing` is permitted when `every` is set.

`can_transition` stays a pure status-pair table; the exception lives beside the other
task-aware checks, so every path agrees without any of them special-casing.

The condition is `every.is_some()`, deliberately **not** `is_due`. `transition` stays
clock-free, and the division of labour is clean:

- **`every` governs reopenability.** A recurrence can always be run early; `tasks start`
  on a sweep that is not due yet simply works, and its `done` re-anchors the cycle from
  that close. Forcing `edit --status todo && start` would be friction with no safety
  value.
- **`is_due` governs visibility.** The clock lives in the read commands and nowhere else.

### 4.4 The completion transition

`last_done` is stamped in `transition`, under the condition it already computes for the
open-dependency check, narrowed by the cadence:

    to == Done && task.status != Done && task.every.is_some()

The `every.is_some()` guard keeps `last_done` off every ordinary task -- an ordinary
completion never acquires an anchor. The `status != Done` clause means re-saving a record
that is already closed leaves the anchor alone. Every path that *completes* a task
therefore anchors the cycle: `done`, `edit --status done`, and an editor save that changes
the status.

The same branch appends the occurrence note (§2.1), so the note and the anchor cannot
drift apart:

    - 2026-10-09T09:12:44Z (keith): completed; next due 2026-11-08

A `done` message, when given, is appended after it as today.

**Closing an occurrence requires reopening first.** `can_transition` treats `Done -> Done`
as an idempotent no-op, so without a rule `tasks done` on a due recurrence would silently
change nothing: no anchor, no note, still due. That is the silent fallback this codebase
forbids, so it is refused instead. Closing a record that is already `Done` and carries
`every` is a validation error naming the way forward:

    tasks-5caeae is already done and recurs every 30d; `tasks start tasks-5caeae` before
    closing the next occurrence

The refusal is narrow in two ways. An already-done record *without* `every` keeps its
idempotent no-op. And it fires only where a completion is actually attempted: `edit
--status done` and an editor save that leave the status unchanged never reach `transition`
at all (`src/commands/edit.rs` branches on `to == task.status` first), so they stay the
no-ops they are today. That is the right line -- `done` is a command to complete an
occurrence, while asserting a field value that already holds is the "merely editing" case
this section preserves, and refusing it would break `edit --status done --priority 1` on a
closed recurrence.

The refusal has one exception, and it follows from wording the rule around *change*
rather than around the command. When `save` writes the record but then fails to release
the claim, it tells the caller to run `tasks done <id>` again to finish the cleanup
(`src/commands/mod.rs`, the `Release` arm). That retry arrives at a record that is already
`done`. So the refusal fires only when the store holds no entry for the task; while an
entry is still there, `done` performs the release it was asked for, warns that the
occurrence was already recorded, and touches neither the anchor nor the notes.

The refusal sits beside the `Done -> Doing` allowance of §4.3, which is what makes the
reopen a single command. The cycle is `start`, work, `done`.

### 4.5 Dependencies

No special case. A due record's `depends` are re-checked each cycle exactly as a `todo`
record's are. In practice they closed cycles ago and the check is a no-op, but the
uniformity is worth more than the shortcut, and it gives the one interesting case a
correct answer for free: a due but still-`Done` recurrence **satisfies** its dependents,
because it is closed; once started it is open and holds them until it closes again. That
falls out of ordinary status semantics with no second mechanism.

### 4.6 Ordering

Due records sort by the existing `ready_order` -- priority, size, created, id. Overdueness
does not jump the queue. Priority is the field that expresses urgency, and a sweep three
days late is not more important than a P1.

### 4.7 Goals are never periodic

`is_ready` excludes any task with children, so `every` on a goal is configuration that can
never fire. Rather than leave a silent dead end, it is refused in both directions at write
time, on `add`, on `edit`, and on the editor save: `--every` is refused when the task
already has children, and setting or changing a `parent` is refused when the *prospective
parent* carries `every`.
`check` keeps a finding for the combination arriving by merge or hand edit (§6).

### 4.8 Parking a recurrence

`park` refuses any closed record, and a due recurrence is closed. It stays that way: no
allowance is added, and `parked::candidates` keeps its `is_open` filter unchanged.

Parking means "I set this down mid-work, and here is the next step". A closed recurrence
is not work in progress; it is schedule. Someone who needs to record that a sweep waits on
the user reopens it first -- `start` is legal on any recurrence and costs nothing (§4.3)
-- and parks the `doing` record, where every existing park rule applies with no new case.
The cost is one command; the alternative is a park entry whose task is closed, which the
park design's own state machine has no meaning for.

One consequence for testing (§8): a park entry sitting on a due record is not reachable
through any ordinary sequence, because `done` removes the entry. The park filter in
`ready_tasks` still covers due records, since it is generic, but this design creates no
workflow that reaches it.

## 5. Surfacing

### 5.1 `ready` and `next`

A due record appears among the ready tasks in `ready_order`. Its status column reads
`done`, which is honest -- the record *is* closed, and the cadence is what makes it
eligible. The cadence and the date it fell due are rendered after the title in the
emphasis style, `every 30d, due 2026-09-08`. The row's date column is *not* used for this:
`ListOut` carries one `DateColumn` for the whole list, and a row showing a due date under
an `updated` header would be a lie about the column. `ready` keeps `DateColumn::Updated`
for every row.

`next` returns a due record in the show shape like any other head-of-ready, after parked
candidates as today. `show --pretty` renders `every`, `last_done`, and the derived due
date for any record carrying a cadence.

### 5.2 `list --periodic`

Every record carrying `every`, at any status. `--periodic` bypasses the default
open-status filter, since most of a healthy series is closed at any moment. Ordering:

1. unanchored due-now records, whose due cell renders `now`;
2. anchored `Done` records by due date ascending, so overdue leads;
3. everything else -- open and dropped records, whose due cell renders `-`;

with id breaking ties within each group. The view needs a third `DateColumn::Due`.

Flag interactions (§3.3): `--periodic` conflicts with `--parked`, `--sort`, and
`--reverse`, matching what `--parked` already declares -- each imposes an ordering
incompatible with the three groups above. Ordinary filters intersect with it as usual: `--tag`, `--owner`, `--source`, `--parent`, `--project`, and an explicit `--status`
all narrow the set.

### 5.3 `prime`

One line under the counts, omitted entirely when nothing is scheduled:

```
periodic: 3 scheduled, next due 2026-10-09 (in 6d)
```

`scheduled` counts anchored `Done` records whose due date is in the future, and `next due`
is the earliest of those. Due records are excluded from the count because they are work,
not schedule; they are eligible for `ready`, though a dependency, a claim, or a park
waiting on the user may still keep any given one off that list.

### 5.4 JSON shapes

`TaskSummary` gains one nullable nested object, following the `claim` and `park` precedent
rather than scattering four flat fields across the summary:

```json
"periodic": {
  "every": "30d",
  "last_done": "2026-09-09T11:00:00Z",
  "due": "2026-10-09T11:00:00Z",
  "due_now": false
}
```

Present whenever `every` is set, `null` otherwise. `due` is `null` for an open or dropped
record and for an unanchored one; `due_now` is carried explicitly because `due: null`
cannot distinguish "not applicable" from "due now, no anchor". `show` and `next` carry the
same object.

`prime` output gains the aggregate its pretty line renders, so a consumer need not
recompute it:

```json
"periodic": { "scheduled": 3, "next_due": "2026-10-09T11:00:00Z" }
```

`scheduled` is `0` and `next_due` is `null` when nothing is scheduled.

## 6. Validation and errors

Interval shape, timestamp shape, and the `last_done` requires `every` invariant are all
enforced in `validate_task`, which `parse_task` calls -- so a record violating any of them
does not load at all. `check` reads through `scan_lenient`, which still parses each file,
so such a record reaches `check` as a `parse` finding naming the file, not as a finding of
its own. This is the treatment `step requires plan` already gets, and it is why there is
no `anchor_without_cadence` finding: a record that could carry one cannot be read.

That leaves `check` one condition of its own, the one parsing cannot see because it spans
two files:

| Finding         | Level | Condition                                     |
|-----------------|-------|-----------------------------------------------|
| `periodic_goal` | error | `every` on a task that has children            |

`open_child_of_closed_parent` (`src/commands/check.rs`) is **exempted** for a task
carrying `every`. A periodic child reopened under a goal that has since closed is
intentional, and the goal stays closed; warning on it would be noise on exactly the
records this feature exists to create.

Three refusals live outside `check`, each a validation error at the moment of the write:

- closing a record that is already `Done` and carries `every`, which names the reopen
  (§4.4);
- `--every` on a task with children, and reparenting under a task that carries `every`,
  each naming both ids (§4.7);
- `park` on a closed recurrence, which reuses the existing closed-status error (§4.8).

## 7. Documentation and protocol

Landing in the same change: this spec's status header, `README`, `AGENTS.md`, and
`skills/tasks/SKILL.md`. The skill needs the cadence flags, the fact that `done` on a
recurrence is an ordinary `done`, and `list --periodic` for the "what is coming up"
question.

## 8. Testing

Both dueness states are reachable end to end without an injectable clock, through the
unanchored rule of §3.2:

- **due**: `add`, `done` with no cadence so no anchor is stamped, then `edit --every 30d`;
  the task appears in `ready` and `next`;
- **not due**: `add --every 30d`, `done`, which anchors at that close; the task is absent
  from `ready` and present in `list --periodic`.

End-to-end coverage in `tests/cli.rs`:

- both flows above, including the `prime` line and its JSON aggregate;
- the full cycle: `start` on a due record, `done`, and the record leaves `ready` anchored
  at that close;
- early reopening: `start` on a not-yet-due recurrence succeeds, and the following `done`
  re-anchors from that close;
- **repeated `done` is refused, not silent** (§4.4): `done` on an already-done recurrence
  errors naming the reopen, and the anchor, the notes, and `updated` are unchanged; an
  already-done record *without* `every` still accepts `done` as the no-op it is today;
- the cleanup retry still works (§4.4): with the claim store left unwritable so that
  `done` warns that cleanup failed, a second `done` releases the claim, warns that the
  occurrence was already recorded, and leaves the anchor and the notes as they were;
- an editor save cannot set or move `last_done` (§3.2), can clear it together with
  `every`, and can change `every` on its own;
- the equal-status paths stay no-ops (§4.4): `edit --status done --priority 1` on a closed
  recurrence succeeds and changes only the priority, and an editor save that leaves the
  status `done` touches neither the anchor nor the notes;
- each completion appends the occurrence note: three cycles leave three notes and one
  anchor, and a `done` message is appended after the automatic note;
- an ordinary task never acquires `last_done`, through `done`, `edit --status done`, and
  the editor save;
- `--no-every` clears both fields, and a subsequent `check` is clean;
- both hierarchy refusals: `--every` on a task with children, and parenting a task under
  one that carries `every`, each via `edit` and via the editor save;
- `check` reports `periodic_goal` on a hand-written pair of records, and does not warn
  `open_child_of_closed_parent` for a periodic child;
- a hand-written record with `last_done` and no `every`, and one whose `last_done + every`
  overflows, each fail the scan and reach `check` as a `parse` finding (§6);
- a due record satisfies a dependent, and holds it once started;
- a due record is omitted from `ready` when another session holds a live claim on it, with
  the existing takeover warning -- reachable by starting it in a second worktree;
- **parking follows §4.8**: `park` on a due record is refused with the closed-status
  error, and `start` then `park --waiting-on user` succeeds and omits it from `ready` with
  the existing parked warning;
- `list --periodic` ordering across all three groups, its bypass of the default status
  filter, its intersection with `--tag`, and its conflicts with `--parked`, `--sort`, and
  `--reverse`;
- `--no-every` and `--every` together fail as a flag conflict;
- frontmatter round-trip with both fields present, absent, and one without the other, and
  the nested JSON shape in `list`, `show`, and `next`.

Unit tests keep the arithmetic: interval parsing and every rejection in §3.1, the cap,
checked addition against an anchor near the end of the representable range, and `is_due`
at the boundary -- one second before, exactly at, and one second after
`last_done + every`. `due` returns `None` for an anchored open record and an anchored
dropped one (§4.1), which is the case the JSON contract depends on.

## 9. Out of scope

- Calendar cadences of any kind (§2.4). If interval anchoring proves wrong in practice
  that is a new decision, not a flag.
- Per-occurrence records, templates, and series history beyond notes (§2.1).
- Lifecycle hooks (tasks-d40e8e), now independent of this design.
- Any notification, reminder, or process that runs without the user invoking a command.
- Recurrence on a goal (§4.7).
- Parking a closed recurrence, and any park entry that outlives a completion (§4.8).
