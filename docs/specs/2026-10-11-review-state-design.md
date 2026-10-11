# Review state: which ideas and todos still need a look

Status: accepted (plan pending)
Task: tasks-a9b8d3 (lane tasks-46d207; answers tasks-e34b99)
Brief: `docs/notes/2026-09-30-work-selection-brief.md`

## 1. Goal

`tasks list --pretty` cannot tell a reader which rows still need someone to look at
them. An idea that a scope pass briefed days ago and an idea filed a minute ago render
identically: priority set, `-` for size, complexity and process. Goals render the same
way, because their columns are empty by design. The backlog looks far less reviewed
than it is. Across the registered projects on 2026-10-11 there are 337 open ideas, 140
leaf todos with size, complexity or process missing, and 46 goals with all three empty.

Today the only record of review is the scope skill's audit note. Scope reads each
idea's latest note to decide eligibility (`scope:` or a pending `proposal:` excludes
it, and any later note readmits it), so finding five eligible ideas among 24 took 24
`show` calls (brief, §Current behaviour).

This design adds a left-most `*` column to every pretty task table, marking rows that
need review, like an unread marker. Behind it:

- **Ideas** carry an explicit stored flag, `reviewed`, that a scope pass sets and a
  wake clears. The user chose a top-level boolean shown as `*` on 2026-10-08
  (tasks-e34b99, first note).
- **Todos** need no new field: a leaf todo with no `process` is unassessed (process
  design, `docs/specs/2026-09-13-task-process-design.md`), and that is what marks it.
  The user chose this scope on 2026-10-11.

Success:

- `tasks list --pretty` shows `*` on a never-reviewed idea and on a leaf todo with no
  process, and on nothing else. A briefed idea, an assessed todo, a goal and a doing
  task show no mark.
- `tasks list --status idea --needs-review` returns exactly the ideas a scope pass
  should consider, without reading notes.
- A research result that wakes an idea readmits it in the same command that records
  the finding. An incidental note (curate, a comment) no longer readmits it.

## 2. Alternatives considered

1. **Keep note-derived eligibility and render it** (compute the mark from the latest
   note at read time). No new field, but it keeps the rule the user asked to replace:
   any later note, including curate's, readmits an idea, and the rule lives in prose
   in two skills and in `sample.rs`. Rejected.
2. **A stored boolean on ideas, a derived mark for todos** (this design). One field,
   set and cleared by explicit commands; the todo half reuses the existing
   "unassessed" meaning of `process`.
3. **A review-state enum** (`new`, `briefed`, `question`, `proposed`, …). The scope
   audit note already carries the verdict and its history; the list needs only one
   bit. The brief found no use for the enum (§Alternatives 3). Rejected.

Naming: the stored field is `reviewed`, not `scoped`, because the scope skill's
`scoped` verdict means "promoted to todo", and a briefed idea is reviewed without
being scoped in that sense (tasks-e34b99, first note). The derived row value is
`needs_review`, which reads correctly for both ideas and todos.

## 3. The stored field: `reviewed`

### 3.1 Record

`reviewed: true` in the frontmatter, written only when true, like `parallel` and
`lane`. Absent means false. `KEYS` in `src/format.rs` gains it, and `Task` gains
`reviewed: bool`. A new idea starts without it, so `add` and `feedback` write nothing
new.

### 3.2 Validity

`reviewed` is meaningful only on an idea.

- **Write paths refuse it on a non-idea.** `edit --reviewed` and `note --reviewed` on
  a task whose status is not `idea` fail with a validation error naming the status,
  and modify nothing.
- **Every status transition clears it**, the way it clears `defer`
  (`docs/specs/2026-09-15-defer-design.md`). So an idea promoted to todo, shelved,
  dropped or moved loses the flag, and a task that comes back to `idea` (unshelve,
  `edit --status idea`) arrives unreviewed. That is intended: a shelf's wake condition
  firing, or a task demoted to an idea, is new evidence that warrants a fresh look.
- **The editor path follows the same rule.** `tasks edit <id>` with no flags parses
  the saved text before deciding whether the status changed, so the parse accepts
  `reviewed` on any status. When the save changes the status, `transition()` clears
  the flag, as the flag forms do. When the status is unchanged, `transition()` does
  not run (`src/commands/edit.rs`, the `status == original.status` branch), so that
  branch refuses a non-idea that carries `reviewed`, with the same validation error
  as the flag forms, and keeps the edited text for retry as other editor refusals do.
  On an idea, a same-status save may add or remove the flag.
- **`check` reports `reviewed` on a non-idea** as a finding. With the write paths and
  the editor refusing it, only a hand edit outside the binary can produce it.

### 3.3 Setting and clearing

| Command | Effect |
|---|---|
| `tasks note <id> --reviewed "<text>"` | Appends the note and sets `reviewed` in one write |
| `tasks note <id> --no-reviewed "<text>"` | Appends the note and clears `reviewed` in one write |
| `tasks edit <id> --reviewed` | Sets it |
| `tasks edit <id> --no-reviewed` | Clears it |

The two flags conflict (clap `conflicts_with`). Setting a flag that is already set,
or clearing one that is already clear, succeeds without a warning: a rerun is
harmless. Each write bumps `updated` as any write does, so the scope skill's
revalidation still sees the change.

The note forms exist for atomicity. A scope audit note and the flag, or a research
finding and the readmission, are one fact. Two commands could leave the idea with the
note but not the flag after a failure partway.

A plain `tasks note` on a reviewed idea leaves it reviewed and warns once:
`<id> stays reviewed; add --no-reviewed to readmit it for scope`. That keeps
incidental notes from readmitting ideas while showing a waker using older wording
what it missed (§7.3).

Claims, stale copies and other guards apply to these writes exactly as they apply to
`note` and `edit` today. This design adds no new guard and relaxes none.

## 4. The derived value: `needs_review`

A row needs review when either holds:

- it is an idea without `reviewed`; or
- it is a leaf (`child_count == 0` and not a lane), its status is `todo` or `blocked`,
  and `process` is unset.

Goals never need review by this rule: their empty size, complexity and process are
expected. `doing` tasks are in hand, and shelved and closed tasks are out of sight,
so none of them are marked.

The value is computed in `TaskSummary::of` from fields the summary already has. It is
never stored.

## 5. Interfaces

### 5.1 JSON

- `Task` (the `show` and `next` task object) gains `reviewed: bool`, always present.
- `TaskSummary` (`list`, `ready`, `search`, `sample`, `prime`, `tree` rows) gains
  `reviewed: bool` and `needs_review: bool`, both always present like `parallel`.

Both changes are additive. Consumers to check in the plan: tasks-tui, obs's task
indexing, and the `tests/cli.rs` JSON fixtures.

### 5.2 Filter

`FilterArgs` (`src/cli.rs`) and `TaskFilter` (`src/filter.rs`) gain `--needs-review`,
a switch that keeps rows whose `needs_review` is true. Every command that flattens
`FilterArgs` accepts it (`list`, `search`, `ready`).
`ready` already omits ideas, so there it narrows to unassessed todos. No
`--reviewed` filter is added: `--status idea --needs-review` and its complement cover
the scope pass, and nothing else selects by the flag.

### 5.3 Pretty tables

`task_row` gains a two-character prefix of the id cell, `* ` for a row that needs
review and two spaces otherwise. Like `||` and `type`, it is reserved only when at
least one row in the output needs it (`Marks::of`, `Marks::of_tree`),
so a fully reviewed listing renders as today. The id column keeps its header;
`prime` shares the id width across its sections through column 0 and `tree` already
indents inside the id cell, so a prefix keeps both intact. The mark uses the emphasis
style. Every caller of `task_columns` (`list`, `ready`, `search`, `sample`,
`prime` sections, `tree`) gets the prefix through the shared
`task_row`, so the views cannot disagree.

### 5.4 Parked views

`list --parked` and `prime`'s parked section render `ParkedRow` through
`parked_columns` and `parked_row`, not `task_columns`, so they get the marker
separately:

- `ParkedRow` gains `reviewed: Option<bool>` and `needs_review: Option<bool>`, filled
  from the record by `ParkedRow::resolved` and `None` on an unresolved row
  (`ParkedRow::unresolved`), like the row's other record-derived fields. In JSON both
  are present when the record resolved and omitted when it did not.
- `parked_row` gains the same id-cell prefix, reserved under the same any-row rule.
  An unresolved row shows two spaces: with no record there is nothing to judge.
- `--parked --needs-review` filters through `Fields::of_row`, which gains
  `needs_review`. An unresolved row has no `Fields`, and the existing rule in
  `list_parked` already hides such a row whenever any filter is set; this design
  keeps that rule.

### 5.5 Show

`show` adds a `reviewed  yes` overview row on a reviewed idea, next to the existing
`lane` and `parallel` rows. An unassessed todo already shows `process: unassessed`.

## 6. What does not change

- **Pending proposals stay note-based.** `sample.rs::pending_proposal` still keeps
  an idea whose latest note carries an unanswered `proposal:` out of curate's pool.
  A proposal waits on the user's decision; that is a different signal from review,
  and tasks-84d2a9 and tasks-22f407 already track a view for it. A scope pass that
  proposes a drop sets `reviewed` as well, so the idea also leaves scope's pool.
- **Review is not approval.** `reviewed` says a scope pass has looked at an idea and
  recorded a verdict. It does not make the idea executable, approve a spec or plan,
  or change `ready`, `next` or the phase.
- **History stays in notes.** Each pass still writes its `scope:` audit note. A
  second pass on a readmitted idea sets the flag again and adds a second audit note,
  so repeated reviews need no richer type.

## 7. Rollout

### 7.0 Every reading host first

The record parser rejects unknown frontmatter keys (`src/format.rs`, `KEYS`), so a
binary built before this change cannot read a record carrying `reviewed: true`. Task
records sync between hosts, so the first such write on one host would break `tasks`
on every host still running an older binary.

The first write of the flag on any project (a `note --reviewed`, an
`edit --reviewed`, an editor save, or the backfill) therefore waits until every host
that reads synced task records runs the new binary. The plan names the hosts and
checks each one's `tasks --version` (or the installed binary's build) before that
first write. The skills that write the flag (§7.1) are installed on a host only
after its binary. The README states the constraint beside the existing one for
`move`: "Older binaries cannot parse records carrying `reviewed`: update every host
reading synced tasks before the first review write."

### 7.1 Skills

- **scope** (`skills/scope/SKILL.md`): select with
  `tasks list --status idea --needs-review [--tag] [--project]` instead of reading
  latest notes. An explicit id still bypasses this. Write the audit note with
  `--reviewed` for the `briefed`, `question` and `drop` verdicts. `scoped` and
  `shelved` change the status first, so their audit note is a plain note: the
  transition already cleared the flag, and `--reviewed` would be refused on a
  non-idea (§3.2). The research template's "Ideas it wakes" line becomes
  `run tasks note <id> --no-reviewed "<finding>"`.
- **curate** (`skills/curate/SKILL.md`): say that curate notes do not readmit an idea
  for scope.
- **tasks** (`skills/tasks/SKILL.md`): one line on the `*` column and
  `--needs-review`.
- tasks-91c622 (cross-project scope design) takes `needs_review` as its eligibility
  rule rather than the latest-note rule.

### 7.2 Backfill

Without a backfill, every briefed idea would show `*` and the column would mislead
from the first day. The backfill runs once per registered project, after the binary
and skills ship, as an operator step in the plan. It uses only the CLI:

- **Freeze once.** The first run on a project derives the candidates from notes and
  writes them, with each record's `updated` stamp, to a snapshot file outside the
  project checkouts (the plan names the path, under the operator's state directory).
  A candidate is an open idea whose `reviewed` is false and whose latest note starts
  with `scope:` or is a `curate:` or `scope:` note carrying a pending `proposal:`
  (today's eligibility rule). The snapshot is written once and never re-derived: a
  later run that finds it reads it instead of reading notes.
- **Apply only to unchanged records.** For each snapshot entry not yet marked done,
  a run reads the record. When it is still an idea, still unreviewed, and its
  `updated` equals the frozen stamp, the run executes `tasks edit <id> --reviewed`
  and marks the entry done. Otherwise it leaves the record alone and reports it as
  changed since the freeze, with its current status and `reviewed` value, and marks
  the entry done so later runs do not report it again.
- **Why this holds.** No run after the first decides eligibility from notes, so an
  idea that someone readmitted, scoped, or otherwise touched after the freeze is
  never marked by the backfill. That covers an interrupted run (the edit landed but
  the entry was not yet marked done: the next run sees a changed `updated` and
  reports it, so a later `edit --no-reviewed` stands) and an idea reviewed by an
  ordinary scope pass (not a candidate if reviewed before the freeze; reported as
  changed if reviewed after it). A rerun never rewrites a record it already set.
- A live-claimed idea is skipped and reported, like any edit would refuse; its entry
  stays not done and is retried on the next run under the same unchanged-record test.
- **No other writers during an apply pass.** The stamp test and the edit are two
  commands, and `edit` checks only that the file did not change during its own run,
  not that it still matches the snapshot. A `note --no-reviewed` from another session
  between the two would be overwritten in effect: the edit would set the flag on a
  record carrying the new finding. So each apply pass, the first run and every
  rerun, runs inside a writer pause on that project: the operator stops every
  other session that may write its task records, on every host that reads
  them, before the pass, and resumes them after. The plan states the pause as a
  checklist step before each pass with two parts. First, `tasks claims` run on each
  reading host (claim stores are host-local) shows no live claim on a task in the
  project. Second, the operator explicitly confirms that every other writer of the
  project's task records is paused on every host. That confirmation is required
  because writers need not hold claims: a scope pass, a curate pass, or a plain
  `tasks note` writes ideas without one, so no command can prove the pause. A safe concurrent
  backfill would need a stamp-checked edit inside the CLI; a one-time migration
  does not justify that surface.
- Each project's writes land in one `chore(tasks)` commit in that project.

The writes bump `updated` on the backfilled ideas. That keeps them out of curate's
`sample` age window for a while, which matches tasks-ec3ff9's complaint that sample
spends draws on recently briefed ideas. The list's date column for those ideas will
show the backfill date.

### 7.3 Wakers already filed

Open research tasks filed before this ships say "run tasks note on <idea>" without
`--no-reviewed`. When one completes, its plain note leaves the idea reviewed, and the
warning from §3.3 names the missing flag at that moment. The plan lists these tasks
across projects (`tasks search "Ideas it wakes" --all-projects`) so the controller
can see how many remain. Their bodies are not rewritten.

## 8. Acceptance

Tests in `tests/cli.rs` and the record/model unit tests, without new tooling:

1. **New idea:** adding an idea → no `reviewed` key on disk; JSON `reviewed:
   false`, `needs_review: true`; pretty shows `*`; `--needs-review` includes it.
2. **Briefed idea:** `note --reviewed "scope: briefed; …"` → one write carrying both
   the note and `reviewed: true`; `needs_review: false`; no `*`; filter excludes it.
3. **Explicit readmission:** `note --no-reviewed "<finding>"` on a reviewed idea →
   flag cleared and note appended in one write; marked again.
4. **Repeated review:** readmit, then `note --reviewed` again → two audit notes,
   flag set; no other state.
5. **Incidental note:** plain `note` on a reviewed idea → still reviewed; the warning
   names `--no-reviewed`; a plain note on an unreviewed idea does not warn.
6. **Pending proposal:** `note --reviewed "scope: drop; …; proposal: …"` → excluded
   from `--needs-review` and still reported pending by `sample`.
7. **Non-idea transitions:** a reviewed idea moved to todo, shelved, dropped or moved
   loses the flag. Unshelve and `edit --status idea` return it unreviewed.
   `edit --reviewed` and `note --reviewed` on a todo fail and leave the file
   byte-identical.
8. **Editor:** a same-status editor save that adds `reviewed` to a todo is refused
   and the file is unchanged; an editor save that moves a reviewed idea to todo
   drops the flag; a same-status save on an idea may add or remove it.
9. **Todos:** a leaf todo without process is marked; setting `--process` clears the
   mark; a goal todo without process is not marked; a doing task without process is
   not marked; a blocked leaf without process is marked.
10. **Agreement:** for one fixture project, the ids with `*` in `list --pretty`, the
   ids with `needs_review: true` in JSON, and the ids returned by `--needs-review` are
   the same set, in `list`, `ready` and `tree`.
11. **Column reservation:** a listing with no marked row renders exactly as before
    (no leading column); one marked row reserves it for all rows.
12. **Check:** a hand-written `reviewed: true` on a todo is a `check` finding.
13. **Guards:** on a task live-claimed by another session, and from a stale copy,
    `note --reviewed` behaves exactly as a plain `note` does today and
    `edit --reviewed` exactly as another `edit` flag does: same refusal or same
    success, with the flag written only when the write itself succeeds.
14. **Backfill rerun:** in a fixture project, a second run rewrites no record
    (byte-identical files, unchanged `updated`) and re-derives nothing from notes.
    Each of these leaves the idea unreviewed after a later run: readmission between
    runs with `note --no-reviewed`; readmission with `edit --no-reviewed` while the
    latest note is still `scope:`; a run interrupted after its edit but before
    marking the entry done, followed by `edit --no-reviewed`; and an idea reviewed
    by an ordinary `note --reviewed` after the freeze, followed by
    `edit --no-reviewed`. The last two are reported as changed since the freeze. An
    idea skipped for a live claim on the first run, and untouched since, is set on
    the second.
15. **Parked agreement:** for a fixture with a parked unreviewed idea, a parked
    reviewed idea and an unresolved park, `list --parked --pretty` marks only the
    first; JSON carries `needs_review: true`, `false` and no field respectively;
    `--parked --needs-review` returns only the first; `prime`'s parked section marks
    the same row.

## 9. Out of scope

- A view of pending proposals (tasks-84d2a9, tasks-22f407).
- Cross-project scope selection (tasks-91c622, which adopts §4).
- Moving proposals to `park --waiting-on user` (suggested in tasks-e34b99's first
  note; no design here).
- Rewriting existing research task bodies (§7.3).
