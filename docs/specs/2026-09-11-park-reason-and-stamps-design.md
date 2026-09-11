# Park reasons and lifecycle stamps: the minimal telemetry a task record carries

Status: approved design, not yet implemented
Task: tasks-82b559 (source: ops-2cb205, the friction diagnosis in ops
docs/reports/2026-09-11-friction-diagnosis.md)

## 1. Problem

A record carries `created` and a mutable `updated`. When work first began and when it
was completed are not recoverable: the first `start` leaves a claim that `park` and `done`
remove, and `updated` moves on every note. The diagnosis had to take the last note's
timestamp as the completion time and could not recover a start at all.

A park says who it waits on (`user` or `agent`) and a free-text next step. It cannot say
*why* the work stopped. Reading transcripts, the reasons fall into a handful of classes
that call for different responses — an art sheet the user must open and judge, a
recommendation the agent already holds and only wants confirmed, a checkout with no
binaries, a session that ran out of context — and today each is prose in a note, uncountable.

## 2. Decision

Two additions, neither changing any existing contract:

- Two optional frontmatter timestamps, `started` and `completed`, stamped by
  `transition()` and by nothing else.
- One optional flag on `park`, `--reason <why>`, with a fixed vocabulary, stored on the
  park entry, printed on parked rows, and written into the park note.

`--waiting-on` keeps its two values and its meaning. Who the work waits on drives the
readers (`ready` omits user-waits, `next` prefers agent-waits); why it stopped is
orthogonal, has no clean mapping onto who (`environment` is nobody's), and so is a
separate field. Absence of a reason means "not recorded"; there is no `unknown` value.

Out of scope, deliberately: an append-only transition history, session-to-task linkage,
and any report over the stamps. Those wait until these fields have been collected for a
while and the diagnosis is rerun against them.

## 3. The stamps

| Field       | Stamped when                                                | Moved by                          | Cleared by                                      |
|-------------|-------------------------------------------------------------|-----------------------------------|-------------------------------------------------|
| `started`   | the first transition into `doing`, when the field is absent | nothing                           | nothing                                         |
| `completed` | every transition into `done` from another status            | the next completing transition    | any transition out of `done` (reopen, recur, drop) |

Rules, in the shape the model-provenance design fixed:

- **Write path.** Both live in `transition()` (src/commands/mod.rs), beside the `model`
  stamp, so `tasks start`, `tasks done`, `tasks edit --status`, and an editor status flip
  all stamp identically. `started` fires when `to == Doing && task.started.is_none()`;
  later starts, resumes from park, and `--force` takeovers leave it alone — it is
  *first* start. `completed` fires exactly when `completing` is true (the same predicate
  that stamps `model`), and is set to `None` whenever `task.status == Done` and
  `to != Done`. The claim-release retry branch (`ctx.recovered`) stamps nothing, as it
  does for `model`.
- **Relation to `last_done`.** `last_done` is the recurrence anchor: it requires `every`,
  survives a reopen, and is cleared only with the cadence. `completed` is telemetry: it
  exists on every record and clears on reopen. A recurring completion stamps both with
  the same instant. They are two fields because their clearing rules differ, and neither
  is renamed.
- **Not editable.** `check_invariants` (src/commands/edit.rs) refuses an editor save that
  sets or moves either stamp, with the `last_done` error's shape: "`started` is stamped
  by starting the task; it cannot be edited" / "`completed` is stamped by completing the
  task; it cannot be edited". Clearing them by hand is refused too: unlike `last_done`,
  no other field's edit needs to clear them. No `edit` flags; a wrong stamp is a
  transition bug to fix, not a value to correct.
- **Frontmatter.** Written after `updated` and before `last_done`, as `Value::Raw`, and
  added to `quote_timestamps` beside their neighbours so they parse as strings. Omitted
  when absent. `KEYS` gains both. Records without them parse and `check` says nothing;
  the corpus is not backfilled.
- **Validation.** Each must parse as an RFC 3339 UTC timestamp (`crate::time::parse`),
  as `last_done` must. `completed` on a record whose status is not `done` is *not* a
  parse error: the editor path parses the edited text before `transition()` runs, so a
  status flip from `done` in the editor necessarily passes through that state and the
  transition then clears the stamp. It is instead a `check` finding ("completed stamp
  on an open task"), which only a hand edit outside the binary can produce.
- **JSON.** Every task object gains `started` and `completed`, `null` when absent: the
  full `Task` in `show` and `next`, `TaskSummary` in `list`, `ready`, `prime`, and
  `tree`, and `ParkedRow`. Additive; no existing key changes.
- **Pretty output.** `show --pretty` prints the two frontmatter lines. Tables gain no
  column.

## 4. The reason

    tasks park <id> "<next step>" [--waiting-on user|agent] [--reason <why>]

| reason        | The work stopped because…                                                        |
|---------------|----------------------------------------------------------------------------------|
| `review`      | an artifact the user must inspect and judge: art sheets, screenshots, a document read |
| `decision`    | a decision only the user can make — scope, taste, priority                       |
| `approval`    | the agent holds a recommendation and wants it confirmed                          |
| `environment` | the checkout or machine cannot run the work — missing deps, a restart, a TTY     |
| `dependency`  | another task or project must land first                                          |
| `session`     | the session ended before the work did — context exhausted, time, crash           |

- **Parsing.** A `Reason` enum in `claims.rs` beside `WaitingOn`, with `ALL`, `parse`,
  and `as_str`, `serde(rename_all = "lowercase")`. A value outside the six is a
  validation error listing them, in `WaitingOn::parse`'s shape. The flag is optional and
  has no default.
- **Independence.** Any reason combines with either `--waiting-on`. `--reason approval
  --waiting-on agent` is odd but not refused; the vocabulary describes the stop, the
  readers act on who, and tasks does not police the pairing.
- **Store.** `Park` gains `reason: Option<Reason>`, serialized with
  `skip_serializing_if = "Option::is_none"` and `default` on read, so existing store
  files load unchanged and a park without a reason writes no key. Re-parking replaces
  the whole entry, reason included; a re-park without `--reason` records none.
- **Note.** The park note becomes `parked (waiting on <who>, <reason>): <next step>`
  when a reason is given and stays `parked (waiting on <who>): <next step>` when not.
  The note is the durable trail (park design §2.1); no park frontmatter is added.
- **Rows.** `ParkedRow` gains `reason` (`null` when absent), so `prime`, `list --parked`,
  and `next` carry it. The pretty parked listings print it after the who, in the same
  parenthetical the note uses.
- **Completion.** `complete::reason` offers the six values;
  `ArgValueCandidates` on the flag, as for `--waiting-on`.
- **Rename.** Park entries migrate through `tasks rename` byte-for-byte as today; the
  new key rides along.

## 5. Docs

`skills/tasks/SKILL.md` step 5 gains the flag and the six words in one sentence, with the
instruction to record a reason when one of them fits and to leave it off otherwise — a
park is frequent and must stay cheap. The README's park section gains the vocabulary
table, and its record-format section lists `started` and `completed` beside `last_done`.

## 6. Testing

Units in `format.rs` mirroring `last_done`: round-trip of both stamps, the write order,
`quote_timestamps` covering them, the parse-time refusal of `completed` on an open task,
and of an unparsable value; a `check` finding for `completed` on an open record. Integration cases that carry the contract:

- `start` stamps `started`; a second `start` after `park`, a `--force` takeover from
  another session, and `edit --status doing` all leave the original value.
- `done` stamps `completed`; `edit --status todo` (reopen) clears it and leaves
  `started`; a fresh `done` restamps with a later value.
- A recurring task: `done` stamps `completed` and `last_done` to the same instant;
  `start` on the due occurrence clears `completed` and keeps `last_done`; the retry
  path with a lingering claim (the provenance test's arrangement) stamps neither.
- An editor save that sets, moves, or clears either stamp is refused with the named error.
- `park --reason review` writes the entry, the row (`prime`, `list --parked`, `next`
  JSON), and the note in the parenthetical form; `park` without the flag writes `null`
  and the old note form; `--reason nope` fails listing the six; re-park without the flag
  drops a previous reason.
- A store file written before this change (no `reason` key) loads.
- JSON for `show`, `list`, and `prime` carries `started` and `completed`; pretty `show`
  prints them; tables are unchanged.
