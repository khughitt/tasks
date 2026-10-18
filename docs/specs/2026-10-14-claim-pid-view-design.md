# Claim liveness judged only from the PID view that recorded it

Task: tasks-860db2. Status: accepted 2026-10-14 at revision 3 (spec round 3, reviewer
human); implementation requires a separately reviewed plan. Revision 1 failed review
(spec round 1): it identified a view by procfs `st_dev`, it recorded the writer's view
for exported PIDs it had not established, and it called mixed versions safe.
Revision 2 failed spec round 2: it took ancestry membership as proof of an exported
pid's namespace.
Source: tasks-fe6c59, whose attached `pidns-proof-evidence.json` holds the cases cited
below.

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

After this change a claim records the view its pid was established in, when the
writer can establish one. An evaluator in another view, or one where either side is
unknown, gives a third verdict, `Unverifiable`. That verdict protects the claim as
`Live` does: nothing prunes it, takes it over without `--force`, or releases its
holds. Pid-less (TTL) claims are unaffected.

In scope:
- the claim field and the rules for writing it;
- the new verdict and how every call site treats it;
- the `tasks claims`/`ClaimInfo` JSON;
- the README paragraphs on PID namespaces and upgrading.

Out of scope: relay matching and process proof, which already fail closed across
namespaces (tasks-fe6c59); the refusal wording, which is tasks-19b244.

## Evidence checked

On 2026-10-14, against the worktree base `f23487b`:

- `claims::liveness_with` (src/claims.rs) checks the pid, boot and start time with no
  notion of view. `ClaimStore::prune_dead` keeps only `Live`. The claim write in
  `commands/mod.rs` records `pid_start` from the relay proof, or else by reading
  `proc_stat(pid)` in the writer's own `/proc`. It accepts the pid even when that read
  fails, and then records no start time.
- About 30 call sites read `Liveness`. Most ask `== Live` (protect only `Live`); some
  ask `!= Live` (act on anything else): `holds.rs` releasing holds, `update_holds`,
  and `claimed_elsewhere` in `commands/list.rs`.
- ops `hooks/claim-guard` requires `live` to be a bool in `tasks claims`, and treats
  `live: true` rows as held.
- `StoreFile` and `Claim` do not deny unknown fields, so an older binary loads a store
  with a new field. Before it saves, its own `prune_dead` judges every pid through its
  own `/proc`, and it drops the new field when it saves.
- `relay::ancestry` already walks the caller's ppid chain through `/proc`.
- This host has three claims across all stores; one carries a pid.

PID-view probes on this host (kernel 7.2.2, unprivileged `unshare`):

| caller | `/proc` `st_dev` | `NSpid` entries | `/proc/self/ns/pid` | `/proc/1/ns/pid` |
|---|---|---|---|---|
| host | 24 | 1 | dev 5, ino 4026531836 | n/a |
| child namespace, inherited host `/proc` | 24 | 2 | readable | permission denied |
| child namespace, private `/proc` | 38 | 1 | readable, its own inode | n/a |

Reviewer probes (spec round 1):

- A procfs device number (38) was reused by another namespace while the first
  namespace's pid 1 was still alive.
- A live host pid exported into a private-`/proc` child was absent from the child's
  `/proc`.

## Design

### 1. What a PID view is

Linux numbers `/proc/<pid>` by the PID namespace of the procfs mount, not by the
caller's namespace. Case `ns-plain-host-proc` shows a child-namespace caller reading
host pids through an inherited `/proc`. A pid therefore means something only together
with the namespace its procfs numbers in.

**A PID view is the PID namespace that the caller's `/proc` numbers pids in,
identified by the `(st_dev, st_ino)` of its nsfs file.** namespaces(7) names that pair
as a namespace's identity. Reading the current view:

1. Read `/proc/self/status` and count the `NSpid` entries. The kernel lists them from
   the procfs's namespace down to the caller's. One entry means the procfs numbers in
   the caller's own namespace. A caller cannot see itself in a procfs of an unrelated
   or descendant namespace (`/proc/self` does not resolve there), so any failed read
   means the view is unknown.
2. If there is exactly one entry, the view is the `(st_dev, st_ino)` of
   `stat("/proc/self/ns/pid")`, following the link.
3. Otherwise, or if any read fails, the view is **unknown**. With two or more entries
   the procfs belongs to an ancestor namespace. Naming that namespace would mean
   reading `/proc/1/ns/pid`, which an unprivileged caller is refused (table above).
   `NS_GET_PARENT` returns `EPERM` outside the caller's scope.

**Why this identity cannot be reused under a live owner.** A namespace's inode
number is freed only when the namespace itself is destroyed. A process in a namespace,
or in any descendant namespace, holds the namespace alive. The recorded pid was valid
in the view's procfs, so its process lived in that namespace or below it. While that
process lives, no other namespace can carry the same identity. If a view's identity
turns up again, the recording process is dead, and `Stale` is the right verdict.
Unmounting a procfs does not end its namespace, so the identity does not depend on
any mount.

Rejected alternatives:

- **The procfs `st_dev`** (revision 1). A device number is freed when the superblock
  goes, not when the namespace ends. The reviewer reproduced a reuse while the
  original namespace's pid 1 lived. A matching device then let a differing start time
  read as `Stale` and delete a live owner's claim.
- **`/proc/self/ns/pid` without the `NSpid` check.** It names the caller's namespace,
  not the procfs's, so it would misjudge an inherited host `/proc`.
- **The `NSpid` count alone.** The host and a private sandbox `/proc` both show one
  entry.

**Cost.** In a child namespace that keeps the host `/proc`, every view reads unknown.
Pid claims written there carry no view, and every pid claim evaluated there reads
`Unverifiable`. Nothing is misjudged; such claims clear only through §4's paths. The
Codex sandbox has a private `/proc`, so this cost does not reach it.

A second cost follows from §2. Outside the initial PID namespace, in a container or
sandbox, claims from exported pids (`CLAUDE_PID`, `TASKS_SESSION_PID`) record no view.
They stay protected after their session dies, until §4's paths clear them. Relay and
`sid:` claims still record a view there.

### 2. Recording

`Claim` gains `pid_view: Option<String>`, the identity formatted `"<dev>:<ino>"`,
serialized only when present. A claim write records a view only when it
**establishes** the pid in the writer's current view, and only when that view is
known:

- **Relay level.** The match is made by walking the writer's own `/proc` ancestry. The
  matched process is the writer's nearest harness boundary in that view, and its pid,
  start time and harness name all equal the registry row's. The claim then tracks
  that process, the writer's own session in this view. It does not depend on where
  the registry numbered the pid. A row from another namespace could match only with
  the same pid, start tick and harness name. Even then the tracked process is still
  the writer's own nearest harness.
- **Unix session fallback (`sid:<pid>`).** The session id comes from
  `/proc/self/stat`, which the kernel translates into the procfs's namespace. It is
  established when it is nonzero and `proc_stat` finds it.
- **Exported pids (`TASKS_SESSION_PID`, `CLAUDE_PID`).** An export can be inherited
  across a namespace boundary, so its number may belong to an outer namespace. That
  number can also exist in this view as an unrelated process, even one of the
  writer's ancestors. Spec round 2 reproduced this. Numeric matches, ancestry
  membership included, therefore establish nothing. An exported pid is established
  only when the writer's known view is the **initial PID namespace** and `proc_stat`
  finds the pid there:
  - A process's ancestors live in its own namespace or in ancestor namespaces, and the
    initial namespace has no ancestor. So any process that passed the value down
    through the environment numbered pids the same way.
  - The initial namespace is recognized by its nsfs inode, the kernel constant
    `PROC_PID_INIT_INO` (`0xEFFFFFFC`, 4026531836 in the evidence). If a kernel ever
    changed that constant, the check would fail toward no view, which is protective.
  - Anywhere else the claim keeps the pid, and the start time if one was read, and
    records no view.
  - Residual: a value carried into the initial namespace outside process inheritance,
    for example a namespace-local pid typed or sourced from a file, is outside this
    guarantee. Such a claim would be judged as a host pid. The README says
    `TASKS_SESSION_PID` must name a process in the namespace of the command that sets
    it.

A claim with a pid and no view reads `Unverifiable` everywhere (§3). It is protected,
not misjudged.

**Refuted pids record no pid.** Amended by tasks-f11052. A pid that the writer's own
view shows is not running would be held until reboot under that rule, since no view
can ever judge it. The write records a pid-less (TTL) claim instead, first write and
heartbeat alike. A pid is refuted when it is 0, when it is a Unix session id that
`proc_stat` finds gone or a zombie (the kernel numbered it in this procfs), or when it
is an exported pid gone or a zombie in the initial namespace, which numbers every
process. An exported pid missing from any other view may be an outer namespace's, so
its absence refutes nothing, and it keeps the rule above. The Unix fallback also
refuses session id 0 as an identity: the session leader is outside the namespace, and
every shell inside shares that id, so it cannot key ownership. `TASKS_SESSION` names
the session there.

**Heartbeats keep established evidence.** The owner's writes refresh `seen`. They
change the process evidence (`pid`, `pid_start`, `boot_id`, `pid_view`) only under
these rules:

| This write's pid | Evidence written |
|---|---|
| established in a known view | all four fields from this write |
| equal to the recorded pid, not established | the existing evidence, unchanged, unless it reads `Stale` here* |
| different from the recorded pid, not established | the new pid, any start read, no view |
| absent | the existing evidence, unchanged, unless it reads `Stale` here* |

\* Amended after the final implementation review (Important 1). Recorded evidence that
reads `Stale` from the writer's own view protects nothing. Keeping it would write the
owner's own claim stale, free for another session to take without `--force`. Such a
write records this write's evidence instead: for an absent pid, a pid-less TTL claim, as
before this design. Evidence the writer cannot judge (`Unverifiable`, the sandbox case)
is still kept.

The third row cannot keep the old evidence. That evidence names a process the owner
no longer reports, and once it dies its own view would read it `Stale` while the
session lives. Recording the new pid with no view keeps the claim protected.

A takeover or a fresh acquisition has no existing evidence of its own to keep, and
writes this write's evidence.

### 3. The verdict

```rust
pub enum Liveness {
    Live,
    Stale(String),
    Unverifiable(String),
}
```

`liveness_with` gains an injected current view, `Option<String>`, beside the injected
`stat`. For a claim with a pid it decides in this order:

1. A known, differing `boot_id` gives `Stale("recorded on an earlier boot")`. A
   reboot ends every view, so this needs no view check and stays first.
2. If the recorded view is absent, the current view unknown, or the two differ, the
   verdict is `Unverifiable`. The reason names the case: "recorded in another PID
   view", "PID view not established when recorded", or "this command cannot
   establish its PID view". The reason carries no pid or namespace values.
3. Otherwise the checks run as today: unknown boot, the stat result, the zombie test,
   the start time and the TTL fallback.

A pid-less claim takes the TTL path exactly as today.

`Unverifiable` never decays into `Stale` with age. A live owner running one long
command keeps its claim past four hours, even against a sandboxed caller (Testing).

### 4. Call sites

The rule: **only `Stale` permits automatic action.** `Unverifiable` gets `Live`'s
protection and its own wording. A helper, `Liveness::is_stale()`, replaces the
`== Live` / `!= Live` comparisons, so each site states its intent.

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

- An evaluator in the recording view, where the verdict is ordinary `Live` or
  `Stale`, so its writes prune the claim once the owner is dead. This needs a
  recorded view.
- A reboot.
- The owner's own `park` or `done`.
- An explicit `start --force`, recorded as `unverifiable, forced`.

A claim recorded without a view, or in a sandbox that has since exited, therefore
stays until reboot, the owner's release, or `--force`. Wrongly deleting a live claim
costs more than that.

No new warnings. Inside a sandbox every host pid claim reads `Unverifiable`, so a
warning per claim would be noise on every command. `tasks claims` and `show` carry
the reason.

### 5. JSON

`ClaimInfo` keeps `live: bool`, defined as "held": `true` for `Live` and
`Unverifiable`. It gains an optional `unverified: "<reason>"`, present only for
`Unverifiable`. Consumers that read `live: false` as "may be taken", ops
`claim-guard` among them, stay safe without a change. The rejected alternative,
`live: false` plus the new field, would tell every older consumer that a protected
claim is free. The addition is additive. The README documents both fields where it
lists `tasks claims`, which has no field-level contract today.

### 6. Decisions on the task's remaining questions

- **Refusing `TASKS_SESSION_PID` outside a common numbering:** no. No evaluator can
  know another view's numbering. An exported pid that the writer cannot establish
  records no view (§2), so it reads `Unverifiable` rather than reused or gone. That
  closes `host-view-of-ns-pid` without a refusal.
- **A relay walk that ends at a pid 1 with ppid 0 in a non-initial namespace:** no
  change. With a plain sandbox helper the walk ends Outside, and the native Codex
  level records a pid-less TTL claim (`ns-plain-private-proc`). That claim does not
  depend on any view. Its one harm was the prune, which §4 removes. The Codex-helper
  shape already refuses; tasks-19b244 names the cause.
- **Legacy claims (a pid and no view):** `Unverifiable` under §3 rule 2, in every
  view. They clear through §4's paths. The heartbeat rules in §2 give one a view at
  the owner's next write that establishes its pid. One such claim exists on this
  host today.

### 7. Mixed versions are unsafe; upgrade every binary together

The store field is additive, but an older binary still judges every pid through its
own `/proc`:
- its `prune_dead` can delete a protected claim before it saves;
- it drops `pid_view` when it saves;
- its read commands report a protected claim as stale.

The protection holds only when **every `tasks` binary that reads or writes a host's
claim stores runs this version or later**. That covers each `tasks` on `PATH` in each
harness's environment, including one a sandbox can execute. The README's Upgrading
paragraph states this requirement, as it does for relay registry schema 2, with the
same `tasks --version` check.

Rejected: making older binaries fail on the new store. No serde-compatible change
makes an older binary refuse only writes. Any change it cannot parse also breaks its
read commands in every project on the host. The relay precedent documents the
requirement instead.

## Testing

Unit tests for the view reader, on injected `/proc/self/status` text and `stat`
results:
- one `NSpid` entry with a readable link gives the `"<dev>:<ino>"` identity;
- two entries give unknown;
- a missing or unparsable `NSpid` line gives unknown;
- an unreadable link gives unknown.

Unit tests on `liveness_with`, injecting the view as the tests already inject `stat`:

- Same view: live, gone, zombie and reused give today's verdicts.
- Different view: a claim that would read gone or reused gives `Unverifiable`.
- Different view, `seen` older than four hours: `Unverifiable`, not `Stale`.
- Recorded view absent with a pid: `Unverifiable`. Current view unknown:
  `Unverifiable`.
- Known boot mismatch: `Stale` whatever the view.
- Pid-less claim: TTL as today, whatever the view.

Unit tests on the evidence rules (§2), with the view injected:

- A relay-level write records the view.
- A `sid:` write whose pid `proc_stat` finds records the view.
- An exported pid that `proc_stat` finds records the view when the view is the
  initial PID namespace.
- An exported pid records no view in any other known view, even when `proc_stat`
  finds it and it is the writer's ancestor.
- An exported pid records no view when `proc_stat` fails, or when the view is
  unknown.
- The initial-namespace test matches only `PROC_PID_INIT_INO` with one `NSpid` entry.
- Heartbeats: each of the four rows in §2's table.

Call-site tests:
- `prune_with`/`prune_dead` keeps an `Unverifiable` claim.
- The hold snapshot keeps its holds.
- `ClaimInfo` gives `live: true` and `unverified`.
- `takeover_summary` and `describe_claim` give the new wording.

Integration tests in `tests/cli.rs` write the store as fixtures, so no environment
backdoor is needed. The fixture claim carries the test process's pid and start time
and a `pid_view` that cannot match:
- An unrelated `start` leaves it in place.
- Another session's `start` refuses `claimed`.
- `start --force --reason` takes it over with an `unverifiable, forced` note.
- `tasks claims` reports it as `live: true` with `unverified`.

Acceptance runs against the worktree build, using unprivileged `unshare` as
tasks-fe6c59's probe does.

- **The probe's own cases.** Rerun `prune-*`, `ns-explicit-pid1` and
  `host-view-of-ns-pid`. The host claim must survive the sandboxed write, and the
  host must refuse the takeover without `--force`.
- **Procfs unmount and reuse.** A claim is written in namespace A (private `/proc`)
  through the Unix session fallback, so it records A's view, while A's pid 1 stays
  alive. Then A's `/proc` is unmounted, and namespace B mounts a
  procfs, reusing the device number if the kernel allows. A write from B must leave
  A's claim in place. Record whether the device was actually reused. The case still
  passes if it was not, because the identity never reads the device.
- **Exported host pid.** In a private-`/proc` child, `TASKS_SESSION_PID` is set to a
  live host pid absent from the child's `/proc`. The claim must record no view. A
  second write in the child must leave it in place.
- **Colliding ancestor (spec round 2).** Use nested PID namespaces. The intended
  owner is pid 2 in the outer namespace and launches the inner one. In the inner
  namespace a different ancestor also has pid 2, and `TASKS_SESSION_PID=2` is
  inherited. Check five things:
  - The inner claim records no view.
  - After the inner ancestor exits while the outer owner lives, an inner write
    leaves the claim in place.
  - Another inner session's `start` refuses `claimed`.
  - An inner heartbeat of an owner claim recorded in the outer namespace keeps the
    outer evidence.
  - `NSpid` has one entry inside, as the reviewer observed.
- **Heartbeat from a sandbox.** The owner's claim is written on the host with a view,
  then heartbeated from a private-`/proc` child with an unestablished pid equal to the
  recorded one. The host view must still be recorded afterwards.

The genuine `codex sandbox` cases are optional. They need the installed Codex CLI and
no model call.

## Documentation

- The README paragraph on PID namespaces covers three points:
  - Claims record the PID view they were established in. Evaluators in other or
    unknown views protect such a claim but do not judge it.
  - Exported pids establish a view only in the initial PID namespace.
  - `TASKS_SESSION_PID` must name a process in the namespace of the command that
    sets it.
- The README's Upgrading paragraph: the coordinated-upgrade requirement in §7.
- The README's `tasks claims` entry: `live` means held, and `unverified` gives the
  reason a held claim could not be judged.
- The tasks skill, in step: one sentence in the claim paragraph saying that a claim
  this command cannot judge is held until its own view, its owner, a reboot or
  `--force` releases it.
