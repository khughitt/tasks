# Park: setting a task down with its next step

Status: implemented (2026-09-09)
Task: tasks-08b9d5; consumers tasks-202e1f (quick launch), fam-5b276b (session-end hook)

## 1. Problem

`doing` is the only active state. A task set down at the end of a session says nothing
about whether it waits on the user or on the agent, or what the next concrete action is.
The information usually exists, but as prose in the last of several notes, invisible in
every list view. The next session re-derives it from the notes or, more often, does not,
and the effort is left silently open.

Two things follow. The morning view (`prime`, `next`) cannot list unfinished work by where
it was left. And the session that did the work is forgotten the moment its claim clears,
so nothing can offer to resume it.

Adding phase states (brainstorming, planning, implementing) to `Status` was considered and
rejected. Every phase is already derivable from the record (§4.2), and a new status variant
changes the JSON contract, `ready`, `closeout`, and every consumer. What is *not*
derivable is the next step and who it waits on. That is what park records.

## 2. Decision

One command, `tasks park`, that converts the caller's hold on a task into a **park entry**
in the shared claim store: when it was parked, the one-line next step, whether it waits on
the user or the agent, and the session that set it down. The record itself gains only a
note. Status is untouched; park works on any open task. `start` replaces the park entry
with a live claim; `done` and `drop` remove it. `prime` gains a `parked` section, `next`
prefers parked work that waits on the agent, `ready` omits work that waits on the user, and
`list --parked` feeds pickers.

Phase is derived from the record's spec and plan links and shown on parked rows only.

### 2.1 Why the store, not the record

The first draft wrote four frontmatter fields and released the claim. Review found two
faults, both structural. Releasing the claim removed the one mechanism that makes work in
a worktree visible from the main checkout (the work-claims design exists because a
branch-local `doing` was invisible elsewhere). And a record field cannot represent
cancellation: park in worktree A, then `start` and `done` in worktree B, and A's unchanged
frontmatter says parked again the moment it is scanned.

The store has neither fault. It is shared by every worktree of a prefix, and one entry per
task means "parked", "claimed", and "neither" are exclusive states that any worktree can
change. The record keeps the durable trail as a note. The trade is the one the claims
design already made: a fresh machine or a fresh clone shows nothing parked until something
is parked there, and losing the store file costs the overlay, never a task.

## 3. The command

    tasks park <id> "<next step>" [--waiting-on user|agent]

- `<id>` may be any open task: idea, todo, doing, or blocked. A done or dropped task fails
  with the invalid-transition error, from and to spelled `done` (or `dropped`) and
  `parked`.
- `<next step>` is a non-empty single line, validated with the same rule and error shape
  as `title` and `source`.
- `--waiting-on` defaults to `agent`. Any other value is a validation error listing the
  two accepted values.
- Park appends a note, `parked (waiting on <who>): <next step>`, so re-parking leaves a
  trail on the record. Re-parking replaces the entry; the notes accumulate.
- Park routes by the id's prefix like every other id-taking command, so it works on a task
  in another registered project.
- Park is a mutation: it runs under the store's mutation lock, and the file write and the
  store write move together in the order the claims design fixed (validate everything,
  then write the record, then the store). If the store write fails after the note has
  landed, the result is a warning in the shape `note` uses for a failed heartbeat, saying
  the note landed but parking was not updated (a previous park entry, if any, is intact).

### 3.1 Store effects

| Store holds before park                  | Result                                          |
|------------------------------------------|-------------------------------------------------|
| nothing                                  | park entry written                              |
| the caller's own claim (live or stale)   | replaced by a park entry                        |
| another session's live claim             | `claimed`, naming that session; no `--force`    |
| another session's stale claim            | replaced by a park entry, with the takeover warning `start` uses |
| a park entry (any session)               | replaced; re-parking is allowed from any session |

No `--force`: taking over a live session's task in order to set it down is not a thing.
An idea being brainstormed without `start` has no entry; parking it writes one, which is
what makes it visible from other worktrees.

## 4. The store entry

The per-prefix store (`claims.rs`) holds at most one entry per task id: a claim, as today,
or a park. Park entries carry:

| Field        | Value                                                                  |
|--------------|------------------------------------------------------------------------|
| `at`         | timestamp of the park, same format as `created` and `updated`          |
| `next_step`  | the one line                                                           |
| `waiting_on` | `user` or `agent`                                                      |
| `session`    | opaque, scheme-tagged session reference (§6); never interpreted        |
| `owner`      | who parked it, from the same identity `start` records                  |
| `host`, `worktree` | where the record that was parked lives, as claims record them   |
| `title`      | snapshot of the task title at park time, for rows whose file is unreachable (§5.3) |

Park entries have no liveness. They are neither live nor stale, do not heartbeat, and are
never pruned by a read. `claim_guard` treats a park entry as free: `start` from any
session replaces it without `--force` and without a takeover warning.

### 4.1 What changes the entry

| Command                                 | Effect on a park entry                         |
|-----------------------------------------|------------------------------------------------|
| `start`                                 | replaced by a live claim; this is resume, and it applies on `doing → doing` too |
| `done`, `drop`                          | removed                                        |
| `edit --status` / editor save that *changes* status to `doing`, `done`, or `dropped` | as `start`, `done`, `drop` |
| any save that leaves status unchanged   | untouched (§4.3)                               |
| `block`, `unblock`                      | untouched; waiting on the user is not blocked, and a blocked task may carry a next step |
| `note`, `dep`, field edits              | untouched                                      |
| `park`                                  | replaced                                       |

### 4.2 Derived phase

Phase is never stored. It is computed for parked rows from the resolved record, first match
wins:

| Rule                          | Phase           |
|-------------------------------|-----------------|
| status is `idea`              | `brainstorming` |
| has `plan` or `step`          | `implementing`  |
| has `spec` but no `plan`      | `planning`      |
| otherwise                     | `implementing`  |

The last row is deliberate. A todo with neither spec nor plan is scoped work that never
needed a design; the absence of a spec does not mean brainstorming. Only an idea is
unscoped by definition.

`phase` appears on parked rows in `prime` and `list --parked` and in their pretty
listings; it is `null` when the record could not be resolved (§5.3). It is not on `show`
or on any unparked row.

### 4.3 Editor saves and park preservation

Today the editor path calls `transition` on every save, and the claim guard then acquires a
claim whenever the status is `doing`, changed or not. Under this design that would replace
a park entry on an unrelated field edit. The rule, scoped to that one path: **an editor
save that leaves status unchanged neither acquires nor releases a store entry.** Only
`start` (always), `park`, and an actual change of status touch the entry. On a change, a
**claim** behaves exactly as it does today (acquired on `doing`, released on every other
target, including `blocked` and `todo`); a **park entry** follows §4.1 (replaced on
`doing`, removed on `done` and `dropped`, left in place on `blocked`, `todo`, and `idea`).
Re-acquiring a claim on an unchanged `doing` save was incidental behaviour, not a
contract, and stops.

Nothing else about the store changes:

- `note` keeps its heartbeat on the caller's own claim, and stays permitted on a task
  another session holds; it is never refused.
- `block`, `unblock`, `done`, and `drop` keep releasing a claim as they do now.
- Preserving the entry is not bypassing the guards. A status-preserving editor save still
  fails with `claimed` when another live session holds the task, as it does today, and
  still runs the concurrent-edit checks (`updated` comparison, raw-content comparison,
  newer-sibling-copy warning).

### 4.4 Prefix rename

`tasks rename` today authorizes against live claims, then deletes the source prefix's
store file outright. Park entries have no liveness, so that check would pass and the
deletion would discard every parked task. The store is now authoritative for parking, so
rename **migrates** it. Rejecting the rename while parks exist was considered and declined:
it would force every parked task through `start` and re-park to rename a prefix.

**Preflight**, before any mutation and alongside the live-claim check, on a fresh rename
only; recovery validates the destination against the inventory (below) and never against
emptiness, since the store it finds may be the one it wrote: the target prefix's
store must hold no park entries. `unregister` frees a prefix but leaves its store behind,
so parking under `new`, unregistering `new`, then renaming `old → new` is reachable, and
the migration must not overwrite `new`'s parks. A target store with park entries fails
authorization, naming them: `target store holds N parked tasks (ids…); remove or resume
them before renaming`. Stale claims in either store are dropped as today.

**Expectation.** The inventory records the migration: the bytes of the target store the
rename will write (source parks with ids rewritten to the target prefix, no claims) and
their digest, `store_to`, beside `config_from` and `config_to`. `worktree` paths in entries
are unchanged by a rename. `rename --explain` reports the count of park entries that will
move.

**Order** in the `claims` step: write the target store from the expectation, verify the
written bytes digest to `store_to`, then remove the source store. Recovery accepts an
existing destination only when its digest matches `store_to`; a destination that does not
match is a validation error unless the destination holds no park entries, in which case it
is litter (stale claims at most, since live ones fail authorization) and is replaced;
otherwise the error is in the shape the config rewrite uses when it disagrees with
the inventory, and the source is left in place. With the source already gone, the
destination is verified against `store_to` before the step is considered complete.

The manual rollback procedure in the prefix-rename design gains the store: restore the
source store from the inventory's recorded source parks before deleting the destination,
and remove the destination last.

## 5. Surfacing

The **effective parking state** of a task is the store entry, everywhere: `prime`, `next`,
`ready`, `list`, and `show` all read the same snapshot.

### 5.1 Views

- **`prime`** gains `parked`: every park entry in scope, most recently parked first. Rows
  are the summary row shape plus `phase`. Pretty output prints the section before `ready`,
  one row per task with phase, who it waits on, and the next-step line. Any open status may
  appear; it is a view, not a recommendation.
- **`ready`** omits tasks whose park entry waits on the user, with a warning per omission in
  the shape live-claim omissions use: `<id> omitted: parked waiting on the user: <next
  step>`. Ready means an agent may start it, and a task waiting on a decision is not that.
  Tasks parked waiting on the agent stay in `ready` on their own merits.
- **`next`** returns the most recently parked **candidate** (§5.2). Only when there is none
  does it fall back to the first ready task, which by the rule above can no longer be a
  user-parked task. The output shape is unchanged. `--project` and `--all-projects` apply
  as today.
- **`list --parked`** keeps only tasks with a park entry, resolved the same way `prime`'s
  section is. It combines with the other `list` filters and both read scopes. Rows gain
  `phase`.
- **Pretty `show`** prints a `# parked` footer after the related-task footers: the
  waiting party, the park date, and the next step, then the session and worktree.
  (`show` prints no claim line, so the block is a footer like the others.)

### 5.2 Candidates for `next`

A parked task is a candidate when an agent may act on it alone:

- its record is in the current scan (store-only entries are never candidates, §5.3) and
  its status is open;
- `waiting_on` is `agent`;
- status is not `blocked`;
- every dependency resolves and is closed (an unreachable dependency holds the task,
  exactly as `ready` and `done` treat it);
- it has no children.

An idea is a candidate. A park on an idea is an instruction to **resume scoping**, which is
the one thing `ready` cannot express for ideas. It never authorizes implementing an
unscoped idea; the next step says what scoping remains.

### 5.3 Store-only entries

A park entry whose id is not in the scan belongs to a task that exists only in another
checkout: created and parked in a worktree whose branch has not merged. Reading its file
alone is not enough. Its children, its local dependencies, and its spec and plan live in
that checkout too, and a parent parked there would look childless from here.

**Resolution** opens the entry's recorded `worktree` as a project (it must be a checkout
of the same prefix; anything else counts as unavailable) and scans it. The row is built
from that scan exactly as a local row would be: status, phase, dependencies, child counts.
A task that exists in both checkouts is not store-only; it is scanned locally, and its
row reflects the local copy even where the parked copy has moved on.

**Store-only entries are never `next` candidates**, resolved or not. `start` reads
`tasks/<id>.md` from the current checkout, so a task that is not here cannot be resumed
from here. The warning says where it can be:
`<id> is parked in <worktree>; resume it from that checkout`.

**Unresolved store-only entries** (worktree removed, not a checkout of this prefix, file
missing or unparseable) are rendered from the payload alone (§5.4) with the warning
`<id> is parked in <worktree>, which is unavailable; the row shows the park entry only`.
They stay in `prime` and `list --parked` so nothing goes silent. Reads never delete an
entry; `start`, `done`, or `drop` on the task from a checkout that holds it does.

### 5.4 JSON shapes

Additive only; no existing key changes.

- Every task object (`show`, `next`, and the summary rows of `list`, `ready`, `prime`,
  `tree`) gains `park`, `null` when the task has no park entry, beside `claim`:

      park: { at, next_step, waiting_on, session, owner, host, worktree }

- `prime` gains `parked: [row]`, where `row` is the summary row plus `phase`. Resolved
  rows, local or store-only, are complete summary rows with their real status and phase.
- `list --parked` rows gain `phase`.
- `park` returns the id shape every mutating command returns.

**Unresolved store-only rows** keep the summary row keys so consumers see one shape:

| Key | Value |
|---|---|
| `id`, `title` | from the entry (`title` is the snapshot) |
| `park` | the entry |
| `phase`, `status`, `priority`, `size`, `owner`, `created`, `updated`, `parent`, `source`, `child_count`, `open_descendant_count`, `claim` | `null` |
| `parallel` | `false` |
| `depends`, `tags` | `[]` |

`status: null` is the marker; nothing else in the contract produces it.

**Ordering and filters.** `prime.parked` and `list --parked` order by `park.at`, most
recent first, resolved or not; `--parked` conflicts with `--sort` and `--reverse`.
Unresolved rows match none of `--status`, `--tag`, `--source`, or `--parent`: with any of
those given they are dropped from the output, and the unavailable warning still names
them.

## 6. The session value

The claim entry keeps the raw session string for matching and is not changed. The park
entry stores a tagged copy, following the level that resolved the identity:

| Resolved from            | Written as              |
|--------------------------|-------------------------|
| `TASKS_SESSION`          | verbatim; the caller tags it |
| `CLAUDE_CODE_SESSION_ID` | `claude:<id>`           |
| Unix session id          | `sid:<pid>`             |

tasks never reads the value back for anything. A picker decides what a scheme means,
exactly as consumers of `source` do. Resume is harness-specific and perishable: the
transcript may be gone, or on another machine. The human handle for a parked task is its
title plus the next-step line, not the session.

## 7. Validation and errors

- `park` rejects an empty or multi-line next step and an unknown `--waiting-on` value with
  the existing validation error shape.
- `park` on a closed task fails with the invalid-transition shape (§3).
- A foreign live claim fails with `claimed` (§3.1).
- A corrupt store file fails as it does today for every store reader.
- `check` gains nothing: the record carries only a note, and the store is not the
  record's concern.

## 8. Documentation and protocol

- **Field checklist.** No task field is added, so the field checklist (tasks-4e1cae) does
  not apply. The JSON shapes block of the tasks design (`docs/specs/2026-08-29-tasks-design.md`
  §3) gains the `park` object and `prime.parked` addendum; the work-claims design gains a
  pointer to this document for the second entry kind; the prefix-rename design's cleanup
  step and manual rollback procedure gain the store migration (§4.4).
- **Protocol.** The tasks skill and this repository's agent guide gain two rules: park
  before ending a turn that waits on the user, with the next step as the message; and
  `next` may hand you parked work, including an idea, which means resume its scoping. Both
  currently describe `next` as the first ready task; the same commit changes that to
  "the most recently parked task waiting on the agent, else the first ready task". The
  skill's "never pick an idea" rule gains that one exception.
- **Hook interface.** The familiar-side session-end hook (fam-5b276b) needs no new
  command. `tasks prime` JSON exposes live claims with their sessions, so a hook can find
  the task its session still holds, and `tasks park <id> "<placeholder>" --waiting-on
  agent` writes the entry. The placeholder text is the hook's business.

## 9. Testing

- **Unit.** Store round trip of a park entry beside claims; one-entry-per-id invariant;
  phase derivation over the four rules and their order; session tagging over the three
  resolution levels; `claim_guard` treats a park entry as free.
- **End to end** (`tests/cli.rs`, against the built binary, with `XDG_STATE_HOME` and
  `TASKS_SESSION` set per test so stores and sessions are isolated):
  - park then `show` carries `park`; re-park replaces the entry and leaves two notes;
  - `start` on a parked doing task (doing → doing) replaces the entry with a claim;
  - `done` and `drop` remove it; `block` leaves it;
  - park replaces the caller's own claim, refuses a foreign live one with `claimed`,
    replaces a foreign stale one with a warning;
  - park on a done task fails;
  - `edit --status done` on a parked task removes the entry; an editor save that changes
    only the body leaves the entry untouched **and** still fails with `claimed` when a
    foreign live session holds the task;
  - `prime` lists parked before ready with `phase`; `ready` omits a user-parked todo with a
    warning; `next` with no agent-parked candidate and a user-parked ready todo returns the
    next ready task, not the parked one (the fallback regression);
  - `next` prefers an agent-parked task over a ready one; skips a blocked one, a
    dependency-held one, a goal with children, and one with an unreachable dependency;
    returns an agent-parked idea;
  - **park in worktree A, `start` then `done` in worktree B, inspect A**: A shows nothing
    parked and no resurrection;
  - **park a task that exists only in a new worktree, inspect main**: `prime` in main lists
    it with the worktree named and is not returned by `next` (resume-from-that-checkout
    warning); remove the worktree and it is listed from the payload with `status: null`,
    the unavailable warning, and is still not returned by `next`;
  - **store-only parent and child**: create a parent and a child only in worktree A, park
    the parent, inspect main: the row shows `child_count` 1 from A's scan and the task is
    not a `next` candidate;
  - `list --parked` with `--tag` drops an unresolved row and still warns about it;
    `--parked` with `--sort` fails as a flag conflict;
  - `note` on a parked task leaves the entry untouched; `note` on a task another live
    session claims is still accepted and still heartbeats only the caller's own claim;
  - **rename with parks**: park two tasks, `tasks rename`, and the target prefix's store
    holds both with rewritten ids while the source store is gone; `rename --explain`
    reports the count;
  - **rename onto a pre-existing target store**: park under `new`, `unregister new`, park
    under `old`, `rename old new` fails in preflight naming `new`'s parks, and neither
    store has changed;
  - **rename interrupted between destination write and source removal**: resume completes
    with the destination verified and the source removed; a destination altered in the
    interval fails recovery with the source left in place;
  - **park a doing task, then explicit `start` versus an unchanged-status editor save**:
    the first clears, the second preserves;
  - `list --parked` filters and combines with `--project`.

## 10. Out of scope

- The picker itself (tasks-202e1f): a script over `list --parked` JSON, not part of the
  binary.
- The session-end hook (fam-5b276b).
- Any interpretation of `session`, including launching or resuming anything.
- An explicit phase override. If derivation proves wrong in practice, that is a new
  decision, not a flag.
- Parking visible across machines. The store is per machine, as claims are.
