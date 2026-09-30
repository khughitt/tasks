# Halt tasks at the task entry path — design

**Status:** draft for review, 2026-09-30. Task: tasks-c543ae. Contract: ops
`docs/specs/2026-09-29-test-latency-escalation-design.md` §6, approved for
ops-5beefd. This document settles its implementation in tasks.

## Outcome

An open task tagged `halt` stops new lower-priority starts in its project,
including starts from a worktree whose copy predates the halt. The normal
`prime`, `ready`, and `next` views name the halt. A person can override the stop
with a reason, recorded on both tasks. tasks knows only the tag and status; it
does not read test timings.

## Authority and predicate

The project's registered checkout supplies the halt records, including
uncommitted records. A worktree's own copy never starts or lifts a halt. Use
the existing registry and project scan in process, not a subprocess or a new
state file. The same predicate serves every command: a task with tag `halt`
whose status is neither `done` nor `dropped`. `idea`, `todo`, `doing`, `blocked`,
`shelved`, and deferred tasks all halt. Sort multiple halts by priority and id
for stable output. If the registered checkout cannot be read, `start` fails
before changing either the task or its claim.

The write lock already held by `start` covers the registered read and the
local task transition. Read commands use one snapshot per project,
as they do for ordinary task listings; a concurrent edit may change what they
show, but `start` makes the final decision under its lock. Cross-host sync is
eventual, as with existing task records.

## Start rule

`tasks start <id>` keeps its current claim and transition checks. An already
`doing` task may resume. For a new start, allow the target if it is any open
halt task, a descendant or dependency of any open halt task, or has priority
at least as urgent (numerically no greater) as the most urgent halt. Follow
dependency edges transitively so a prerequisite of a prerequisite remains
startable. Other new starts fail with error kind `halted`, naming the halt task
and `tasks start <id> --force --reason "..."` as the override. This covers
`todo`, `blocked`, and a new occurrence of a completed recurring task.

`--force` keeps its existing claim takeover meaning. Under a halt, any use of
`--force` needs a nonblank single-line `--reason`; a reason without `--force`
is invalid. A successful override of a blocked start adds one note to each
blocking halt and one to the started task, each naming the other task(s), the
session and the reason. Write halt notes first, then save the target and claim
with its note; a failed later write may leave attempted-override notes, but
never an unaudited start. A failed registered read is never overridden by
`--force`.

The CLI already has the spelling `--reason` for park in the shared vocabulary,
so this adds it to start without adding a new vocabulary term. Add a typed
`Halted` error and keep the existing JSON error envelope. No new tag or
configuration key is needed.

## Entry views

Add optional `halts` metadata to `prime`, `ready`, and `next` JSON: each row
has the halt task's id, title, owner, and priority. Pretty output prints a
`halt:` line before other sections, naming every halt and the allowed work.
An absent field means no halt, preserving unhalted output. The ready rows in
`prime`, `ready`, and `next` use one filter for lower-priority new starts and
warn once with the number hidden. The halt metadata remains visible even when its task is
already claimed, shelved, deferred, or absent from the caller's worktree.
Eligible halt tasks that are locally present retain their ordinary ready
ordering. A worktree whose copy lacks the halt still sees it in metadata and
can start it by id from the registered checkout: `start` falls back to that
checkout only when the missing local id is itself an active halt. Other ids
keep current routing.

## Verification

Integration tests use two scratch registered projects and a linked worktree.
They cover every open status, done/dropped and deferred records, cross-worktree
reads, an unreadable registered checkout, multiple halts, descendants and
transitive dependencies, priority, resumed claims, blocked and recurring new
starts, a forced override's two notes, and failed or missing reasons. Read
tests assert JSON metadata and pretty `halt:` position, plus the single
hidden-count warning. The targeted tests run through `just test-one`, then
`just test-fast` and `just check` before commit.

## Boundaries

This rule does not cancel active claims or require closing a halt before
existing work finishes. It does not change ordinary `list` or `show`, which
remain record queries. A separate record-home design is active in tasks;
implementation will recheck its landing before using the current routing
functions, while preserving the registered checkout as halt authority.
