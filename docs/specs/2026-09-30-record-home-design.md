# Which checkout owns a task record

Status: round 3 revision, for the user's re-review (drafted under tasks-ab8d2d, revised
under tasks-9949f3). Round 2 was a Codex accept; the user returned round 3. Brief:
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
   error kind, `stale_copy`, before anything is written. So does a sibling with the same
   stamp but different bytes, which only a same-second fork produces. The error names the
   newer checkout and says what to do: rerun there, merge that copy here, or leave it to the
   session working there (§3.2). There is no override flag. This reverses the work-claims
   rule that the sibling check is "a signal, not a gate".
2. **The claim follows its holder.** Every successful write by the session holding a live
   claim that leaves the claim in place sets the claim's `worktree` to the checkout it wrote
   in. That covers every write except those that release the claim: closing, parking, and
   any other status change away from `doing`. The claim names where the work is, so no
   separate handoff command is needed.
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
with `stale_copy`. Nothing is written or created, and no input is read beyond the arguments:
not stdin, not the clipboard, and not a file to attach.

**Same stamp, different bytes.** A sibling whose stamp equals the loaded one but whose
bytes differ from the bytes this command loaded also refuses. Only a same-second fork
produces that: every write sets a new stamp, and a new worktree copies the bytes unchanged.
A write stamps strictly after the stamp it loaded (now, or one second past the loaded stamp
when the clock has not passed it), so a write that follows another checkout's within the
same second still leaves its own copy the unique newest rather than an equal-stamp fork
(tasks-cff04e).
The work-claims design rejected content comparison because copies that are *behind* would
fire constantly. That still holds: a sibling that is behind is never compared by content.

**When it runs.** The check runs as soon as a write command has loaded the record, under the
mutation lock. That is before the command reads any other input or creates any file. The
lock is per prefix and shared by every checkout on the host, so no sibling can write between
the check and the save. `save` itself does not check. Checking there is too late: `attach`
copies its payload into `tasks/files/<id>/` before calling `save`, and `edit --body -` and
`attach -` have consumed stdin by then.

- **The `$EDITOR` path** releases the lock while the editor is open, so it runs the check a
  second time after `lock_and_revalidate` takes the lock back.
- **A feedback recurrence** writes to an existing record without going through `save`. It
  happens two ways: `feedback --recur <id>`, and a report whose title matches an open
  feedback entry. Both reach `recur_into`, which runs the same check after it reads the
  owner's record and before anything is appended.

One shared function replaces `warn_on_newer_sibling_copies`, and every one of these points
calls it.

Unchanged from the warning:

- A sibling that is merely **behind** says nothing. A long-lived worktree that is behind is
  the resting state, and writing ahead of it is what the merge will carry. A sibling with
  the same stamp and the same bytes is the same record, and says nothing either.
- A sibling copy that cannot be read or parsed is a **warning**, not a refusal: the rule
  refuses only on proof that a newer copy exists. The same holds for a git failure while
  listing worktrees.
- A checkout with no siblings, or outside git, is never compared.
- Creating a record (`add`, a new `feedback`) has nothing to compare and is never refused.

The rule applies to every command that writes an existing record or its attachments:
`note`, `edit` (flags and `$EDITOR`), `start`, `done`, `drop`, `block`, `unblock`, `park`,
`shelve`, `unshelve`, `dep`, `attach`, `detach`, and both kinds of feedback recurrence. A
write that never reaches `save` is still covered, because the check runs at load. `detach`
has such a path: when the ledger already records the detach, it saves nothing and only
removes a leftover file from `tasks/files/<id>/`. The load check refuses before that
removal as well. `note` has always been "never refused" by the claim guard; that is still
true of the claim guard, but `note` is not exempt from this rule. A note written to a stale copy is exactly the tasks-142d2f failure.

`start` gets no exemption either. A handoff into a checkout that is behind would leave the
newer copy's changes out, so it refuses like any other write.

### 3.2 The error

    stale_copy: tasks/<id>.md in <root> is newer than this copy (<theirs> there, <ours>
    here); nothing was written. <remedy>

`<root>` is the sibling with the newest stamp. When several siblings are newer, the others
follow in `(also newer in: <root>, …)` before `; nothing was written`. In the same-stamp
case (§3.1) the head reads `tasks/<id>.md in <root> has the same stamp as this copy
(<stamp>) but different content, so both were written in the same second`.

The remedy depends on where the newer copy is and who works there. The first rule that
applies decides it:

1. **Another session works there.** A live claim held by a session other than the caller
   (`Ctx::ownership` is `Foreign`), or a park recorded by another session, names `<root>`
   as its worktree. For a park, "another session" means its recorded session differs from
   the caller's resolved identity, or the caller's identity does not resolve. The claim or
   park may be on any task, not only this one: a worktree's work touches other records too,
   as §11's notes do. The remedy names each such task and its session, then says `wait for
   that branch to merge, or merge its copy of tasks/<id>.md into this checkout, then rerun
   here`. There is no `-C` retry, because a write there would leave an uncommitted edit on
   someone else's branch.
2. **Same stamp, different bytes.** The remedy is the merge alone: `merge that copy of
   tasks/<id>.md into this checkout, then rerun here`. A write to either copy leaves the
   other still forked.
3. **A handoff from the main checkout.** This checkout is a linked worktree, and `<root>` is
   the main checkout (the first entry of `git worktree list`). The remedy leads with `commit
   tasks/<id>.md in <root> if it has changes, merge it into this branch, then rerun here;
   the merge may conflict where both copies changed`. The retry follows as the alternative:
   `or, to write in the main checkout instead: tasks -C <root> <args>`. With a bare retry,
   `tasks start` in a new worktree would print `tasks -C <main> start <id>`, which moves the
   claim back to main.
4. **Otherwise**, including when the caller's own claim names `<root>` (a shell that reset to
   the main checkout), the remedy is the retry: `Run it there: tasks -C <root> <args>`.

The caller's identity is resolved only on this refusal path, never for a write that
passes. When the claim store or identity cannot be read, the remedy is chosen as if no
other session were named, and the detail says so. A refused command prints only its error
object, so a warning would never be seen.

Wherever a `-C` retry appears:

- `<args>` is this invocation's arguments after the program name, with any `-C <dir>`,
  `-C<dir>` or `-C=<dir>` removed. Each argument is single-quoted when it contains a
  character outside `[A-Za-z0-9_@%+=:,./-]`, so the line can be pasted into a POSIX shell.
  `-C` only chooses the project; the process keeps its working directory. So a relative
  path, such as `attach`'s file argument, resolves the same way in the retry.
- **Input on stdin.** A command that takes input on stdin (`edit --body -`, `attach -`) has
  not read it when it refuses (§3.1). The detail adds `supply the same input on stdin`,
  because the printed line cannot carry the input itself. `attach --clipboard` has not read
  the clipboard either, and its retry reads it again.
- **The `$EDITOR` path.** When the first check refuses, the editor never opens. When the
  second check refuses, the edit is kept and the detail appends the temp file's path, as
  that path's other errors do. It also says that a rerun opens a fresh editor on the newer
  copy, so the kept file is where the changes are copied from.

A feedback recurrence, explicit or automatic, never offers `-C`, because `-C` would change
the project the report comes from. Rules 1 and 2 apply as written. Where rule 4 would give a retry, the remedy is
`Rerun with --new to file a separate entry`. Rule 3 cannot arise: feedback writes into the
owner's registered root, which is its main checkout.

`rename` rewrites every record under its own preconditions (clean `tasks/`, at most one
worktree) and is not a save of one record, so it is outside this rule.

The CLI never copies records between checkouts (fresh-worktree brief). Every remedy is a
command or a merge that the person or agent runs.

### 3.3 Exit status and JSON

`stale_copy` is a new error kind and exits 1, like every other typed error. The error object
keeps its shape, `{"error": {"kind", "detail"}}`. The kind is additive and needs no other
contract change.

## 4. The claim follows its holder

A claim records `worktree` when it is acquired. In the prescribed order (commit the record,
then `git worktree add`), `start` runs in main, so the claim names main while the work
happens in the worktree.

Rule: **a successful save by the holder of a live claim on that task, when the save leaves
the claim in place, sets the claim's `worktree` to this checkout's root, and `seen` to
now.** Holder means what `note`'s
heartbeat already means: `Ctx::ownership` returns anything but `Foreign`. That includes
`ByIdentity` (the resolved session matches) and `ByProof` (under relay identity, the claim's
recorded process proof names this caller). The refresh keeps every other field of the claim,
as the heartbeat does. When ownership cannot be established, the claim is left alone, and
`note` keeps its existing warning for that case.

- `note` already refreshes `seen` on the holder's claim, and now also sets `worktree`.
- Field edits, `dep`, `attach`, `detach` and a feedback recurrence (explicit or automatic)
  by the holder gain the same refresh. A recurrence lands in the owner's registered root, so
  that is where the claim moves. The store
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
copy is still current. The worktree's next write refuses and leads with the merge remedy
(§3.2 rule 3): commit main's copy, merge it into the branch, then rerun. The step only
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

An id from another registered project already routes to that project's registered root.
The fallback applies there the same way: it reads that prefix's claim store, and the named
checkout must be a checkout of that prefix.

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
- **A `-C` retry into another session's worktree.** A worktree's copy becomes the newest
  whenever its work touches the record, including in passing. Until that branch merges,
  such a retry would send writes from every other checkout into someone else's uncommitted
  tree. §3.2 rule 1 names the session instead.
- **An override flag.** `--force` already carries two meanings. The escape from a refusal is
  to write in the newer checkout, or to merge or remove it.
- **Comparing content instead of stamps.** Rejected by the work-claims design as noise, and
  that reasoning is unchanged.

## 8. Known gaps

- **Worktrees holding a newer copy, abandoned or active.** A worktree with a newer copy
  blocks writes to that record from every other checkout until its branch merges or the
  worktree is removed. That includes an active worktree whose work touched the record only
  in passing, such as a wake-note. When a session works there, the refusal names it and
  gives no retry into its branch (§3.2 rule 1). This is the accepted cost of "refuse always".
- **Same-second forks are caught by content, not stamps.** Equal stamps with different
  bytes refuse (§3.1). What remains is a sibling copy that cannot be read: it is compared
  by neither stamp nor content, and warns.
- **A write in the wrong checkout.** A write that landed in the wrong checkout before
  anything was newer (no §5 step) still needs a manual merge. §3 makes the next write
  report it, leading with the merge remedy (§3.2 rule 3); it does not undo it.
- **Invisible worktree-only tasks.** A task that exists only in a worktree, with no claim or
  park, is still invisible to `show` elsewhere. Nothing names its checkout.
- **Checkouts outside git's worktree list.** A second project root under the same prefix
  that is not a linked worktree (for example, `init --force` in another clone) is not
  compared under §3, although claims and parks may name it.
- **Other machines.** Nothing crosses machines, as the work-claims design already records.

## 9. Documents that change

- Work-claims design §Warnings, "A newer copy elsewhere": becomes the refusal, with this spec
  cited, and gains the same-stamp content rule (§3.1). Its "fires once per divergence"
  limit goes, because the refusal leaves nothing written. The "never refuses" sentence and
  its rationale in `warn_on_newer_sibling_copies` go.
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
  `-C`, and `attach` with a relative path, which the retry resolves unchanged.
- `attach` from a path, from stdin, and from a stale copy refuses and leaves no file or
  directory under `tasks/files/<id>/`.
- `edit --body -` and `attach -` from a stale copy refuse, and the detail asks for the same
  stdin. Running the printed retry with that input succeeds.
- The `$EDITOR` path, with a sibling that becomes newer while the editor is open, refuses
  after the editor closes, keeps the temp file, and names it.
- A feedback recurrence onto a record whose copy is behind refuses, both through
  `--recur <id>` and through an automatic title match, and neither appends anything.
- `detach` of a name the ledger already records as detached, with the leftover file still
  present, from a copy that is behind: it refuses and the file is still there.
- Another session's live claim, on a different task, naming the worktree that holds the
  newer copy: the refusal names that task and session and prints no `-C` retry. The
  caller's own claim naming it keeps the retry.
- A park by another session naming that worktree also suppresses the retry.
- The tasks-142d2f sequence without the §5 step: `start` in main, commit, `git worktree
  add`, a note in main, then a write in the worktree. The detail leads with the merge
  remedy, and the `-C <main>` retry follows as the alternative.
- A record left uncommitted before `git worktree add` (`start` in main, not committed, then
  `tasks start` in the new worktree): the detail leads with the merge remedy, not
  `tasks -C <main> start`.
- Equal stamps with different bytes refuse, with the merge remedy only. Equal stamps with
  equal bytes do not refuse.
- `show` of another project's worktree-only task, from outside that project, falls back
  through that project's claim store.
- The tasks-142d2f sequence: `start` in main, `git worktree add`, `tasks start` in the
  worktree, then `note` in main. The note refuses and names the worktree, and the claim
  names the worktree.
- A holder's `note` and `edit` in a worktree move `claim.worktree` there. Another session's
  `note` leaves it alone. A claim held `ByProof` (relay identity, with the explicit
  `TASKS_SESSION` pair unset) moves the same way.
- `show` in main for a task added and started in a worktree renders it with the warning. A
  parked worktree-only task also renders. A claim naming a removed worktree gives
  `task_not_found` with the checkout named.

## 11. What the ideas get

When this spec is approved, in the same commit as closing tasks-ab8d2d:

- tasks-2c0a1d: closed by §3 (refuse, naming the newer checkout and the retry).
- tasks-bb53e5: covered by §3 together with the §5 step. Without the step, the write in main
  is caught at the worktree's next write instead.
- tasks-fbc32b: `show` covered by §6.1. The `list` half is declined in §6.2.
