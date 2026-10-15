# Release a claim whose holder is gone

Status: draft
Task: tasks-f9fc01 (lane tasks-2fa8c6, Agent identity and lifecycle)
Builds on: [claim liveness by PID view](2026-10-14-claim-pid-view-design.md) (tasks 0.3.0)

## Outcome and scope

Since 0.3.0 a pid claim that this command cannot judge reads `Unverifiable`. That covers
a claim recorded in another PID view, a claim recorded with no established view, and a
legacy claim. Nothing prunes such a claim, takes it over without `--force`, or releases
its holds (design §4). While it stands it hides the task from `ready` and `next` and
blocks `rename`, `rename --adopt`, `retire` and `move`. It clears only through its own
view, its owner's `park` or `done`, a reboot, or `start --force`.

When a person knows the holder is gone (a finished sandbox, a dead container), the only
path today is `start --force` followed by `park` or `drop`. That writes a takeover, a
`resumed` marker and a park the task never had. This design adds one direct, recorded
release. It also makes every refusal that such a claim causes name that release.

Out of scope: showing the unverifiable reason in pretty output (tasks-ea23de), sid:0
claims (tasks-f11052), and any automatic reaping. The 0.3.0 rule stands: only `Stale`
permits automatic action.

## Evidence checked

At main 00cdbd8:

- No verb releases a foreign held claim without a lifecycle. `park` refuses a foreign
  `Live` or `Unverifiable` claim and has no `--force` (`src/commands/park.rs`). The
  status transition guard lets `--force` through only toward `doing`
  (`src/commands/mod.rs`, `Error::Claimed`), so `drop --force` and `done --force` refuse
  too. `tasks claims` opens no checkout and writes nothing.
- The refusals name no remedy for an unverifiable claim. `rename` (`src/rename/mod.rs`
  `authorize`), `rename --adopt` (`src/rename/adopt.rs`) and `retire`
  (`src/commands/retire.rs` `reject_live_claims`) return `claimed` with
  `describe_claim` alone. `ready`'s omission warning (`src/commands/list.rs`
  `claim_omission` + `force_hint`) names only `start --force`.
- `move`'s preflight (`src/moving/preflight.rs`) says "park it with `tasks park …`
  first". For a foreign claim, that park refuses: the hint dead-ends.
- The record-home check counts every non-`Stale` foreign claim as an occupant of its
  worktree (`occupants`, `src/commands/mod.rs`). Suppose the gone holder's worktree
  still holds a newer copy of the record. A write from the main checkout is then
  refused with "wait for that branch to merge". Nobody will ever merge that branch.
- Holds live on the claim (`src/holds.rs`), so removing the claim frees its exclusive
  holds. Parks and escalations are separate store entries.

## Design

### 1. The command

    tasks release <id> "<why>"

The reason is positional and required, like `drop`'s message and `shelve`'s wake
condition. It is the one thing the record keeps that the tool cannot know. The id
routes by prefix as `note` does, so a claim in another registered project is released
from anywhere.

    tasks release <new-id> "<why>" --adopting <old>

The adoption form exists because of one case. Another host renamed the project, the
renamed checkout has synced here, and this host has not yet run `tasks rename <old>
<new> --adopt`. The record is now `<new>-<hex>`, but this host's claim for it still
sits in `<old>`'s store under `<old>-<hex>`. Adoption refuses while that claim is held,
and ordinary routing cannot connect the two: `<old>`'s registered root now holds a
project with another prefix, or no project at all. With `--adopting <old>`, the id
routes as usual to the renamed record in this checkout, and the claim is looked up in
`<old>`'s store under `<old>-<hex>`. The note lands on the renamed record, the store
saved is `<old>`'s, and the rest of the command is unchanged.

`--adopting <old>` is accepted only when adoption itself would accept `<old>` for this
checkout: `rename::adopt::classify_registry(registry, project, old, project.prefix)`
returns `Fresh`, the stage in which adoption checks the old store's claims. Anything
else is `validation`, naming that check, with nothing written. The flag names the
predecessor explicitly. Release never searches the registry for one, because adoption's
check also matches any registered project whose root has vanished, which could reach an
unrelated store. The command takes both prefixes' mutation locks in sorted order before
it loads the record, as `rename --adopt` does, and the record load reuses the lock it
already holds.

Rejected:
- **`park --force`.** Park records a next step and a park under the releaser: the same
  false lifecycle this task removes.
- **`claims --release`.** `claims` reads every store and opens no checkout, but the
  release must write the record.

### 2. What it releases, and what it refuses

Under the claim store's mutation lock (both locks under `--adopting`), the command judges the task's claim from this
command's view:

| Claim | Result |
|---|---|
| none | `validation`: "`<id>` has no claim" |
| this session's (ownership not `Foreign`) | `validation`: "the claim on `<id>` is yours; `tasks park` or `tasks done` releases it" |
| foreign, `Live` | `claimed`, with `describe_claim` and "only its session releases a live claim; `tasks start --force <id>` takes it over" |
| foreign, `Unverifiable` | released |
| foreign, `Stale` | released (any write would prune it; releasing it records why) |

The `park`, `done` and `start --force` remedies in those two rows act on the routed id's
store. Under `--adopting` that is the new prefix's store, so they cannot clear the old
claim being refused, and following them would change the lifecycle or add a second
claim while adoption stays blocked. Under `--adopting`, those two rows therefore read:

| Claim in `<old>`'s store | Result |
|---|---|
| this session's | `validation`: "the claim on `<old>-<hex>` is this session's, recorded before the rename; it clears when this session ends, and `tasks rename <old> <new> --adopt` then proceeds" |
| foreign, `Live` | `claimed`, with `describe_claim` and "the claim on `<old>-<hex>` was recorded before the rename; it clears when session `<s>` ends, and `tasks rename <old> <new> --adopt` then proceeds" |

Neither names `park`, `done` or `start --force`. Once the holder ends, a judgement in its
view reads `Stale`, and adoption passes a stale claim as it does today.

An unresolved identity of this session counts as foreign. Release asserts nothing about
this session. Releasing a `Live` claim is never offered: the person's knowledge that the
holder is gone is evidence only where the tool has none.

### 3. What it records and changes

One note on the task, stamped with lifecycle provenance (`harness_session`) like
`note --stamp`. The text reuses `takeover_summary`'s redaction (no Unix session pid,
no namespace values):

    released session <s> (owner <o>, unverifiable: <reason>); reason: <why>

`Stale` reads `stale` with no reason, as in a takeover note: a stale reason can name a
host pid (`pid <n> is gone`), and a record keeps none. A sid claim reads `Unix session`, as in a takeover.
The note is not a lifecycle marker. Release changes no status, so `obs` and other
marker consumers need nothing new.

The command removes only the claim, and the claim's holds go with it. Status, owner,
park and escalation are untouched. A `doing` task stays `doing` with no claim, the same
state a pruned stale claim leaves. The next `start` takes it without `--force` and
writes an ordinary `resumed`.

### 4. Ordering and failure

The project's mutation lock, which a write takes when it loads the task
(`Ctx::lock`), is held from the judgement through the store save. No heartbeat or
takeover can change the claim between the verdict and the removal. Within the lock, the
order is `park`'s: record first, then store. The record write passes every ordinary
guard (`guard_write`, retired, move-pending, stale copy). Only after it lands is the
claim removed and the store saved.

If the store save fails after the note landed, the command fails with the store's error
and the suffix "the note on `<id>` landed but its claim was not removed; rerun the same
`tasks release` command". Naming the command rather than reprinting it avoids
shell-quoting a free-text reason, and covers `--adopting`. The kind stays the store's (`io`), so `park`'s
silent-success shape (tasks-c87f7d) is not repeated. A rerun appends a second note. The
cost is accepted: a release that leaves no record is worse.

### 5. The stale-copy check

For `release` alone, `occupants` skips every non-`Live` claim of the session being
released: the target claim and that session's other claims in the same worktree. The
person asserts that the session is gone, not one claim. Live claims of that session, and
every other session's claims, still count. Parks still count: a park names work that the
next session resumes, not a session that must return. Stale-copy protection itself is
unchanged. Only the remedy differs, because the occupancy diagnostic no longer names the
gone session. When the gone holder's worktree holds a newer copy, the refusal then gives
the ordinary remedy: rerun there with `tasks -C <root> release …`, hand off, or merge.
It no longer says to wait for a session that will never return. A worktree that no
longer exists has no copy to compare, so the release proceeds.

### 6. Refusals that name it

Each hint appears only when the blocking claim reads `Unverifiable`. A `Live` claim
keeps its wording; a `Stale` one already passes. The hint is
`; if session <s> is gone, run \`tasks release <id> "<why>"\``. It is appended to:

- `rename` (`claimed`), for a claim in the source prefix's store, whose record lives in
  this checkout. A claim in the target prefix's store has no record here to note, so it
  gets no hint;
- `rename --adopt` (`claimed`). A claim in the new store gets the plain hint. A claim in
  the old store gets the adoption form with the renamed id:
  `tasks release <new>-<hex> "<why>" --adopting <old>`;
- `retire` (`claimed`);
- `move`'s preflight. Its park hint is corrected at the same time: an own claim keeps
  "park it with `tasks park …` first"; a foreign `Live` claim says its session must
  park it first;
- `ready`'s omission warning, after the `start --force` hint.

`next` and `prime` pass through the same omission code (`list::ready_tasks`), so they
gain the same hint. `lanes` has no omission warning.
`sample` keeps its curator wording, with no takeover or release hint.

### 7. JSON

The output is `IdOut` (`{"id", "warnings"}`), as for `note` and `park`. No existing
shape changes, and errors use existing kinds. The claim store format is unchanged, so
§7 of the PID-view design (upgrade every binary together) adds nothing new. An older
binary simply lacks the verb.

## Testing

CLI tests build an unverifiable claim the way the PID-view tests do: a store entry with
a pid and no `pid_view` reads `Unverifiable` in every view.

- Release of a foreign unverifiable claim: exit 0, the claim is gone from `tasks claims`,
  one stamped note carries the redacted text and reason, and status, owner and park are
  unchanged. A following `start` without `--force` succeeds with no takeover note.
- Release frees an exclusive hold: another session's `start` of a step needing it then
  passes without `need_held`.
- Release of a stale foreign claim succeeds and records `stale`, with no pid in the note.
- Refusals: no claim (`validation`), own claim (`validation`), live foreign claim
  (`claimed`), each with nothing written.
- Stale copy: with a newer copy in the released claim's worktree, the refusal is the
  rerun remedy, not "where another session works".
- Stale copy, two claims: gone session S holds A and B in one worktree whose copies
  are newer. Releasing A gives the rerun remedy, not "where another session works",
  and the same holds for B. A live claim of another session in that worktree still
  yields the occupied remedy.
- Adoption, end to end. Build a project, simulate another host's rename (the checkout
  says `<new>` and holds `<new>-<hex>`, while this host's registry still has `<old>`),
  and put an unverifiable claim on `<old>-<hex>` in `<old>`'s store. `rename --adopt`
  refuses with the adoption hint. Running the command that hint prints succeeds and
  notes `<new>-<hex>`. Rerunning `rename --adopt` then completes.
- `--adopting` with an own claim and with a live foreign claim in the old store: each
  refusal carries the adoption text and names none of `tasks park`, `tasks done` or
  `start --force`. Nothing is written in either store or the record.
- `--adopting` refusals: a prefix adoption would not accept (unregistered, already
  adopted, the checkout's own prefix) is `validation` with nothing written.
- Hints: `rename`, `retire`, `move` and `ready` name `tasks release` for an unverifiable
  claim and not for a live one. `move`'s hint for a foreign live claim no longer says
  to park it yourself.
- Store failure after the note: the error carries the rerun suffix (unit test with an
  injected store failure, where the existing store tests allow it).

## Documentation

- `skills/tasks/SKILL.md`: the claims paragraph ("Its own view, its owner, a reboot or
  `start --force` releases it") gains `tasks release <id> "<why>"` for a holder known
  to be gone.
- README: the PID-view paragraph and the command list.
- Version: a new command, so 0.4.0. Rerun `cargo install --path .` after merge.
