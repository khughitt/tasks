# Which checkout owns a task record

Status: draft for review (tasks-ab8d2d). Brief:
`docs/notes/2026-09-29-cross-checkout-records-brief.md`. Goal: tasks-c4ad8e.

## 1. Problem

A task's record exists once per checkout. The main checkout and every linked worktree hold
their own `tasks/<id>.md`, and a write lands in whichever copy the command was run next to.
Three failures follow:

- **A write from a stale copy.** `save` warns when another checkout's copy is newer, but
  writes anyway (tasks-2c0a1d). The warning arrives after the damage and falls quiet once
  the write's newer stamp wins.
- **A write that forks equal copies.** With a worktree holding an equal copy, a write in the
  main checkout says nothing, and the two copies have diverged (tasks-bb53e5). This is what
  happened on tasks-142d2f: `start` in main, `git worktree add`, a note in main, and the note
  was later replayed by hand. An agent whose shell resets to the main checkout between
  commands reproduces it without meaning to.
- **A read that cannot see worktree-only work.** `show` reads only the local checkout, so a
  task filed in a worktree is `task_not_found` everywhere else, even while its claim or park
  names the checkout that has it (tasks-fbc32b).

Documents kept out of git are settled separately (tasks-2325a1, tasks-ace27b): readers look
in the main checkout for a spec or plan the worktree lacks.

## 2. Decisions

1. **A write from a copy that is behind refuses** (user, 2026-09-30). For every task, claimed
   or not, a write whose loaded copy is older than another checkout's copy fails with a new
   error kind, `stale_copy`, before anything is written. The error names the newer checkout
   and gives the exact retry. There is no override flag. This reverses the work-claims rule
   that the sibling check is "a signal, not a gate".
2. **The claim follows its holder.** Every successful write by the session holding a live
   claim sets the claim's `worktree` to the checkout it wrote in. The claim names where the
   work is, so no separate handoff command is needed.
3. **The first `tasks` command in a new worktree is `tasks start <id>`.** It is a protocol
   step, not a CLI rule. It moves the claim and makes the worktree's copy the newest, so any
   later write left behind in the main checkout refuses under decision 1.
4. **`show` falls back to the checkout named by the claim or park** when the local checkout
   has no record, with a warning. Plain `list` stays local, and no JSON shape changes.

## 3. The stale-copy refusal

### 3.1 Rule

Every write to an existing record compares the `updated` stamp it loaded with the same
record in every other worktree of the repository. It uses the same `sibling_task_copies`
lookup the warning uses today: `git worktree list --porcelain -z`, read at the project's
offset below the repository top level. If any sibling's stamp is newer, the write refuses
with `stale_copy`, and neither the record nor the claim store changes.

The rule replaces `warn_on_newer_sibling_copies` at the same point in `save`, before the
claim store or the file is touched. It also covers the one writer that bypasses `save`:
`feedback --recur` appending to an existing record in the owner project. A single shared
function serves both.

Unchanged from the warning:

- A sibling that is merely **behind** says nothing. A long-lived worktree that is behind is
  the resting state, and writing ahead of it is what the merge will carry.
- A sibling copy that cannot be read or parsed is a **warning**, not a refusal: the rule
  refuses only on proof that a newer copy exists. The same holds for a git failure while
  listing worktrees.
- A checkout with no siblings, or outside git, is never compared.
- Creating a record (`add`, a new `feedback`) has nothing to compare and is never refused.

The rule applies to every command that saves an existing record: `note`, `edit` (flags and
`$EDITOR`), `start`, `done`, `drop`, `block`, `unblock`, `park`, `shelve`, `unshelve`, `dep`,
`attach`, `detach`, and `feedback --recur`. `note` has always been "never refused" by the
claim guard; that is still true of the claim guard, but `note` is not exempt from this rule.
A note written to a stale copy is exactly the tasks-142d2f failure.

`start` gets no exemption either. A handoff into a checkout that is behind would leave the
newer copy's changes out, so it refuses like any other write.

### 3.2 The error

    stale_copy: tasks/<id>.md in <root> is newer than this copy (<theirs> there, <ours>
    here); nothing was written. Run it there: tasks -C <root> <args>

- `<root>` is the sibling with the newest stamp. When several siblings are newer, the others
  follow in a trailing `(also newer in: <root>, …)`.
- `<args>` is this invocation's arguments after the program name, with any `-C <dir>`,
  `-C<dir>` or `-C=<dir>` removed. Each argument is single-quoted when it contains a
  character outside `[A-Za-z0-9_@%+=:,./-]`, so the line can be pasted into a POSIX shell.
  `attach`'s file argument is made absolute, because `-C` changes the directory that a
  relative path is read from.
- On the `$EDITOR` path the edit is kept, and the detail appends the temp file's path as that
  path's other errors do.
- `feedback --recur` is the exception. `-C` would change the project the report comes from,
  so there is no `-C` retry. The detail names the owner's newer checkout and offers `--new`,
  which files a separate entry.

`rename` rewrites every record under its own preconditions (clean `tasks/`, at most one
worktree) and is not a save of one record, so it is outside this rule.

The retry sends the write to the newer checkout. When that checkout is the wrong home (for
example, a note landed in main before the worktree's first command), the fix is the same as
today: merge the newer copy into the checkout that should own it. The CLI does not copy
records between checkouts (fresh-worktree brief).

### 3.3 Exit status and JSON

`stale_copy` is a new error kind and exits 1, like every other typed error. The error object
keeps its shape, `{"error": {"kind", "detail"}}`. The kind is additive and needs no other
contract change.

## 4. The claim follows its holder

A claim records `worktree` when it is acquired. In the prescribed order (commit the record,
then `git worktree add`), `start` runs in main, so the claim names main while the work
happens in the worktree.

Rule: **a successful save by the holder of a live claim on that task sets the claim's
`worktree` to this checkout's root, and `seen` to now.** Holder means what the claim guard
and `note`'s heartbeat already mean: the same resolved session identity.

- `note` already refreshes `seen` on the holder's claim, and now also sets `worktree`.
- Field edits, `dep`, `attach` and `detach` by the holder gain the same refresh. The store
  write comes after the record write, as `note`'s does. A failed store write is a warning
  naming what landed and what did not, in the form `note` uses today.
- `start` already acquires with this checkout's root and needs no change.
- Closing and parking release the claim, and a park records its own `worktree` as today.
- A save by anyone else never touches the claim, and the claim guard's rules are unchanged.
- Park entries do not follow anyone. They name the checkout `park` ran in, and `start`
  replaces them (park design §4.1).

This is compatible with park design §4.3: an editor save that leaves the status unchanged
still neither acquires nor releases an entry. It only refreshes one the caller already holds.

No warning is printed when the worktree moves. The move is the claim's documented meaning,
and `claim.worktree` is visible on `show`, `prime` and `list`.

## 5. The first command in a new worktree

The skill's session protocol gains one step. **After `git worktree add` for a task you have
started, run `tasks start <id>` in the new worktree before any other `tasks` command for
it.** On a `doing` task this is a resume: it appends `resumed` and re-acquires the claim
with the worktree's root. The worktree's copy is then newer than main's, so a later write
from main refuses under §3 and names the worktree.

Without the step, nothing is lost that §3 cannot catch. A write in main lands, since main's
copy is still current, and the worktree's next write refuses and names main. The step only
moves detection from the second write to the first.

`skills/tasks/SKILL.md` (Process and workspace, and step 3 of the session protocol) and this
repository's `AGENTS.md` process section carry the step. The global agent instructions
belong to another project, so that change is filed there as feedback rather than made here.

## 6. Reads

### 6.1 `show`

When the local checkout has no record for the id, `show` looks where the live claim names,
and otherwise where the park names. It uses `scan_recorded`, the park design §5.3
resolution that `prime` already uses for worktree-only claims. When the record is found
there, `show` renders it from that checkout's scan, and the output carries the warning:

    <id> exists only in <worktree>; shown from that checkout

When no claim or park names a checkout, or the one named is gone, belongs to another prefix,
or lacks the record, the result is `task_not_found` as today. When a checkout was named, the
detail names it and says it was unavailable.

A record present locally is always read locally (park §5.3), even when another checkout's
copy is newer. Reading does not risk divergence, and the next write from here will refuse
under §3.

`ShowFields` keeps its shape. The spec and plan paths resolve against the checkout the
record was read from, with the main-checkout fallback of tasks-ace27b.

### 6.2 `list`

Plain `list` stays a view of the local checkout. Worktree-only work that someone holds is
already visible elsewhere: `prime`'s `doing` resolves claims in other checkouts
(tasks-476c6b), and `list --parked` resolves parks (§5.3). Adding worktree-only rows to
`list` would change what its rows mean and would need a `checkout` field on every row. That
is a JSON contract change with no request behind it beyond tasks-fbc32b, whose `show` half
this design covers.

## 7. Rejected alternatives

- **Refuse only for claimed or parked tasks** (the brief's lean). Unclaimed ideas diverge the
  same way, and a rule that depends on claim state is harder to predict. The user chose
  "refuse always".
- **Stay a signal.** This would leave tasks-2c0a1d and tasks-bb53e5 as documented gaps.
- **A `not_home` refusal for writers who do not hold the claim.** The design offered with the
  decision included it. It is dropped because §3 already covers it: every home move (§4, §5)
  is a write that leaves the home's copy the newest. So any write from elsewhere is already
  behind and refuses as `stale_copy`. The copies are equal again only after a merge, which is
  exactly when writing elsewhere is harmless.
- **Routing a write to the home checkout automatically.** The command reports success while
  the change sits in a checkout the writer is not looking at, and the commit that should
  carry it happens somewhere else. The retry is printed instead, and a person or agent runs
  it.
- **Re-`start` as the only handoff** (the brief's lean). The holder's own writes already
  show where it works. Requiring a handoff command would make an ordinary `done` in the
  worktree refuse.
- **An override flag.** `--force` already carries two meanings. The escape from a refusal is
  to write in the newer checkout, or to merge or remove it.
- **Comparing content instead of stamps.** Rejected by the work-claims design as noise, and
  that reasoning is unchanged.

## 8. Known gaps

- **Same-second writes.** `updated` has second precision, so writes in two checkouts within
  the same second compare equal and neither refuses.
- **A write in the wrong checkout.** A write that landed in the wrong checkout before
  anything was newer (no §5 step) still needs a manual merge. §3 makes the next write
  report it; it does not undo it.
- **Abandoned worktrees.** A worktree that is abandoned but not removed, holding a newer
  copy, blocks writes to that task everywhere else until it is merged or removed. This is
  the accepted cost of "refuse always", and the error names the worktree.
- **Invisible worktree-only tasks.** A task that exists only in a worktree, with no claim or
  park, is still invisible to `show` elsewhere. Nothing names its checkout.
- **Checkouts outside git's worktree list.** A second project root under the same prefix
  that is not a linked worktree (for example, `init --force` in another clone) is not
  compared under §3, although claims and parks may name it.
- **Other machines.** Nothing crosses machines, as the work-claims design already records.

## 9. Documents that change

- Work-claims design §Warnings, "A newer copy elsewhere": becomes the refusal, with this spec
  cited. The two accepted limits stay. The "never refuses" sentence and its rationale in
  `warn_on_newer_sibling_copies` go.
- Work-claims design, `note` under §Command behaviour: never refused by the claim guard, but
  subject to §3.
- Park design §5.3: `show` resolves a record that exists only in another checkout the same
  way.
- Tasks design §7 is unaffected. The README and `skills/tasks/SKILL.md` gain `stale_copy`,
  the §5 step, and the `show` fallback.

## 10. Testing

End-to-end tests in `tests/cli.rs`, on a scratch repository with a linked worktree:

- `note`, `edit`, `done` and `start` from a copy behind a sibling each exit 1 with
  `stale_copy`. The detail names the sibling and a retry that, run verbatim, succeeds. Both
  record files and the claim store are unchanged afterwards.
- The same writes from the newer checkout succeed, and a sibling that is only behind
  produces no finding.
- An unreadable sibling copy warns and writes.
- The retry's quoting: a note with spaces and quotes, an invocation that already carried
  `-C`, and `attach` with a relative path.
- `feedback --recur` onto a record whose copy is behind refuses.
- The tasks-142d2f sequence: `start` in main, `git worktree add`, `tasks start` in the
  worktree, then `note` in main. The note refuses and names the worktree, and the claim
  names the worktree.
- A holder's `note` and `edit` in a worktree move `claim.worktree` there. Another session's
  `note` leaves it alone.
- `show` in main for a task added and started in a worktree renders it with the warning. A
  parked worktree-only task also renders. A claim naming a removed worktree gives
  `task_not_found` with the checkout named.

## 11. What the ideas get

When this spec is approved, in the same commit as closing tasks-ab8d2d:

- tasks-2c0a1d: closed by §3 (refuse, naming the newer checkout and the retry).
- tasks-bb53e5: covered by §3 together with the §5 step. Without the step, the write in main
  is caught at the worktree's next write instead.
- tasks-fbc32b: `show` covered by §6.1. The `list` half is declined in §6.2.
