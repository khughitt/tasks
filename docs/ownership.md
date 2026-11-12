# Work ownership

Type: protocol; document types 1.
The claim/park protocol is currently unversioned; its optional relay input accepts
registry schema 2. This defines ownership across worktrees and links the
[task model](task-model.md) and [project recovery guide](project-operations.md).

## Participants and state

The operator/session chooses work, the CLI validates it under a per-project
mutation lock, and the host-local claim store shares ownership across worktrees.
Relay optionally supplies concrete harness process identity. Git records keep
advisory `owner` and lifecycle notes; claims, parks and complexity escalations
live outside git under `$XDG_STATE_HOME/tasks` or `$HOME/.local/state/tasks`.
These stores are local to one host and are not a distributed lock.

`tasks show <id>`, `tasks claims` and `tasks prime` expose current claims and
continuations. A claim names its session, owner, host, worktree, started/seen
stamps, and any PID/start/boot/PID-view/root evidence. A root association is
descriptive; it grants no authority over a worker's claim.

## Acquire, refresh and release

`tasks start <id>` acquires/resumes a claim, records doing/owner and a started or
resumed note, and clears that host's park. Repeated starts by the owner refresh
the claim. Guarded operations — lifecycle and status transitions (`start`, `done`,
`drop`, `park`, `block`, `unblock`, `shelve`, `unshelve`, `edit --status`), `edit`s
that change `needs` or open `$EDITOR` / `--editor`, `archive-notes --expect`, and
`retained` lifecycle writes (`checkpoint`, `transfer`, `reconcile`) — refuse a
live or unverifiable foreign claim with `claimed`. Append-style and ordinary
metadata writes (`note`, `attach`, `detach`, `dep`, and flag-based `edit`s that do
not touch `status` or `needs`) enforce `stale_copy` and record-write guards
without entering the foreign-claim refusal guard: they prune dead claims and
refresh or follow a live claim only when the caller already owns it, never making
a foreign session the owner. Claim and record writes can fail separately: inspect
the named landed paths before retrying.

Live or unverifiable foreign claims block work. `ready`, `next` and `prime` omit
held choices and explain why. Exclusive needs and lanes also block other
sessions' picks. A claim reserves the exclusive needs declared by that task's
project; names share one namespace across registered projects on the host.
An open halt in the registered checkout blocks lower-priority starts across
worktrees. An explicit `start --force --reason "<why>"` records an override;
it does not prove the displaced process stopped or make an unsafe action safe.

The owner uses `done`, `drop` or `park` to release. Parking does not change task
status. Its required next step is an action, with `--waiting-on user|agent` and
optional fixed-list reason: review, decision, approval, environment, dependency,
session, capability or quiet. A capability park can escalate complexity. A quiet
park requires `--minutes` and `--needs idle|headless`. `tasks quiet` reports those
resume recipes. A quiet attempt records:

```text
run: <actual> min (est <n>, <needs>); <phase> <m>, …; passed|failed|refused|hung|aborted[: <cause>]
```

Parks can outlive a close on another host. Prime warns and names the repeated
close that clears a closed park; a read never silently deletes it. Agent-waiting
parks are visible to every agent, not an exclusive handoff.

## Identity and liveness

Identity resolves as a pair from one level: explicit nonempty `TASKS_SESSION`
with optional `TASKS_SESSION_PID`; enabled relay; native Claude session with
optional CLAUDE_PID; native Codex thread (otherwise session) without a PID;
native agy conversation (`ANTIGRAVITY_CONVERSATION_ID`, stored as `agy:<id>`)
without a PID, only when no Claude or Codex variable is present; then Unix
session id. A missing PID is never borrowed from another level. Codex
commands can be their own short-lived session leaders, so the native Codex level
uses TTL rather than a command PID. Without usable process evidence a claim's
seen stamp remains live for four hours. Session id 0 is refused; set an explicit
session there. `TASKS_OWNER`, git branch, then USER chooses advisory owner.

With `[identity] relay = true`, Linux `/proc` ancestry selects the nearest
claude/codex/opencode/agy harness boundary (including versioned Claude binaries).
Daemon/pty hosts are not sessions. Registry schema 2 must match host, boot, PID,
process start and harness. Missing or conflicting evidence refuses acquisition.
The explicit session level bypasses relay for recovery and distinguishing agents.
It supplies no native note provenance; see [task model](task-model.md#origin-and-provenance).

Codex workers can share the controller process. Both native Codex values are
validated; a worker row is selected only by its own CODEX_THREAD_ID, with its
root stored separately. An unregistered worker is refused rather than assigned
the controller's row. Nested harnesses are separate sessions.

Exact resolved session equality authorizes continuation. Relay proof can also
continue an existing claim after the registry disappears, using the recorded
host/boot/start evidence and current nearest boundary. For Codex, proof also
requires a valid concrete CODEX_THREAD_ID matching the claim: sibling, absent,
malformed or session-only hints cannot substitute. At an agy boundary, relay
identity and proof-only continuation require a valid `ANTIGRAVITY_CONVERSATION_ID`.
Rows sharing the agy process are selected by it, and a conversation with no row is
refused rather than assigned another conversation's row. Parentage never authorizes
a foreign claim. A mismatched explicit TASKS_SESSION disables proof fallback.

Parking releases ownership, so the next start is a new acquisition and needs
current registry evidence or an explicit session. Legitimate owners recovering
from missing hints can inspect `show` and use its exact held session. Set a PID
only for a real long-lived process, never for the current CLI invocation. When a
harness continues one conversation under a new native session id (Claude Code
`/background` hand-off or `--resume` copy, or Codex `/new`, `/resume` or
`/fork` in one process), `tasks` receives no verified predecessor lineage and
guarded non-acquisition writes read the earlier claim as foreign. When that
refusal occurs in the claim's recorded worktree, the `claimed` detail appends a
conditional continuation hint (`tasks start --force <id> --reason "continued from <holder>"`
before retrying, redacting `sid:<pid>` holders as `Unix session`); refused
transitions to `doing` (`start` without `--force` and `edit --status doing`) omit
that continuation text. Confirm the claim's session is this conversation's
predecessor before running the forced takeover.

## PID views and takeover

A PID is judged only in the PID namespace view in which it was established.
An unknown, unreadable, or different view makes the claim `unverifiable`: it remains held and
is neither pruned nor stripped of its resource holds. Exported PIDs establish a
view only in the initial namespace; a number inherited into a container cannot
prove its origin. Relay matches also need hooks and tasks to share the PID view.
A nearest harness at PID 1 with no parent produces a namespace diagnosis.
Use the exact TASKS_SESSION without TASKS_SESSION_PID in that situation, or run
with the approved host PID view. A locally gone/zombie/zero PID is not recorded
as fresh proof; the claim uses TTL instead where supported.

An owner release, reboot evidence, or forced start can clear an unverifiable
claim. When the operator knows a foreign holder has stopped,
`tasks release <id> "<why>"` removes its stale/unverifiable claim and records a
single explanatory note without changing status. It refuses a live claim and
refuses this session's own claim (use park/done). Foreign `unverifiable` `claimed`
refusals (including refused `start` without `--force` and `edit --status doing`)
append a conditional `tasks release <id> "<why>"` hint; `live` and caller-owned
claims never suggest `release`, and unresolved relay identity errors take
precedence over `claimed`. In the stale-copy check,
`release` skips the target's non-live claim and, for non-`sid:<pid>` sessions,
that session's other non-live claims in the same worktree. During prefix
adoption use the refusal's exact `release ... --adopting <old>` command for a
foreign claim in the old store.
Do not release a worker based on silence, timeout or a shared PID; establish
worker-specific completion before forced controller takeover.

## Failure and recovery

`stale_copy` means another checkout has a newer record or conflicting bytes at
the same stamp. A parked worktree with no live or unverifiable foreign claim
offers the `-C` retry. When another session actively works in the newer
checkout, non-note writes withhold `-C` while `tasks note` (excluding feedback
recurrence and same-second forks) still names the occupant and appends an
in-place `tasks -C <wt> note ...` alternative. Follow the remedy: wait or merge
conflicting history, or rerun in the named checkout when offered. Do not
overwrite the newer record or change the claim file to bypass the guard. The
comparison is local across known checkouts, not proof that every host has
synced.

Missing-record discovery has a separate remedy: with a unique stable file,
complete search and unchanged shorthand context, `note` can offer an explicit
append into a known occupied records home. Other occupied writes and every
unknown occupancy offer inspection only. Linked code claims map to the
registered overlay they write to, not all overlay branches. These hints never
route a write or move a claim; explicit retries run normal guards. The older
claim/park missing-record path retains its strict occupied-write remedy.

`claimed`, `need_held`, halt and identity errors retain the held state. Recheck
the named task/session and supply an evidence-based override only when warranted.
Failure to read identity on a read view owns no holds; it does not free them.
Pending move/rename/migration and retired-project guards are independent of claims;
follow [project recovery](project-operations.md), not forced claim edits.

Upgrade every tasks binary that accesses a host's stores before enabling schema-2
relay and PID-view claims. Older binaries can drop proof fields or prune a claim
from the wrong view. Version output includes a build commit when known, but does
not mark uncommitted source changes. Neither local locks nor this protocol offer
cross-host exclusion or power-loss durability guarantees.
