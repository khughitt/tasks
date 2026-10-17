# Wait-aware review state: reviewed ideas that wait on nothing come back

Status: draft for review (revised after review round 1, 2026-10-17: backfill freeze and writer pause, checkout-scoped dependency views, equal stamps count as woken)
Task: tasks-1a3280 (parent tasks-5408fe, lane tasks-46d207; answers tasks-163055)
Brief: `docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md`
Amends: `docs/specs/2026-10-11-review-state-design.md` §1, §3.3, §4, §5, §6

## 1. Goal

A scope pass sets `reviewed` on an idea it briefs. Today the idea returns to the scope
queue (`--needs-review`) only when someone writes `note --no-reviewed`. When nothing
recorded will prompt that note, no default pass ever reaches the idea again. The brief
measured 25 such ideas of 59 reviewed in one project, and 139 reviewed ideas with no
dependency edge across the registry.

This design makes the queue derive re-entry from the record. A reviewed idea stays out
of the queue only while it waits on something recorded. When the wait ends, or when it
never had one, it comes back on its own, and no waker has to remember a note.

The user's answers to the brief's four questions (2026-10-17) are inputs:

1. A park waiting on the user is not a wait, because parks are host-local. The wait goes
   on the synced record. This design chooses the marker (§3.1).
2. A pending drop proposal is a wait, read through the shared `pending_proposal` helper
   (tasks-22f407).
3. Re-entering ideas sort in two tiers, told apart by a synced `reviewed_at` stamp and
   by a new `closed` stamp on done and dropped tasks. A dropped dependency is new
   evidence. A moved dependency is followed to its successor.
4. The backfill runs per project, from that project's own checkout.

Success:

- A reviewed idea whose last open dependency closes is back in `--needs-review` with no
  further write. This is ops-1a552c's case.
- A reviewed idea with no recorded wait is in `--needs-review`, sorted after the ideas
  that carry new evidence.
- A reviewed idea that waits on the user, on a pending proposal, on an open dependency
  or on a future deferral stays out, on every host alike.
- The scope skill cannot finish a `briefed` or `question` verdict without leaving a
  recorded wait.

## 2. Alternatives considered

The brief's three options (§Alternatives):

1. **A skill rule and one backfill, no CLI change.** Rejected alone. A waker that
   closes with a plain `done` still strands its idea, edge or no edge. ops-1a552c had
   its edge and was stranded anyway.
2. **Warnings at `done`, `drop` and `check`.** Rejected. A warning can be ignored, and
   a `check` warning on every stranded idea repeats on every commit until it is fixed.
3. **Derived re-entry.** Chosen. Option 1's skill rule and backfill are its rollout.

For the wait-on-user marker:

- **A stored boolean `awaiting_user`.** Chosen (§3.1).
- **A reserved tag** (`awaiting-user`). Rejected. It would give a word in the user's
  tag vocabulary a meaning the binary enforces, and readmission and status changes
  would have to strip a tag.
- **The `## Open questions` body heading** that the `question` verdict already writes.
  Rejected as implicit: the queue would depend on body prose, and an answer would have
  to remove the heading to count.
- **A declared need** (as material's `owner`). Rejected, as the user expected: needs
  claim shared resources, and a need does not end when a question is answered.
- **A host-local park.** Ruled out by the user (question 1).

For the tier test, the user's proposal: `reviewed_at`, compared with the close time of
each dependency. `updated` was rejected because incidental notes bump it. The time of
the latest `scope:` note was rejected because it means reading notes again.

## 3. Stored fields

Three new frontmatter keys. Each is written only when set, like `reviewed`, and added to
`KEYS` in `src/format.rs` and to `Task`.

### 3.1 `awaiting_user: true`

The idea waits on the user's answer. The scope `question` verdict sets it.

- **Validity:** only on a reviewed idea. A write that would leave `awaiting_user` on a
  non-idea, or on an idea without `reviewed`, is refused with a validation error and
  changes nothing.
- **Set:** `tasks note <id> --reviewed --awaiting-user "<text>"`, or `--awaiting-user`
  alone on an idea that is already reviewed. `tasks edit <id> --awaiting-user` does the
  same without a note.
- **Cleared:** by `--no-reviewed` on `note` or `edit` (the answer readmits the idea),
  by `tasks edit <id> --no-awaiting-user`, and by every status transition, as
  `reviewed` is (review-state §3.2).
- **Plain note:** a plain `note` on an idea awaiting the user leaves both flags and
  warns, extending the existing warning:
  `<id> stays reviewed and awaiting the user; add --no-reviewed to readmit it for scope`.
- **Editor:** like `reviewed`. A same-status save may add or remove `awaiting_user` on a
  reviewed idea, and is refused when the result breaks the validity rule.

`--awaiting-user` conflicts with `--no-reviewed` and `--no-awaiting-user` (clap
`conflicts_with`).

### 3.2 `reviewed_at: <timestamp>`

The instant of the latest review. It uses the RFC 3339 UTC form of `updated`.

- **Set** to now by every write that sets `reviewed`: `note --reviewed`,
  `edit --reviewed`, and an editor save that adds `reviewed`. Setting `reviewed` again
  on a reviewed idea re-stamps it, because a second review is a new review.
- **Cleared** wherever `reviewed` is cleared: `--no-reviewed`, every status transition,
  and an editor save that removes `reviewed`.
- **CLI-managed:** an editor save that changes `reviewed_at` otherwise is refused, as
  `moved_to` is (`src/commands/edit.rs`).
- **Legacy:** an idea reviewed before this field exists keeps `reviewed` with no
  `reviewed_at`. That is valid, and the tier test treats it as legacy (§4.2).

### 3.3 `closed: <timestamp>`

The instant a task last closed.

- **Set** by `transition()` (`src/commands/mod.rs`) on every move into `done` or
  `dropped`, from the same `now` that stamps `completed`. A recurring completion
  re-stamps it, as it re-stamps `completed`.
- **Cleared** on every move out of `done` or `dropped`, mirroring `completed`.
- **Not set by `move`.** A `moved` tombstone is followed, never closed (§4.1).
- `completed` is unchanged and stays done-only. The wake rule reads only `closed`.
  ops `bin/obs-index` reads `completed` inside `status == "done"` and is unaffected.
  obs reads `completed` as a close time (obs-1d6eb4) and can adopt `closed`
  deliberately.
- **CLI-managed:** the editor refuses a change to it, as it does for `reviewed_at`.
- **Legacy:** tasks closed before this field exists have none. No fallback to
  `completed` is read (§4.2 explains why none is needed).

### 3.4 Check

`check` reports each of these as an error finding, which only a hand edit can produce:

- `awaiting_user` on a non-idea or on an idea without `reviewed`;
- `reviewed_at` without `reviewed`;
- `closed` on a task whose status is not `done` or `dropped`.

## 4. The derived state

### 4.1 Recorded waits

A reviewed idea **waits** when any of these holds:

- **An open dependency.** One of its `depends` resolves, through
  `list::resolve_dependency` (which follows `moved_to` chains with
  `tombstone::follow`), to a task whose status is open. An unresolvable dependency (an
  unreachable project, or a broken or cyclic move chain) counts as open, as `ready`
  already treats it. That keeps an idea out of the queue on a missing read rather than
  waking it, and `list` already warns about the edge.
- **A pending proposal.** The latest note is a `curate:` or `scope:` note carrying
  `proposal:`, by the shared `pending_proposal` helper (tasks-22f407).
- **The user.** `awaiting_user` is set.
- **A future deferral.** `defer::is_deferred(task, now)`: the `defer` date is still
  ahead of the UTC day. The brief did not list this. Without it, every reviewed idea
  that a pass deferred (`--defer 60d`, "a thought to revisit later") would enter the
  queue as stranded the day this ships, although its wait is recorded and synced.

Open children are not a wait. Waits are recorded as edges.

### 4.2 `needs_review` and `review_reason`

`needs_review` keeps its name and its place in every row, and widens for ideas:

| Row | `needs_review` | `review_reason` |
|---|---|---|
| Idea without `reviewed` | true | `unreviewed` |
| Reviewed idea that waits (§4.1) | false | (absent) |
| Reviewed idea, no wait, **woken** | true | `woken` |
| Reviewed idea, no wait, not woken | true | `stranded` |
| Leaf `todo` or `blocked` without process (review-state §4, unchanged) | true | `unassessed` |
| Anything else | false | (absent) |

A reviewed idea that waits on nothing is **woken** when it has `reviewed_at` and either:

- one of its dependencies, resolved as in §4.1, has a `closed` at or after
  `reviewed_at`; or
- its `defer` date has come due and is later than the UTC date of `reviewed_at`.

**Equal stamps count as woken.** Stamps have whole-second precision (`time::now`), and
`time::after` orders writes only within one record. A review and a dependency's close
in the same second therefore carry equal stamps, and the order cannot be recovered.
The rule takes the conservative reading: an equal `closed` is new evidence. The cost is
that a review written in the same second as a close it already saw comes back once as
`woken` rather than `stranded`, so it is drawn earlier. Wrongly demoting a real wake is
the worse error, because a stranded idea may wait several passes. The defer test stays
strict: a deferral comes due at the start of its UTC day, so a review stamped on that
day was written after the deferral came due and has already seen it.

Every other reviewed idea that waits on nothing is **stranded**. That includes every
legacy idea without `reviewed_at`, as the user ruled (question 3). Because a legacy
idea is stranded whatever its dependencies say, dependencies closed before `closed`
exists never need a close time, and no fallback to `completed` is read.

A readmission (`--no-reviewed`) makes an idea `unreviewed`. A proposal or a question
answered with `--no-reviewed` is therefore new evidence without any timestamp test.

### 4.3 Where it is computed

`review::needs_review` grows into `review::state`. It takes the record and a
dependency view, and returns `needs_review` and `review_reason` together, so the
summary row and the filter cannot disagree. The dependency view is built once per read
command:

- Only reviewed ideas' edges are resolved. Other rows need no dependency read.
- A view is bound to one scan, the scan its rows came from. A dependency is read from
  that scan when present there, otherwise through the existing resolver, exactly as
  `list::resolve_dependency` reads it today. Within one view each distinct dependency
  id is resolved at most once. The memo is therefore keyed by the source checkout and
  the dependency id, never by the id alone.
- Parked rows keep their current resolution (park design §5.3). A row that
  `resolve_recorded` (`src/commands/parked.rs`) reads from its recorded checkout uses
  a view bound to that checkout's scan, and a row read from the registered copy uses
  the registered scan's view. The same dependency can be done in one checkout's copy
  and open in another's, and each row's review state follows its own copy, as its
  children, dependencies and documents already do.
- `TaskSummary::of`, `Fields::of_task` and `ParkedRow::resolved` take the view rather
  than resolving edges themselves. Their callers (`list`, `ready`, `search`, `sample`,
  `prime`, `tree`, `lanes`, `parked`) build it from the scan they already hold.

Cost: `list` already resolves every listed row's dependencies for its unreachable
warning, so `list` adds no new reads, and the other views add at most one resolution
per distinct edge of a reviewed idea. The brief counted 111 reviewed ideas with an edge
across the registry, most of them in their own project's scan.

The rule reads one note, the latest, for the proposal test. That is the single
exception to review-state §1's "without reading notes", which question 2 grants.

## 5. Interfaces

### 5.1 JSON

All additive. `needs_review` changes meaning: for ideas it is now true for woken and
stranded ideas too. The brief found no reader of `needs_review` outside tasks
(2026-10-17), and this task authorizes the change.

- `Task` (`show`, `next`) gains `awaiting_user: bool`, always present like `reviewed`,
  and `reviewed_at` and `closed`, each present only when set.
- `TaskSummary` gains the same three fields and `review_reason`, which is present
  exactly when `needs_review` is true.
- `ParkedRow` gains `review_reason`, which is omitted on an unresolved row as
  `needs_review` is.

### 5.2 CLI

- `note`: `--awaiting-user`.
- `edit`: `--awaiting-user`, `--no-awaiting-user`.
- `list`, `ready`, `search`: `--needs-review` is unchanged as a flag and keeps every row
  whose `needs_review` is true.

Following AGENTS.md, the rows land in ops's `cli.toml` first and are adopted here with
`vendored adopt` in the same commit as the parser change.

### 5.3 Pretty

- The `*` mark keeps its meaning: needs a look. Woken and stranded ideas carry it too.
  A separate mark for stranded rows was rejected, because the tier is a draw order for
  scope, not a different action for the reader.
- `show` on a reviewed idea prints `reviewed  yes (<reviewed_at date>)`, or `yes` alone
  for legacy, and an `awaiting  user` row when set. A closed task shows `closed` beside
  `completed`.

### 5.4 Order

`list` keeps its sort. Draw order belongs to the scope skill, which reads
`review_reason` (§6).

## 6. Scope skill

`skills/scope/SKILL.md`:

- **§1 selection.** The batch draws from `unreviewed` and `woken` ideas first. When
  none is eligible, it draws from `stranded` ideas. Only then does it draw from
  `unassessed` rows as re-scopes. Stranded ideas come before process-less todos, so the
  "ideas first" rule stands, and the stranded ideas the user wants back are not starved
  by todo intake. Replace the paragraph saying an idea "returns only when a waking
  finding is noted": it now also returns when its wait ends or when it has none.
- **§3 verdicts.** Each verdict leaves a recorded wait or leaves the queue another way:
  - `briefed` requires at least one `dep --on` edge to a follow-up (§5 already says to
    add it). A brief with no follow-up and nothing to wait on is not `briefed`. It is
    `question`, when the user must decide, or `shelved` with a wake condition.
  - `question` writes its audit note with `--reviewed --awaiting-user`.
  - `drop` needs nothing new. Its pending `proposal:` is the wait.
  - `scoped` and `shelved` leave `idea`, so they are out of the queue.
- **Re-scoping a woken or stranded idea.** Gather the closed follow-ups' outcomes as
  evidence (§2) before choosing a verdict. A re-review re-stamps `reviewed_at`, so
  edges closed earlier stop counting as new evidence.
- **§5 "Ideas it wakes".** The line stays, because it carries the finding to the idea.
  It no longer carries readmission: the closing waker readmits through the edge. Its
  `--no-reviewed` wording stays, since it makes the idea `unreviewed` with the finding
  attached before the waker closes.
- **Answering.** A person answering a question or rejecting a proposal does so with
  `note --no-reviewed "<answer>"`. That line goes in scope §3 and in the curate skill
  where it names how proposals are answered.

`skills/tasks/SKILL.md` updates the `--needs-review` paragraph: reviewed ideas that
wait on nothing are in the queue, the four `review_reason` values, and the
`--awaiting-user` marker.

## 7. Rollout

### 7.1 Every reading host first

As with `reviewed` (review-state §7.0), the parser rejects unknown keys. Older binaries
cannot read a record carrying `awaiting_user`, `reviewed_at` or `closed`. `closed` is
written by every `done` and `drop`, so installing the binary on one host is itself the
first write the next time a task closes there, and that write breaks every older host.
The plan therefore names the reading hosts and upgrades all of them before any of them
closes a task or writes a review with the new binary, checking each one's
`tasks --version` before writes resume. The README gains the constraint beside the one
for `reviewed`. Skills ship after the binary on each host.

### 7.2 What changes on the first day

The queue grows by every reviewed idea without a recorded wait, all of them `stranded`,
because none has `reviewed_at`. The brief measured about 139 registry-wide before ops's
2026-10-17 drops. They sort after `unreviewed` and `woken` ideas, so passes on new work
are not displaced, and the backfill (§7.3) shrinks them.

### 7.3 Backfill

The backfill runs once per registered project, from that project's own checkout, after
the binary and skills ship there. It uses only the CLI and follows review-state §7.2's
pattern: decisions are frozen against a record stamp, and writes apply only to records
that have not changed since. A report can go stale before it is applied. For example,
an idea classified as waiting on the user may be re-reviewed by a scope pass that gives
it a research edge. Applying the stale `--awaiting-user` would still succeed, since the
idea is still reviewed, and when the research closed the obsolete marker would keep the
idea hidden. A stale edge fails the same way. Claims and stale-copy checks cannot catch
this, because they guard the write, not the assumptions the report made.

1. **Report and freeze (read-only toward records).** For each idea with
   `review_reason: stranded`, read its latest `scope:` note and extract the task ids it
   names. Resolve each, following moves, and keep those that are open and are not the
   idea itself. Classify the idea:
   - **auto edge**: exactly one open id is named. The decision is
     `tasks dep <idea> --on <id>`.
   - **judge**: several open ids, or none; prose that waits on the user; or a note
     naming only closed tasks (a missed wake, or a closed edge that was re-briefed).
   Write every candidate to a snapshot file outside the project checkouts (the plan
   names the path, under the operator's state directory). Each entry carries the id,
   the record's `updated` stamp at the freeze, the class, the evidence, and its
   decision. The snapshot is written once and never re-derived: a later run that finds
   it reads it rather than reading notes again.
2. **Rule.** The operator reviews the snapshot and records a decision on each judged
   entry, editing only the snapshot, never a record. The decision is one of: an edge
   (one `dep` command, which may name several `--on`); `edit --awaiting-user`; or
   `leave`, the default for missed wakes and closed edges, since those need a re-review
   rather than a marker. An entry with no decision is not applied.
3. **Apply, inside a writer pause.** Each apply pass, the first and every rerun, runs
   inside the same writer pause as review-state §7.2. Before the pass, `tasks claims` on
   each reading host shows no live claim in the project, and the operator explicitly
   confirms that every other writer of the project's records is paused on every host.
   For each entry not yet marked done, the pass reads the record:
   - When it is still an idea, still reviewed, and its `updated` equals the frozen
     stamp, the pass runs the entry's command (or nothing, for `leave`) and marks the
     entry done.
   - Otherwise it writes nothing, reports the entry as changed since the freeze with
     the record's current status, `reviewed` and `review_reason`, and marks it done. A
     changed idea has had a later look, and that look decides its wait.
   - A live-claimed idea is skipped and reported. Its entry stays not done and is
     retried on the next pass under the same unchanged-record test.

   An interrupted pass is safe. When a command landed before its entry was marked
   done, the write bumped `updated`, so the next pass reports the entry as changed
   and does not repeat it.

   The pause is needed for the reason review-state §7.2 gives: the stamp test and the
   write are two commands. An atomic expected-stamp option on `dep` and `edit` (an
   `--if-updated <stamp>`) would remove the pause, but it is permanent CLI surface for
   a one-time migration. Review-state §7.2 rejected it for the same reason, and this
   design does too.

A project without `[git] commit` gets one `chore(tasks)` commit of the run's writes. A
project with `[git] commit = true` commits each write as the CLI does.

`reviewed_at` and `closed` are not backfilled from history (§9).

### 7.4 Wakers already filed

Legacy "Ideas it wakes" bodies with a plain `note` (tasks-d4e73d counted 108) no longer
strand their ideas. When the waker closes, the edge readmits the idea, provided the
edge exists. The backfill adds the missing edges.

## 8. Acceptance

Tests in `tests/cli.rs` and in the unit tests of `review.rs`, `format.rs` and the
commands:

1. **Edge wake:** a reviewed idea (with `reviewed_at`) depending on an open todo is not
   in `--needs-review`. After `done` on the todo it is, with `review_reason: woken`, and
   no write touched the idea. The same holds after `drop` on the todo.
2. **Move followed:** a reviewed idea depending on a task that `move` sends to another
   project stays out. It wakes when the successor closes.
3. **Unreachable:** a dependency in an unregistered project keeps the idea out, and
   `list` warns about it as before.
4. **Stranded:** a reviewed idea with no edges, no proposal, no marker and no deferral
   is `stranded`. A legacy reviewed idea (no `reviewed_at`) whose dependency closed is
   `stranded`, not `woken`.
5. **Closed before review:** a dependency closed before `reviewed_at` gives `stranded`.
   Re-reviewing a woken idea re-stamps `reviewed_at` and turns it `stranded` (or keeps
   it out, if a new edge was added).
6. **Proposal:** a reviewed idea whose latest note is a pending `scope:` drop proposal
   stays out. A later plain note answers the proposal and the idea comes back
   (`stranded`, or `woken` if an edge closed after review).
7. **User wait:** `note --reviewed --awaiting-user` keeps the idea out.
   `note --no-reviewed` clears both flags and `reviewed_at` and gives `unreviewed`. A
   plain note warns with the extended text and keeps both flags. `--awaiting-user` on
   an unreviewed idea without `--reviewed`, or on a todo, is refused and the file stays
   byte-identical.
8. **Deferral:** a reviewed idea deferred to a future date stays out. Once the date has
   come due (under a test clock) it is `woken` when the defer date is after the date of
   `reviewed_at`.
9. **Stamps:** `done` writes `closed` equal to `completed`. `drop` writes `closed` and
   no `completed`. Reopening clears `closed`. A recurring completion re-stamps both.
   `move` writes no `closed`. `note --reviewed` writes `reviewed_at`, and every status
   transition clears `reviewed`, `reviewed_at` and `awaiting_user`.
10. **Editor:** a save that changes `reviewed_at` or `closed` is refused. A save that
    adds `reviewed` stamps `reviewed_at`. A save that removes `reviewed` while keeping
    `awaiting_user` is refused.
11. **Check:** hand-written `awaiting_user` on a todo, `reviewed_at` without `reviewed`,
    and `closed` on an open task are each a finding.
12. **Agreement:** extend review-state acceptance 10 and 15 with woken, stranded and
    waiting ideas. The `*` marks, `needs_review`, `review_reason` and the
    `--needs-review` filter agree in `list`, `ready`, `tree`, `prime` and
    `list --parked`.
13. **JSON:** `review_reason` is absent exactly when `needs_review` is false.
    `awaiting_user` is always present on `Task` and `TaskSummary`.
14. **Backfill** (the plan's tool, against a fixture project):
    - One open id named gives an auto edge; two ids, none, or only closed ids give
      judge.
    - Apply runs the decided commands only on records whose `updated` matches the
      freeze.
    - An idea frozen as `--awaiting-user`, then re-reviewed with
      `note --reviewed` and a new `dep --on` before the apply, is reported as changed.
      It gets no marker and wakes when the new dependency closes.
    - An interrupted pass (command landed, entry not marked) reports the entry as
      changed on the rerun and writes nothing.
    - A second pass rewrites no record and reads no notes.
    - An idea skipped for a live claim, and untouched since, is applied on the next
      pass.
15. **Equal stamps:** a reviewed idea whose dependency's `closed` equals its
    `reviewed_at` to the second is `woken`, tested at the `review::state` level with
    fixed stamps. A dependency closed one second before `reviewed_at` gives `stranded`.
    A deferral due on the date of `reviewed_at` gives `stranded`, and one due on a
    later date gives `woken`.
16. **Divergent checkouts:** a reviewed idea is parked in a worktree whose copy of its
    dependency is `done` with `closed` after `reviewed_at`, while the registered
    checkout's copy of that dependency is still open.
    - `list --parked` and `prime`'s parked section, reading the recorded worktree, show
      the idea `woken`.
    - `list --needs-review` in the registered checkout leaves it out, because it is
      still waiting there.
    - One command that reads both copies gives each row its own checkout's answer.

## 9. Out of scope

- **A view of the questions the user owes.** Ideas with `awaiting_user` are not parked,
  so `list --parked --waiting-on user` does not show them. The recommendation is that
  tasks-22f407's `list --pending` also include `awaiting_user` ideas, so it becomes the
  one list of decisions the person owes. That task is noted with the recommendation;
  this design does not change its scope.
- **Backfilling `reviewed_at` from scope notes and `closed` from lifecycle notes.**
  That would let legacy missed wakes sort as `woken`, but it means a note-reading
  migration in every project for a one-time sort difference. The user ruled legacy
  ideas stranded.
- **obs adopting `closed`** (obs-1d6eb4).
- **A done-time hook** (tasks-d4e73d). Readmission no longer depends on a waker's
  action, so that task need not cover it.
- **Pretty sort or filter by `review_reason`.**
