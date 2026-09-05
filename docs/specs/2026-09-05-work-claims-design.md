# Work claims: cross-worktree visibility, agent identity, and liveness

Status: designed, not implemented (2026-09-05)
Tasks: tasks-d184e3 (claims), tasks-8f4b41 (worktree divergence)

## Problem

A `doing` claim is file content on a branch. Three consequences:

1. **Invisible across worktrees.** `tasks prime` in one checkout shows `todo` for a task
   another worktree holds as `doing`, because the other checkout's edit lives on its own
   branch (tasks-d184e3).
2. **No identity or liveness.** `start` records the git branch as `owner`, so two agents on
   one branch are indistinguishable, and a `doing` task abandoned by a dead session looks
   exactly like one under active work. Observed: two agent sessions began executing the same
   plan in the same worktree, neither noticing the other (tasks-d184e3).
3. **Divergent copies.** `start` in a checkout, then `git worktree add`, leaves an
   uncommitted `doing`/`owner` edit on one side and a pristine `todo` on the other; a later
   merge conflicts (tasks-8f4b41).

## Approach

Move the *claim* out of git into a per-project store, keyed by project prefix. The task file
keeps `status: doing` and `owner` exactly as today — the claim store is an overlay that adds
identity, liveness, and cross-worktree visibility without changing the file format.

**Keyed by prefix, not root.** A worktree and its main checkout have different
`Project::root` but the same prefix, and the registry already guarantees a prefix names
exactly one project. The store is therefore shared across every worktree with no worktree
enumeration anywhere.

## Store

`$XDG_STATE_HOME/tasks/claims/<prefix>.toml`, falling back to `$HOME/.local/state/...`,
resolved the way `Registry::path` already resolves `XDG_CONFIG_HOME`. One record per
claimed task:

    [claims."tasks-8f4b41"]
    owner     = "feat/doing-claims"   # unchanged: TASKS_OWNER / branch / USER
    session   = "887b2161-..."        # who, distinctly from the branch
    pid       = 3247245
    pid_start = 15727737              # /proc/<pid>/stat field 22, clock ticks since boot
    boot_id   = "427173f8-..."        # /proc/sys/kernel/random/boot_id
    host      = "..."
    worktree  = "/.../.worktrees/doing-claims"
    started   = "2026-09-05T08:32:43Z"
    seen      = "2026-09-05T09:01:12Z"

State, not configuration: it is per-machine, disposable, and losing it costs only the
overlay, never a task.

### Locking

The lock is not a claim-store lock; it is a **per-project mutation lock**, and it covers the
task file and the claim store together.

Two reasons it has to be that wide. First, checking ownership in `transition()` and writing
in `save()` are separate steps, so a lock spanning only the claim store leaves a takeover
race in the gap between them: B could take the claim after A's guard passed and before A's
file write landed. Second, `note` is not conflict-free at the storage layer — it reads and
rewrites the whole markdown file, so a foreign note racing a status change silently
overwrites it. Both are fixed by the same lock, and only by one that spans guard, task-file
persistence, and claim mutation as a single critical section.

A persistent `<prefix>.lock` sits beside the TOML, opened and exclusively locked with
`std::fs::File::lock`. Available on the project's toolchain (rustc 1.98.1); no new
dependency. It is acquired in `open_ctx()` and held in `Ctx` for the life of the command, so
every write path — `note`, `start`, `done`, `drop`, `block`, `edit`, `dep` — is serialized
without each having to remember to take it. `ReadCtx` takes no lock.

The lock is a separate file *because* `atomic_write` replaces the TOML's inode — locking the
TOML itself would leave each writer holding a lock on a file no longer at that path. A
process that dies holding the lock releases it to the kernel, so there is no stale-lock
recovery path to write.

Two exceptions, both deliberate:

- **The interactive editor** would otherwise hold the lock for as long as a human keeps
  `$EDITOR` open. It drops the lock around the editor invocation and re-acquires it before
  validating and writing. The gap is already covered by the raw-content comparison at
  `edit.rs:134`, which fails with `concurrent_modification` and keeps the edit.
- **`add` and `feedback` take no lock at all.** `create_task` links the file into place
  exclusively, so it has no read-modify-write to protect. This also means no command ever
  holds two locks — `feedback` writes into another project — so there is no lock-ordering
  deadlock to reason about.

## Identity

`session` and `pid` are resolved as a **pair from a single level**, never mixed across
levels. A level that yields a session but no usable pid yields `pid = None`, and the claim
falls to the TTL path; it is never paired with an unrelated fallback pid.

1. `TASKS_SESSION` / `TASKS_SESSION_PID` — explicit, harness-agnostic.
2. `CLAUDE_CODE_SESSION_ID` / `CLAUDE_PID` — both documented in Claude Code's environment
   reference. Recognised because `skills/tasks/SKILL.md` ships to agent projects, so agents
   are a first-class consumer and this makes the feature work with no setup. `CLAUDE_PID`
   requires a recent Claude Code; when it is absent, level 2 still supplies the session and
   `pid` is `None`.
3. The caller's Unix session id, from `/proc/self/stat`.

Level 3 is **terminal identity, not agent identity**: several agents can share one terminal
session, and a terminal outlives the agent that ran in it. It establishes *a* liveness handle
and distinguishes different terminals; it cannot guarantee distinct agent ownership. Where a
harness does not provide that distinction, level 1 remains necessary.

The same caveat applies within level 2. Subagents running inside one Claude Code process
share `CLAUDE_PID`, and probably `CLAUDE_CODE_SESSION_ID`; this was not verified. Setting
`TASKS_SESSION` per agent is the escape hatch, and the shipped skill should say so.

`owner` keeps its current meaning and its current three-level chain. Identity is added
alongside it, not in place of it.

## Liveness

    if claim.pid is None                    -> ttl(claim)
    if current boot_id unreadable           -> ttl(claim)
    if claim.boot_id != current boot_id     -> dead        (machine rebooted; the pair is meaningless)
    read /proc/<pid>/stat:
      not found                             -> dead
      unreadable (permission, no /proc)     -> ttl(claim)  (not proof of death)
      starttime != claim.pid_start          -> dead        (pid recycled)
      state == 'Z'                          -> dead        (zombie)
      otherwise                             -> live

    ttl(claim) = live if now - claim.seen <= 4h, else stale

A **confirmed live** process is live however long ago it was last seen: an agent that thinks
for six hours is not dead. A **confirmed dead** pid is stale at once, with no TTL grace. The
four-hour window applies only to claims whose liveness cannot be established.

`/proc/<pid>/stat` is parsed after the *last* `)`, because the `comm` field may contain
spaces and parentheses. In that remainder, state is field 1 and starttime is field 20.

On a platform without `/proc`, every read simply fails and the TTL path takes over; no `cfg`
gating is needed.

## Command behaviour

The guard belongs in `transition()` and `save()` in `commands/mod.rs`, not in the `start` /
`close` handlers. `edit --status` (`edit.rs:69`) and the interactive editor (`edit.rs:134`)
call `transition()` and `save()` directly and never reach `status.rs`; putting the guard at
the shared chokepoint covers all four paths at once.

- **Entering `doing`**: acquire. A live claim held by a different session refuses with a new
  `claimed` error kind naming owner, session, host, pid, worktree, and age. `--force` takes
  it over and appends a note recording the takeover. A stale claim is taken over without
  `--force`, but with a warning naming the displaced holder and why it was judged stale.
- **Any status change to a task under a live foreign claim** — `done`, `drop`, `block`,
  `edit --status`, an interactive edit — refuses with the same error. This is what stops a
  displaced session from closing work it no longer holds: after B takes A's claim, A's later
  `done` neither closes the task nor erases B's claim.

  The only way through is `tasks start --force <id>`, which makes the takeover explicit and
  recorded. Notably these commands do **not** grow a claim override of their own: `done
  --force` already means "close despite open dependencies or descendants", and `edit --force`
  already means "`--status done` despite the same". Overloading either with a second,
  unrelated meaning would make a single flag mean two different overrides. Takeover is a
  separate act, so it gets a separate command.
- **Release is destination-based, not origin-based.** Once the ownership check passes, a
  status change whose destination is anything other than `doing` releases this session's
  claim. It must not be conditioned on *leaving* a local `doing`: a session can hold the
  shared claim while its own checkout still reads `todo` — that is the ordinary
  cross-worktree case — and its `done` there would otherwise leave the claim behind forever.
- **`note`** refreshes `seen`, but only on a claim held by this session, and never touches a
  foreign claim. It is never refused. It is nonetheless serialized under the same lock as
  every other mutation: notes are append-only in *meaning*, but at the storage layer `note`
  rewrites the whole markdown file, so an unserialized foreign note can clobber a concurrent
  status change.
- **`ready` / `next`** omit every task under a live claim, **including this session's own**,
  and say so in the existing `warnings` array (and the pretty warnings block), so the
  omission is always explainable. `start` remains the authoritative check.
- **`prime`**'s `doing` section is *local status is `doing`* **or** *a live claim exists*, so
  a claim made in another worktree appears even where the local file still says `todo`.

### Write ordering

Under the lock there is no race left between these two writes; the ordering below is about
what a crash or an I/O failure leaves behind. Both orders fail toward "claim held", which is
the safe direction: a claim without a file update makes a task look busy when it is idle, and
it self-heals when the session dies. A file update without a claim is the invisibility bug
this design exists to remove.

- **Acquire**: claim first, then `save()` the task file. If `save()` fails, release the
  just-acquired claim best-effort and return the original error. If that release also fails,
  add a warning naming the orphaned claim and `tasks start --force` as the recovery.
- **Release**: `save()` the task file first, then release the claim. A failed release leaves
  a claim held by a session that is still alive, so liveness pruning will not reclaim it
  until that session ends; it warns, naming the task and `tasks start --force` as the
  immediate recovery.

### Pruning

A claim is removed by exactly two things: a **successful guarded release** by its owner, and
**liveness-based pruning** on the next write.

Local status never prunes. It is tempting to have a checkout that reads `done` drop the claim
as obviously obsolete, but that checkout's `done` may be the *older* state — another branch
can have reopened the task and claimed it since. One checkout's view of a task cannot
establish that a shared claim is obsolete. Local status may govern local presentation; it may
not delete. A live claim on a locally-closed task is therefore reported, not hidden and not
pruned.

## Warnings

- **Divergent copies** (tasks-8f4b41): a live claim whose task's *local* file disagrees
  (local `todo` or `idea`, claim says `doing`) warns that the copies will conflict on merge,
  naming the holding worktree. Unlike a worktree-count test, this fires in the order actually
  reported — `start` first, worktree created afterwards — because it triggers in the new
  worktree at the moment the disagreement becomes observable.
- **Uncommitted before branching**: `start` warns when it leaves the task file uncommitted in
  a repo that already has more than one worktree. Narrower than the above and only covers the
  reverse order, but cheap and correct there.
- **Stale claims**: `prime` warns for every stale claim, naming task and holder. Without this
  a stale claim on a locally-`todo` task would appear nowhere, since the `doing` predicate
  above requires a *live* claim.

## Known gaps

These are real and deliberately unaddressed; each is a candidate follow-up task.

- **The merge conflict itself is not fixed.** Claims restore visibility, so the second
  worktree learns about the first. The divergent `doing`/`owner` bytes still conflict.
  Eliminating them would mean not writing status to the tracked file at all — considered and
  rejected, because it removes `doing` from the file format and makes the historical record
  depend on a disposable store. What remains is a workflow remedy: commit the `start` before
  branching. The warnings above prompt for exactly that.
- **Release does not synchronise completion.** After `done` in worktree A releases the claim,
  a checkout still holding `todo` sees the task as ready again until A's commit merges.
  Claims prevent concurrent *ownership*; they say nothing about *completion*. A released-claim
  tombstone carrying `done` would close this, and is left for a follow-up.
- **No cross-machine visibility.** A Dropbox-synced checkout opened on another machine sees
  no claims: the store is per-machine by choice, and a pid from another host is unverifiable
  anyway. Adding it would mean an in-repo store found through the registry, plus host-aware
  liveness that degrades to TTL for foreign hosts.

## JSON contract

Additive only.

- `TaskSummary` and `ShowFields` gain `claim: Option<ClaimInfo>`.
- `ClaimInfo = { owner, session, host, pid, worktree, started, seen, live }`.
- New error kind `claimed`.
- `--pretty` marks doing rows with the holder.
- `prime --all-projects` loads one store per prefix in scope.

## Testing

Unit tests in `claims.rs`: store roundtrip; lock serialising two writers; pid-reuse rejection
via a mismatched `pid_start`; boot-id mismatch; zombie state; permission failure falling to
TTL rather than reading as dead; TTL boundary.

End-to-end in `tests/cli.rs`, using two `Project` roots that share one prefix and a temp
`XDG_STATE_HOME` to stand in for two worktrees:

- two simultaneous `start`s: exactly one wins
- concurrent writes to *different* claims: neither is lost
- the displaced owner's `done` after a `--force` takeover is refused, and B's claim survives
- `edit --status done` and an interactive edit are refused on a foreign-claimed task
- release on `done`, `drop`, `block`; `unblock` does not re-claim
- destination-based release: a session holding the claim while its own checkout still reads
  `todo` releases it on `done` — the claim does not survive
- a checkout whose local file reads `done` does not prune a live claim made from the other
  root
- a `note` racing a status change loses neither: both are serialized, and the interactive
  editor still reports `concurrent_modification` across its unlocked window
- `ready` and `next` omit claimed tasks and warn, including for the caller's own claim
- `prime` shows a claim made in the other root, and warns on a stale claim over a local `todo`
- the exact tasks-8f4b41 sequence: `start`, then create the second root, then observe the
  divergence warning
- acquire rollback: a failing `save()` leaves no claim behind
