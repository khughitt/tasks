# Park: setting a task down with its next step

Status: designed (2026-09-09); not yet implemented
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
rejected. Every phase is already derivable from the record (§4), and a new status variant
changes the JSON contract, `ready`, `closeout`, and every consumer. What is *not*
derivable is the next step and who it waits on. That is what park records.

## 2. Decision

One command, `tasks park`, that records four optional fields on the task: when it was
parked, the one-line next step, whether it waits on the user or the agent, and the session
that set it down. Status is untouched; park works on any open task. `start`, `done`, and
`drop` clear the fields. `prime` gains a `parked` section, `next` prefers parked work that
waits on the agent, and `list --parked` feeds pickers.

Phase is derived from the record's spec and plan links and shown on parked rows only.

## 3. The command

    tasks park <id> "<next step>" [--waiting-on user|agent]

- `<id>` may be any open task: idea, todo, doing, or blocked. A done or dropped task fails
  with the invalid-transition error, from and to spelled `done` and `parked` (or `dropped`
  and `parked`).
- `<next step>` is a non-empty single line, validated at every write path with the same
  rule and error shape as `title` and `source`.
- `--waiting-on` defaults to `agent`. Any other value is a validation error listing the
  two accepted values.
- Park appends a note, `parked (waiting on <who>): <next step>`, so re-parking leaves a
  trail. Re-parking replaces the four fields; the notes accumulate.
- Park routes by the id's prefix like every other id-taking command, so it works on a task
  in another registered project.

### 3.1 Claims

Park is the moment a session lets go, so it releases the caller's own claim.

- A live claim held by another session fails with `claimed`, naming that session, through
  the same guard status changes use. No `--force`: taking over someone's task to park it
  is not a thing.
- The caller's own claim is removed after the file write succeeds. If the store write then
  fails, the park has landed and a warning says the claim was not released, in the shape
  `note` uses for a failed heartbeat.
- No claim at all is fine. An idea being brainstormed was never started.

## 4. The record

Four frontmatter fields, written after `source` and before `spec`, omitted when absent.
They are all present or all absent; a partial set is a parse error naming the file, in the
shape the second `## Notes` heading uses.

| Field        | Value                                                                  |
|--------------|------------------------------------------------------------------------|
| `parked`     | timestamp of the park, same format as `created` and `updated`          |
| `next_step`  | the one line; quoted by the writer whenever the frontmatter subset needs it |
| `waiting_on` | `user` or `agent`                                                      |
| `session`    | opaque, scheme-tagged session reference (§6); never interpreted        |

### 4.1 What clears the fields

| Command            | Effect on the park record                                        |
|--------------------|------------------------------------------------------------------|
| `start`            | cleared; appends nothing, the park note and the new claim suffice |
| `done`, `drop`     | cleared; a closed record keeps no resumption state               |
| `block`, `unblock` | untouched; waiting on the user is not blocked, and a blocked task may carry a next step |
| `note`, `dep`      | untouched                                                        |
| `edit`             | untouched by flags; the editor path validates the set (§7)      |
| `park`             | replaced                                                         |

### 4.2 Derived phase

Phase is never stored. It is computed for parked rows, first match wins:

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
listings. It is not on `show` or on any unparked row, so the derivation touches one output
shape.

## 5. Surfacing

- **`prime`** gains `parked`: the parked open tasks, most recently parked first. Rows are
  the summary row shape plus `phase`, `parked`, `next_step`, `waiting_on`, and `session`.
  Pretty output prints the section before `ready`, one row per task with phase, who it
  waits on, and the next-step line.
- **`next`** returns the most recently parked task whose `waiting_on` is `agent` and which
  has no live claim by another session. Only when there is none does it fall back to the
  first ready task. The output shape is unchanged; the show shape already carries the four
  fields. A parked task skipped for a foreign live claim is named in a warning, as `ready`
  names its omissions. `--project` and `--all-projects` apply as today.
- **`list --parked`** keeps only parked tasks. It combines with the other `list` filters
  and both read scopes. Rows carry the same five extra keys as `prime`'s section. `ready`
  does not change: readiness is about what can be started, and a parked todo is already in
  it on its own merits.
- **Pretty `show`** prints the four frontmatter lines. Pretty `list` rows do not gain a
  column.

### 5.1 JSON shapes

Additive only; no existing key changes.

- Every task object (`show`, `next`, and the summary rows of `list`, `ready`, `prime`,
  `tree`) gains `parked`, `next_step`, `waiting_on`, and `session`, each `null` when the
  task is not parked.
- `prime` gains `parked: [row]`, where `row` is the summary row plus `phase`.
- `list --parked` rows gain `phase`.
- `park` returns the id shape every mutating command returns.

## 6. The session value

The claim store keeps the raw session string for matching and is not changed. Park writes
a tagged copy to the record, following the level that resolved the identity:

| Resolved from            | Written as              |
|--------------------------|-------------------------|
| `TASKS_SESSION`          | verbatim; the caller tags it |
| `CLAUDE_CODE_SESSION_ID` | `claude:<id>`           |
| Unix session id          | `sid:<pid>`             |

tasks never reads the value back for anything. A picker decides what a scheme means,
exactly as consumers of `source` do. Resume is harness-specific and perishable: the
transcript may be gone, or on another machine. The human handle for a parked task is its
title plus the next-step line, not the session.

## 7. Validation and `check`

- Write paths (`park`, the editor path) reject an empty or multi-line `next_step` and an
  unknown `waiting_on`.
- Reading a file with some but not all of the four fields is a parse error naming the
  file; `check` reports it like any other unparseable record.
- `parked` must parse as a timestamp in the record's format.
- `check` does not verify the session value or the next step against anything. Neither
  points at a thing tasks can see.

## 8. Documentation and protocol

- **Field checklist.** Every place a task field must land, per tasks-4e1cae: the field
  table and JSON shapes block of the tasks design (`docs/specs/2026-08-29-tasks-design.md`
  §3), its usage blocks in §5, `skills/tasks/SKILL.md`, the README, and the `Task`
  struct-literal sites in `src/`. The implementation plan lists each.
- **Protocol.** The tasks skill and this repository's agent guide gain one rule: park
  before ending a turn that waits on the user, with the next step as the message. Both
  currently describe `next` as the first ready task; the same commit changes that to
  "parked work waiting on the agent, else the first ready task".
- **Hook interface.** The familiar-side session-end hook (fam-5b276b) needs no new
  command. `tasks prime` JSON exposes live claims with their sessions, so a hook can find
  the task its session still holds, and `tasks park <id> "<placeholder>" --waiting-on
  agent` writes the record. The placeholder text is the hook's business.

## 9. Testing

- **Unit.** Frontmatter round trip of the four fields including a quoted `next_step`; the
  all-or-nothing rule; phase derivation over the four rules and their order; session
  tagging over the three resolution levels.
- **End to end** (`tests/cli.rs`, against the built binary): park then `show`; re-park
  replaces the fields and leaves two notes; `start`, `done`, and `drop` each clear;
  `block` leaves the record; park releases the caller's own claim and refuses a foreign
  live one with `claimed`; park on a done task fails; `prime` lists parked before ready
  with `phase`; `next` prefers an agent-parked task over a ready one, skips a user-parked
  one, skips a foreign-claimed one with a warning, and falls back to ready; `list --parked`
  filters and combines with `--project`; `check` rejects a partial field set.

## 10. Out of scope

- The picker itself (tasks-202e1f): a script over `list --parked` JSON, not part of the
  binary.
- The session-end hook (fam-5b276b).
- Any interpretation of `session`, including launching or resuming anything.
- An explicit phase override. If derivation proves wrong in practice, that is a new
  decision, not a flag.
