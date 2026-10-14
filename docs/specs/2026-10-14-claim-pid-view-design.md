# Claim liveness judged only from the PID view that recorded it

Task: tasks-860db2. Status: draft for review. Source: tasks-fe6c59, whose attached
`pidns-proof-evidence.json` holds the cases cited below.

## Outcome and scope

A claim carrying a pid is judged by reading `/proc/<pid>/stat`. Today that read goes
through the evaluator's own `/proc`, whatever view the pid came from. Two failures
follow:

- **Sandbox prunes host claims.** A command in a child PID namespace with a private
  `/proc` (the Codex workspace-write sandbox shape) sees a live host claim as "pid
  gone". Every claim write calls `ClaimStore::prune_dead`, so any write there deletes
  that claim with no warning and no takeover note. The record stays `doing`, and the
  next session starts it without `--force` (cases `prune-*`, `codex-host-after`).
- **Host takes over namespace claims.** A pid recorded inside a namespace
  (`TASKS_SESSION_PID=1`) reads as "reused" from the host, which then takes the claim
  over without `--force` (cases `ns-explicit-pid1`, `host-view-of-ns-pid`).

After this change a claim records the view its pid was read through. An evaluator
in another view, or one that cannot tell, gives a third verdict, `Unverifiable`.
That verdict protects the claim as `Live` does: nothing prunes it, takes it over
without `--force`, or releases its holds. Pid-less (TTL) claims are unaffected.

In scope: the claim field, the verdict, every call site's treatment of it, the
`tasks claims`/`ClaimInfo` JSON, and the README paragraph on PID namespaces.
Out of scope: relay matching and process proof, which already fail closed across
namespaces (tasks-fe6c59); the refusal wording, which is tasks-19b244.

## Evidence checked

On 2026-10-14, against the worktree base `f23487b`:

- `claims::liveness_with` (src/claims.rs) checks the pid, boot and start time with no
  notion of view. `ClaimStore::prune_dead` keeps only `Live`. The claim write in
  `commands/mod.rs` records `pid_start` from the relay proof, or else by reading
  `proc_stat(pid)` in the writer's own `/proc`.
- About 30 call sites read `Liveness`. Most ask `== Live` (protect only `Live`); some
  ask `!= Live` (act on anything else): `holds.rs` releasing holds, `update_holds`,
  and `claimed_elsewhere` in `commands/list.rs`.
- ops `hooks/claim-guard` requires `live` to be a bool in `tasks claims`, and treats
  `live: true` rows as held.
- `StoreFile` and `Claim` do not deny unknown fields, so an older binary loads a store
  with a new field. It drops that field when it saves.
- This host has three claims across all stores; one carries a pid.

PID-view probe on this host (kernel 7.2.2, unprivileged `unshare`):

| caller | `/proc` `st_dev` | `NSpid` |
|---|---|---|
| host | 24 | one entry |
| child PID namespace, inherited host `/proc` | 24 | two entries |
| child PID namespace, private `/proc` | 38 | one entry |

## Design

### 1. What a PID view is

Linux numbers `/proc/<pid>` by the PID namespace of the procfs mount, not by the
caller's namespace. Case `ns-plain-host-proc` shows a child-namespace caller reading
host pids through an inherited `/proc`. A pid therefore means something only together
with the procfs it was read through.

**A PID view is the `st_dev` of `/proc`**, read with `std::fs::metadata("/proc")`.
Each procfs superblock has its own device number. An inherited host `/proc` shares
the host's device, so it is the same view, which is correct because it numbers pids
the same way. A private `/proc` gets a new device. Two procfs mounts of one namespace
would read as different views. That errs toward `Unverifiable`, the protective
verdict, never toward a wrong `Stale`.

Rejected alternatives:

- **`/proc/self/ns/pid`** names the caller's namespace, not the procfs's. It would
  misjudge the inherited-host-`/proc` case.
- **The `NSpid` entry count**, the task's candidate signal. One entry means the
  procfs numbers pids in the caller's namespace, but the host and a private sandbox
  `/proc` both show one entry (table above). It cannot tell them apart.
- **The procfs's pid-namespace inode**, from the `NSpid` count plus `/proc/self/ns/pid`,
  or `/proc/1/ns/pid` for an ancestor. Reading `/proc/1/ns/pid` on the host's init
  needs ptrace access that an unprivileged sandbox lacks, and `NS_GET_PARENT` returns
  `EPERM` outside the caller's scope. The inherited-`/proc` case would always read
  unknown.

**Reuse.** A device number, like a namespace inode, is reused after its procfs is
gone. A reused view is not evidence of anything. Its old processes are gone, so a
recorded pid there reads as absent or reused (`Stale`, correctly). A false match
would need the same pid and the same start time, which the existing `pid_start`
check already rules out. A claim with a pid but no `pid_start` keeps today's
behaviour within its view.

### 2. Recording

`Claim` gains `pid_view: Option<u64>`, serialized only when present.

- Whenever a claim write records `pid`, it also records the writer's current view.
  On the native path the writer has just read `proc_stat(pid)` through that view. On
  the relay path, the match was made by walking the writer's own `/proc` ancestry, so
  the handle's pid is valid in the writer's view too.
- A pid-less claim records no view.
- If `/proc`'s metadata cannot be read, the field is absent. The claim is then
  `Unverifiable` everywhere (§3), which protects it and does not misjudge it.
- Every write by the owner refreshes `seen` and rewrites the claim, so the view
  follows the owner's latest write.

### 3. The verdict

```rust
pub enum Liveness {
    Live,
    Stale(String),
    Unverifiable(String),
}
```

`liveness_with` gains an injected current view, `Option<u64>`, beside the injected
`stat`. For a claim with a pid it decides in this order:

1. A known, differing `boot_id` gives `Stale("recorded on an earlier boot")`. A
   reboot ends every view, so this needs no view check and stays first.
2. If the recorded view is absent, the current view unreadable, or the two differ,
   the verdict is `Unverifiable`. The reason names which case applies: "recorded in
   another PID view", "recorded before PID views were tracked", or "this command
   cannot read its PID view". The reason carries no pid or device values.
3. Otherwise the checks run as today: unknown boot, the stat result, the zombie test,
   the start time and the TTL fallback.

A pid-less claim takes the TTL path exactly as today.

`Unverifiable` never decays into `Stale` with age. A live owner running one long
command keeps its claim past four hours, even against a sandboxed caller (the test
in §6).

### 4. Call sites

The rule: **only `Stale` permits automatic action.** `Unverifiable` gets `Live`'s
protection and its own wording. A helper, `Liveness::is_stale()`, replaces the `== Live` /
`!= Live` comparisons, so each site states its intent.

| Site | Today | With `Unverifiable` |
|---|---|---|
| `ClaimStore::prune_dead` | keeps `Live` | prunes only `Stale` |
| `ClaimSnapshot::live` (ready, next, lanes, sample, show, move preflight, prime's doing list) | `Live` only | renamed `held`; returns `Live` and `Unverifiable` |
| `ClaimSnapshot::stale` and its warnings | `Stale` | unchanged |
| `claimed_elsewhere` (list.rs) | skips non-`Live` | skips only `Stale` |
| `holds.rs` hold snapshot | releases non-`Live` | releases only `Stale` |
| `refuse_foreign_live_claim`, the transition guard, `park` takeover | refuse on `Live` | refuse on `Live` or `Unverifiable`; `--force` takes over |
| `update_holds` | returns early unless `Live` | returns early only on `Stale` |
| `rename`, `rename --adopt`, `retire` | refuse on a live claim | also refuse on `Unverifiable` |
| `takeover_summary` | `live, forced` / `stale` | adds `unverifiable, forced` |
| `describe_claim` | `live` / `stale: why` | adds `unverifiable: why` |

**Who removes an `Unverifiable` claim:**

- The recording view, where the verdict is ordinary `Live` or `Stale`, so its writes
  prune the claim when the owner is dead.
- A reboot.
- The owner's own `park` or `done`.
- An explicit `start --force`, recorded as `unverifiable, forced`.

A claim whose recording view has itself disappeared, such as a finished sandbox,
therefore stays until reboot or `--force`. Wrongly deleting a live claim costs more
than that.

No new warnings. Inside a sandbox every host pid claim reads `Unverifiable`, so a
warning per claim would be noise on every command. `tasks claims` and `show` carry
the reason.

### 5. JSON

`ClaimInfo` keeps `live: bool`, defined as "held": `true` for `Live` and
`Unverifiable`. It gains an optional `unverified: "<reason>"`, present only for
`Unverifiable`. Consumers that read `live: false` as "may be taken", ops `claim-guard`
among them, stay safe without a change. The rejected alternative, `live: false`
plus the new field, would tell every older consumer that a protected claim is
free. The addition is additive. The README documents both fields where it lists
`tasks claims`, which has no field-level contract today.

### 6. Decisions on the task's remaining questions

- **Refusing `TASKS_SESSION_PID` outside a common numbering:** no. No evaluator can
  know another view's numbering. With the view recorded, a namespace-local pid reads
  `Unverifiable` from the host, which closes `host-view-of-ns-pid` without a refusal.
- **A relay walk that ends at a pid 1 with ppid 0 in a non-initial namespace:** no
  change. With a plain sandbox helper the walk ends Outside, and the native Codex
  level records a pid-less TTL claim (`ns-plain-private-proc`). That claim does not
  depend on any view. Its one harm was the prune, which §4 removes. The Codex-helper
  shape already refuses; tasks-19b244 names the cause.
- **Legacy claims (a pid and no view):** `Unverifiable` under §3 rule 2, in every
  view. Only the owner's next write or `--force` clears one. One such claim exists
  on this host today.

### 7. Compatibility

The store change is additive. An older binary loads it and drops `pid_view` on
save, which turns that claim into a legacy claim: protected, not misjudged. Claims
are host-local, so the skew lasts only while one host runs two binaries. The JSON
change is the optional `unverified` field.

## Testing

Unit tests on `liveness_with`, injecting the view as the tests already inject `stat`:

- Same view: live, gone, zombie and reused give today's verdicts.
- Different view: a claim that would read gone or reused gives `Unverifiable`.
- Different view, `seen` older than four hours: `Unverifiable`, not `Stale`.
- Recorded view absent with a pid: `Unverifiable`. Current view unreadable:
  `Unverifiable`.
- Known boot mismatch: `Stale` whatever the view.
- Pid-less claim: TTL as today, whatever the view.

Call-site tests:

- `prune_with`/`prune_dead` keeps an `Unverifiable` claim.
- The hold snapshot keeps its holds.
- `ClaimInfo` gives `live: true` and `unverified`.
- `takeover_summary` and `describe_claim` wording.

Integration tests in `tests/cli.rs`, with the store written as fixtures so no
environment backdoor is needed. A claim carries the test process's pid and start
time and a `pid_view` that cannot match.

- An unrelated `start` leaves that claim in place.
- Another session's `start` refuses `claimed`.
- `start --force --reason` takes it over with an `unverifiable, forced` note.
- `tasks claims` reports it as `live: true` with `unverified`.

Acceptance: rerun the tasks-fe6c59 probe's unshare cases `prune-*`,
`ns-explicit-pid1` and `host-view-of-ns-pid` against the worktree build. Expect the
host claim to survive the sandboxed write. Expect the host to refuse the takeover
without `--force`. The genuine `codex sandbox` cases are optional and need the
installed Codex CLI with no model call.

## Documentation

- The README paragraph on PID namespaces: claims record their PID view, and other
  views protect but do not judge.
- The README's `tasks claims` entry: `live` means held, and `unverified` gives the
  reason a held claim could not be judged.
- The tasks skill, in step: one sentence in the claim paragraph saying that a claim
  another PID view recorded is held until its own view, a reboot or `--force`
  releases it.
