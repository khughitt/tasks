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
Series history lives where the rest of a task's history lives: in its notes, which `done`
already appends to each cycle.

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
missing or unknown unit, and any `n` above 36500. The cap is stated rather than left to
the arithmetic so that an absurd interval fails with a number a reader can act on instead
of a platform-dependent overflow, and it makes `last_done + every` unconditionally
representable.

A malformed `every` therefore fails in `parse_task`, which means a hand-edited record
surfaces through the lenient scan as an unparsable file rather than through a `check`
finding. `check` covers only what parsing cannot see (§6).

### 3.2 The anchor

`last_done` is stamped by the completion transition and by nothing else. Three rules make
the edges explicit rather than defensive:

- **Changing the cadence preserves the anchor.** `edit --every 14d` on a task last closed
  20 days ago applies the new interval from that close, which makes the task immediately
  due. This is the useful reading: shortening a cadence should surface the work, not
  restart the clock.
- **`--every` on an already-closed task does not stamp an anchor.** A closed record with a
  cadence and no anchor is due *now* (§4.1). "I want this to recur and I do not know when
  it last happened" should surface the task; its first `done` anchors the cycle.
- **`--no-every` clears both fields.** Clearing a cadence removes the anchor with it, so
  `last_done` without `every` is an invariant violation rather than harmless residue
  (§6). The durable trail of past completions is the notes, which are untouched.

## 4. Dueness

### 4.1 The predicate

```
due(task)    = task.last_done + task.every            // None when either is absent
is_due(task, now) = task.every.is_some()
                 && task.status == Done
                 && (task.last_done.is_none() || now >= due(task))
```

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
open-dependency check: `to == Done && task.status != Done`. That is exactly the right
gate. Every path that completes a task anchors the cycle -- `done`, `edit --status done`,
and an editor save that changes the status -- while re-saving a record that was already
`done` leaves the anchor alone.

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

Flag interactions: `--periodic` conflicts with `--parked` and with an explicit `--sort`,
both of which impose an incompatible ordering. Ordinary filters intersect with it as
usual: `--tag`, `--owner`, `--source`, `--parent`, `--project`, and an explicit `--status`
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

Interval and timestamp shape are enforced at parse (§3.1), so `check` covers only what
parsing cannot see:

| Finding                 | Level   | Condition                                    |
|-------------------------|---------|----------------------------------------------|
| `anchor_without_cadence`| error   | `last_done` set with no `every`               |
| `periodic_goal`         | error   | `every` on a task that has children           |

`open_child_of_closed_parent` (`src/commands/check.rs`) is **exempted** for a task
carrying `every`. A periodic child reopened under a goal that has since closed is
intentional, and the goal stays closed; warning on it would be noise on exactly the
records this feature exists to create.

Write-time refusals (§4.7) are validation errors naming both ids.

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
- early reopening: `start` on a not-yet-due recurrence succeeds, and the following `done`
  re-anchors from that close;
- `--no-every` clears both fields, and a subsequent `check` is clean;
- both hierarchy refusals: `--every` on a task with children, and parenting a task under
  one that carries `every`, each via `edit` and via the editor save;
- `check` reports `anchor_without_cadence` and `periodic_goal` on hand-written records,
  and does not warn `open_child_of_closed_parent` for a periodic child;
- a due record satisfies a dependent, and holds it once started;
- a due record is omitted from `ready` when another session claims it, and when it is
  parked waiting on the user, with the existing warnings;
- `list --periodic` ordering across all three groups, its bypass of the default status
  filter, its intersection with `--tag`, and its conflicts with `--parked` and `--sort`;
- frontmatter round-trip with both fields present, absent, and one without the other, and
  the nested JSON shape in `list`, `show`, and `next`.

Unit tests keep the arithmetic: interval parsing and every rejection in §3.1, the cap and
representability, and `is_due` at the boundary -- one second before, exactly at, and one
second after `last_done + every`.

## 9. Out of scope

- Calendar cadences of any kind (§2.4). If interval anchoring proves wrong in practice
  that is a new decision, not a flag.
- Per-occurrence records, templates, and series history beyond notes (§2.1).
- Lifecycle hooks (tasks-d40e8e), now independent of this design.
- Any notification, reminder, or process that runs without the user invoking a command.
- Recurrence on a goal (§4.7).
