# Halt tasks at the task entry path — design

**Status:** approved after review round 2, 2026-09-30. Task: tasks-c543ae. Contract: ops
`docs/specs/2026-09-29-test-latency-escalation-design.md` §6, approved for
ops-5beefd. This document settles its implementation in tasks.

## Outcome

An open task tagged `halt` stops new lower-priority starts in its project,
including starts from a worktree whose copy predates the halt. The normal
`prime`, `ready`, and `next` views name the halt. A person can override the stop
with a reason, recorded on both tasks. tasks knows only the tag and status; it
does not read test timings.

## Authority and predicate

When the project has a registry entry, its registered checkout supplies the
halt records, including uncommitted records. A worktree's own copy never
starts or lifts a halt in that case. When this host has no entry for the
project, the local checkout is the authority; fresh clones remain usable.
Use the existing registry and project scan in process, not a subprocess or a
new state file. The same predicate serves every command: a task with tag
`halt` whose status is neither `done` nor `dropped`. `idea`, `todo`, `doing`,
`blocked`, `shelved`, and deferred tasks all halt. Sort multiple halts by
priority and id for stable output. If an existing registry entry cannot be
opened, `start` fails before changing either the task or its claim. `--force`
does not waive that failure.

The write lock already held by `start` covers the authority read and the
local task transition. Read commands use one snapshot per project. If an
existing registry entry cannot be read, `prime`, `ready`, and `next` warn
that the halt state is unknown and continue showing their ordinary rows when
their local checkout is readable;
`start` still makes the final decision under its lock. Under
`--all-projects`, an unreachable registered project is skipped as today and
warned as having unknown halt state. Cross-host sync is eventual, as with
existing task records.

## Start rule

`tasks start <id>` keeps its current claim and transition checks. An already
`doing` task may resume. For a new start, allow the target if it is any open
halt task, reachable from any open halt through any sequence of subtask and
dependency links, or has priority at least as urgent (numerically no greater)
as the most urgent halt. This includes a subtask's dependency and a
dependency's subtask. Work toward **any** halt is allowed even when another,
more urgent halt stands. If none of these rules allows the target, it fails
with error kind `halted`, naming the blocking halts and
`tasks start <id> --force --reason "..."` as the override. This covers
`todo`, `blocked`, and a new occurrence of a completed recurring task.

The root of each allowed-work walk is the halt record from the authority
checkout. Other parent and dependency links and the target's priority come
from the caller's local checkout. This lets a worktree start a remedy subtask
it just filed, even before that record reaches the registered checkout. It
also means `edit --priority 0`, `edit --parent`, and `dep` can make work
eligible without a reason. Those are accepted bypasses of this first rule;
the halt and the start remain visible in the task records. A dependency in
another project remains subject to that project's own halt state. A target
missing from the local checkout keeps the existing not-found behavior:
start the halt task from its registered checkout if a worktree lacks it.

`--force` keeps its existing claim takeover meaning. Under a halt, any use of
`--force` needs a nonblank single-line `--reason`; a reason without `--force`
is invalid. For a blocked new start, the blocking halts are those more urgent
than the target (numerically lower priority). A successful override adds one
note to each blocking halt and one to the started task, each naming the other
task(s), the session and the reason. Their fixed prefix is `halt override:`:
halt notes say `halt override: attempted <target> by <session>: <reason>`;
the target note says `halt override: started past <halt ids> by <session>: <reason>`.
Write halt notes first, then save the target and claim with its
note; a failed later write may leave attempted-override notes, but never an
unaudited start. For `--force` on an already allowed task, put the reason in
its existing takeover note if there is one; otherwise warn that the reason
was unused. Apply that unused-reason rule when a halt lifts between a read
and `start` too, so a caller's prepared command still works.

The spelling `--reason` already appears on park, where it is an enum of park
reasons; start's `--reason` deliberately takes free text. Add its string
option to the start row in ops `cli.toml` **first**, then re-copy the changed
source to its vendored copies, including tasks `tools/cli.toml`, before
changing the parser. Start's option must not use park's `complete::reason`
completer. The `src/surface.rs` parser-surface test must match the revised
row. Add a typed `Halted` error and keep the existing JSON error envelope.
No new tag or configuration key is needed.

## Entry views

Add optional `halts` metadata to `prime`, `ready`, and `next` JSON: each row
has the halt task's id, title, owner, and priority. Pretty output prints a
`halt:` line before other sections, naming every halt and the allowed work.
An absent field means no halt, preserving unhalted output. The ready rows in
`prime`, `ready`, and `next` use one filter for lower-priority new starts and
warn once with the number hidden. The halt metadata remains visible even when its task is
already claimed, shelved, deferred, or absent from the caller's worktree.
Eligible halt tasks that are locally present retain their ordinary ready
ordering. A worktree whose copy lacks the halt still sees it in metadata;
the halt line says to start it from the registered checkout. Lifecycle
commands keep their existing routing, so `start`, `note`, `park`, and `done`
do not split one task across two checkouts. If filtering leaves no local
candidate, `next` returns `next: null` alongside `halts` and the hidden-count
warning.

## Verification

Integration tests use two scratch registered projects and a linked worktree.
They cover every open status, done/dropped and deferred records, registered
and unregistered projects, unreadable registered checkouts on writes and
reads, multiple halts, mixed subtask/dependency paths, local-only remedy
subtasks, accepted edit bypasses, priority, resumed claims, blocked and
recurring new starts, a forced override's notes, failed or missing reasons,
and an unused reason. Two cross-worktree assertions are explicit: an
uncommitted incident in the registered checkout refuses a `start` from an
existing linked worktree at once; a halt closed in a worktree does not lift
until the registered checkout's record is closed. Read tests assert JSON
metadata and pretty `halt:` position, a `next: null` when all local choices
are hidden, and the single hidden-count warning. `--all-projects` tests show
that each project's halts affect only its own rows. The targeted tests run
through `just test-one`, then `just test-fast` and `just check` before commit.

## Boundaries

This rule does not cancel active claims or require closing a halt before
existing work finishes. It does not change ordinary `list` or `show`, which
remain record queries. A separate record-home design is active in tasks;
implementation will recheck its landing before using the current routing
functions, while preserving the registered checkout as halt authority when
that checkout exists.
