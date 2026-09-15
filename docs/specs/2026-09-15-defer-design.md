# Deferred tasks: a one-shot date that hides work until it arrives

Status: draft
Task: tasks-be6fcc; feedback tasks-f5ab4a; motivating records prism-49a068 and prism-8a8eac

## 1. Problem

"Revisit this in two months" has no home. The only date the tracker knows is `every`,
anchored to each completion (`2026-09-09-periodic-design.md`), so a fresh recurring task
is ready at once and only its second occurrence waits. Today the choices are a date in
prose, which nothing reads, or an idea left in the roadmap where every `prime` shows it
and every `sample` may draw it before its time. Two prism records carry
`revisit 2026-11-10` in their bodies for exactly this reason.

What is missing is one field and one rule: a date on an open record, and pickers that
leave the record alone until the date arrives.

## 2. Decision

A frontmatter field `defer` holding a calendar date. While the date is in the future the
record is *deferred*: `ready`, `next`, `quiet`, and `sample` skip it, and `prime` and
`list` show it with its date. From the date on, the deferral is spent: a `todo` is back in
`ready` and an `idea` is back in the roadmap, each marked as due so the reader knows why
it returned. Every status transition clears the field, because each one is the task
receiving the attention the deferral was postponing.

Dueness is derived at read time from the field and the clock, as periodic dueness is.
Nothing rewrites a record on a timer, and there is no tick.

### 2.1 A date, not a timestamp

The other dated fields are RFC 3339 timestamps because they record instants. A deferral
is an intention about a day: "revisit on the 10th", never "at 14:02:11Z". Storing the day
keeps the record readable and the comparison honest — the record is due from the start of
that day in UTC, which is the tracker's only clock. The hours of skew against a local
calendar are accepted and stated, not hidden behind a timezone field.

### 2.2 One shot, cleared by attention

`defer` is an overlay on attention, not a state. It is set by the user and cleared by any
status transition: `start`, `done`, `drop`, `shelve`, `unshelve`, and `edit --status`
that changes the status. Running a deferred task early is `tasks start`, which works and
spends the deferral; scoping a deferred idea early is `edit --status todo`, likewise.
There is no allowance to keep and no clock in `transition`: the clock stays in the read
commands, as it does for periodic dueness (periodic §4.3).

Edits that do not change status leave the field alone; `--no-defer` clears it on demand.

A new deferral and a status change cannot travel in one write, because the order in
which they apply would decide the outcome: `edit` applies fields before it transitions,
so `edit <idea> --status todo --defer 60d` would set the date, clear it, and hand back an
immediately ready task with no error. `--status` therefore conflicts with `--defer` on
`edit` (a clap conflict), and an editor save that changes the status must leave `defer`
exactly as the original had it — the transition then clears it; a save that changes both
is refused naming the two-save sequence. Scoping an idea into a deferred todo is two
commands: `edit --status todo`, then `edit --defer 60d`.

### 2.3 Attention, not blocking

A deferred record is open. Its dependents stay held exactly as they are held by any open
dependency, and it satisfies nothing until it closes. Deferral says when *this* task
wants attention; it says nothing about the work around it. `check` gains no finding for a
dependent of a deferred task: the date is visible on the record, which is more than a
shelved dependency offers.

### 2.4 Not a recurrence

A record cannot carry both `every` and `defer`. Recurrence already defers, from each
completion; a second date on the same record would need a rule for which wins, and the
motivating cases are all one-shot. Setting either when the other is present is a
validation error naming both.

## 3. The field

| Field   | Value                          | Written by                                          |
|---------|--------------------------------|-----------------------------------------------------|
| `defer` | a calendar date, `YYYY-MM-DD`  | `add --defer`, `edit --defer`, the editor save; cleared by `--no-defer` and by every status transition (§2.2) |

In `serialize_task` (`src/format.rs`) it sits with the shape fields after `every`; in
`KEYS` and `parse_task` likewise. `quote_timestamps` is untouched: it exists because the
frontmatter parser reserves `:` and an RFC 3339 stamp carries two, whereas a bare
`2026-11-10` has no reserved character and parses as the scalar it is.

### 3.1 The grammar of `--defer`

Two forms, both stored as the absolute date:

- `YYYY-MM-DD`, an absolute date;
- `<n>d` or `<n>w`, the periodic interval grammar (periodic §3.1), meaning today plus
  that many days, today being the UTC date at the moment of the write.

The flag refuses a date that is not after today: a deferral to today or the past can only
be a typo, and it would be visible at once as a due record rather than doing anything.
The refusal lives in the flag, not in `validate_task`: a record whose date has passed is
the due state (§4), and it must parse.

`--defer` offers `30d`, `60d`, `90d` as completion candidates from `complete.rs`,
best-effort as ever.

### 3.2 Invariants

Two invariants are status-free and belong in `validate_task`, so a violating record does
not load at all and reaches `check` as a `parse` finding (periodic §6):

- `defer` is a valid calendar date;
- `defer` and `every` are never both present (§2.4).

The third is about status: `defer` may sit on `idea`, `todo`, and `blocked`, and on
nothing else, since every path into the other statuses clears it (§2.2). It is
deliberately **not** in `validate_task`. The editor parses and validates the saved text
before it transitions (`src/commands/edit.rs`), so a save that moves a deferred todo to
`doing` and leaves the date alone — the ordinary case, and exactly what §2.2 promises to
clear — would fail validation before `transition` ever saw it. The rule is enforced
instead where a deferral is written: `--defer` refuses on a record whose status cannot
carry one, naming the status; an editor save that keeps the status and sets or keeps
`defer` on such a record is refused the same way; and a save that changes the status
reaches `transition`, which clears the field. A record arriving by merge or hand edit
with `defer` on a `doing`, `shelved`, or closed status parses and is reported by `check`
as `defer_status` (§6).

An editor save may otherwise set, change, or clear `defer` — it is ordinary intent, not
an anchor — subject to §2.2's one-thing-per-save rule.

### 3.3 Goals

`is_ready` excludes any task with children, so `defer` on a goal would hide nothing from
`ready`; it would only mark the roadmap row. That is a silent dead end, and it is refused
the way `every` is (periodic §4.7): `--defer` is refused when the task has children,
setting or changing `parent` is refused when the prospective parent carries `defer`, and
`check` reports `deferred_goal` (error) for the combination arriving by merge or hand
edit.

## 4. Dueness

```
// The date the deferral spends itself; None when the record carries no deferral.
defer_until(task) = task.defer

// Deferred means hidden: the date is still ahead of the UTC calendar day.
is_deferred(task, now) = task.defer.map_or(false, |d| now.date() < d)

// Due means the deferral has been spent by the clock but not yet by attention.
is_defer_due(task, now) = task.defer.map_or(false, |d| now.date() >= d)
```

`now` is the single value each read command already captures for periodic dueness, so
every predicate in one run agrees.

### 4.1 Eligibility

`is_actionable` (`src/query.rs`) gains the deferral gate:

```rust
pub fn is_actionable(task: &Task, now: OffsetDateTime) -> bool {
    (task.status == Status::Todo && !is_deferred(task, now)) || is_due(task, now)
}
```

A periodic record can never be deferred (§2.4), so the second clause needs no gate. Every
other `ready` rule applies to a due deferred record exactly as to any `todo`.

### 4.2 The pickers

One predicate, `is_deferred`, applied by everything that hands out work:

- **`ready`** omits deferred records through `is_actionable`, and reports the omission in
  one warning for the whole list rather than one per record, since a deferral is the
  user's own standing intent and not a transient state worth a line each:

      2 deferred tasks omitted, next due 2026-11-10; `tasks list --deferred` shows them

  The count is over records `ready` would otherwise have listed.
- **`next`** returns nothing deferred, in either of its sources: the head of `ready`,
  which already excludes them, and the parked-waiting-on-agent candidates, which are
  filtered by `is_deferred` too. A park and a deferral on one record is the user saying
  "resume" and "not yet" at once; the deferral wins because it is the later, dated
  instruction, and `prime` still lists the park so nothing is lost. `next` counts its
  omissions over its own pool — the parked candidates and the ready list, each record
  once — so a deferred parked idea, which `ready` never sees, is still named in the
  warning rather than producing a silent "nothing ready"; the warning has the same shape
  as `ready`'s.
- **`quiet`** filters its parked rows the same way. Its rows resolve to the parked
  worktree's copy of the record when there is one (`rows_preferring(Prefer::Recorded)`),
  and that copy may carry a different date from the registered checkout's — a deferral
  set in one and not yet merged into the other. `ParkedRow` therefore carries the
  `deferred` object (§5.4) computed from the copy the row resolved to, and `quiet`
  filters on it: each picker judges the copy it would hand out. `next`'s candidates come
  from the command's selected scope — the worktree it runs in for a plain `tasks next`,
  the registered roots under `--project` or `--all-projects` — and are judged there. An
  unresolved row has no record and no deferral.
- **`sample`** excludes deferred records silently, beside its age and status exclusions;
  a due one is drawn.

### 4.3 Ordering

A due deferred `todo` sorts by the existing `ready_order`. Being past its date does not
jump the queue: priority is the field for urgency (periodic §4.6).

## 5. Surfacing

### 5.1 Rows

Wherever a row of a deferred or due record renders — `list`, `ready`, `next`, `prime`'s
ready and roadmap sections, `tree` — the date follows the title in the emphasis style
where the periodic marker sits (the two never coexist, §2.4): `defer 2026-11-10` while
deferred, `due 2026-11-10` once the date has passed. Every one of those views renders
through `output::table`, so the marker has one site. The row's date column is untouched,
for the reason periodic §5.1 gives: one `DateColumn` per list.

`show --pretty` renders `defer` and whether it is due.

### 5.2 `list --deferred`

Every record carrying `defer`, ordered by date ascending with id breaking ties, so the
soonest leads and due records lead everything. The date column is `DateColumn::Due`,
reused from `--periodic`. `--deferred` conflicts with `--periodic`, `--parked`, `--sort`,
and `--reverse`, each of which imposes its own ordering; ordinary filters (`--tag`,
`--owner`, `--source`, `--parent`, `--status`, `--project`) intersect as usual. No
status bypass is needed: a deferred record is open by invariant (§3.2).

### 5.3 `prime`

One line under the counts, beside the periodic line and omitted when no record carries
`defer`:

```
deferred: 3 waiting, next 2026-11-10 (in 56d); 1 due
```

`waiting` counts deferred records and `next` is the earliest of their dates, with the
calendar-day difference of periodic §5.3. `due` counts records whose date has passed and
still carry the field — the revisit queue: due todos are in `ready` above, and due ideas
are in the roadmap with their `due` marker. The `; N due` clause is omitted when it is
zero.

The roadmap keeps deferred ideas and todos where they are, marked (§5.1). Hiding them
from the roadmap was considered and rejected: the roadmap is the map of intent, and a
dated item is intent with a date, whereas the pickers are where hiding pays.

### 5.4 JSON shapes

`Task` carries the raw field: `"defer": "2026-11-10"` or `null`.

`TaskSummary` and `ShowOut` gain one nullable nested object, named apart from the raw
field so the two shapes never collide:

```json
"deferred": { "until": "2026-11-10", "due": false }
```

Present whenever `defer` is set, `null` otherwise. `due` is `is_defer_due`. `ParkedRow`
carries the same `deferred` object, from the copy of the record the row resolved to
(§4.2), `null` for an unresolved row.

`PrimeOut` gains the aggregate its line renders:

```json
"deferred": { "waiting": 3, "next": "2026-11-10", "in_days": 56, "due": 1 }
```

`next` and `in_days` are `null` when `waiting` is `0`.

## 6. Validation and errors

Enforced in `validate_task`, hence surfacing as `parse` findings from `check` (§3.2): a
malformed date, and `defer` beside `every`.

`check` has two findings of its own: one spans two files, the other is the status rule
that §3.2 keeps out of parsing:

| Finding         | Level | Condition                                                  |
|-----------------|-------|------------------------------------------------------------|
| `deferred_goal` | error | `defer` on a task that has children                        |
| `defer_status`  | error | `defer` on a `doing`, `shelved`, `done`, or `dropped` record |

Refusals at the moment of the write, each a validation error naming the value or the ids:

- `--defer` with a date not after today (§3.1);
- `--defer` on a task carrying `every`, and `--every` on a task carrying `defer` (§2.4);
- `--defer` on a task with children, and parenting under a task that carries `defer`
  (§3.3);
- `--defer` on a `doing`, `shelved`, or closed record, and an editor save that keeps such
  a status and carries `defer` (§3.2); the error names the status;
- `edit --status` with `--defer`, as a clap conflict, and an editor save that changes the
  status and the date together (§2.2);
- `--defer` and `--no-defer` together, as a clap conflict.

## 7. Documentation and protocol

Landing in the same change: this spec's status header; the field checklist of the core
design (`2026-08-29-tasks-design.md` §3.4) applied in full — the §3.1 row, the §5.1
addenda for `Task`, `TaskSummary`, `ShowOut`, and `PrimeOut`, and the add/edit usage
blocks; `README.md`; and `skills/tasks/SKILL.md`, which needs the flag under "Recording
work" (an idea to revisit later is `tasks add "<title>" --status idea --defer 60d`), the
`--defer`/`--no-defer` entries in the "Never edit `tasks/*.md` directly" flag list, the
`ready` omission rule and `list --deferred` in the session protocol, and the note that
`start` spends a deferral.

Closing the task also moves the two motivating prism records onto the field
(`tasks edit prism-49a068 --defer 2026-11-10`, and the same for prism-8a8eac, from any
checkout by prefix routing) and records the outcome on feedback tasks-f5ab4a.

## 8. Testing

The due state is reachable end to end without an injectable clock: the flag refuses a
past date, but the editor save does not (§3.1), so a test writes `defer: 2026-01-01`
through `$EDITOR` and the record loads as due. The deferred state is `--defer 60d`.

End-to-end coverage in `tests/cli.rs`:

- `--defer 2026-12-31` and `--defer 60d` store an absolute date, and the record is absent
  from `ready`, `next`, and `sample` and present in `list --deferred` and `prime`'s
  roadmap with the `defer` marker; `ready` carries the single summary warning;
- a due record (editor save) is in `ready` with the `due` marker, `next` returns it, and
  `sample` may draw it (`--seed`-fixed);
- the flag refuses today, a past date, `0d`, a malformed value, and a deferral on a task
  carrying `every`, with children, or in `doing`; `--every` refuses on a deferred task;
  parenting under a deferred task refuses via `edit` and via the editor save;
- every status transition clears the field: `start`, `done`, `drop`, `shelve`, and
  `edit --status todo` on a deferred idea; an `edit` that changes only priority keeps it;
  `--no-defer` clears it; `--defer` with `--no-defer` is a flag conflict;
- the editor path of §3.2: a save that changes a deferred todo's status to `doing` and
  leaves the date succeeds and the saved record has no `defer`; a save that keeps the
  status `doing` and adds a date is refused naming the status; a save that changes the
  status and the date together is refused, and the record is unchanged;
- the combined-write rule of §2.2: `edit --status todo --defer 60d` is a flag conflict,
  and the two-command sequence leaves a deferred todo;
- a parked-waiting-on-agent record that is deferred is not returned by `next` and not
  listed by `quiet`, `prime` still lists the park, and `next` carries the omission
  warning even when the deferred record is an idea and the ready list is empty;
- `quiet` judges the resolved copy: with the parked worktree's record deferred and the
  main checkout's not, the row is omitted; with the dates reversed, it is listed, and the
  row's `deferred` object reports the worktree's copy;
- a deferred todo holds its dependents and does not satisfy them (§2.3);
- `prime`'s line and its JSON aggregate across waiting-only, due-only, and both;
- `list --deferred` ordering with a due record leading, its intersection with `--tag`,
  and its conflicts with `--periodic`, `--parked`, `--sort`, and `--reverse`;
- `check` reports `deferred_goal` on a hand-written pair and `defer_status` on a
  hand-written `done` record carrying `defer`; a hand-written record with `defer` beside
  `every` or with a malformed date fails the scan as a `parse` finding;
- frontmatter round-trip with the field present and absent, and the nested JSON shape in
  `list`, `show`, and `next`.

Unit tests keep the predicates: `is_deferred` and `is_defer_due` one day before, on, and
one day after the date, and against a `now` late in the UTC day; the `<n>d` form added
to a fixed date; and `is_actionable` for a deferred todo, a due todo, and a plain todo.

## 9. Out of scope

- A deferral on a goal that hides its subtree (§3.3).
- Local-time or timezone-aware dates (§2.1).
- Combining a deferral with a recurrence (§2.4).
- Any notification when a date arrives; `prime` is the surface.
- A `check` finding for dependents of deferred work (§2.3).
- Sorting `ready` by due date (§4.3).
