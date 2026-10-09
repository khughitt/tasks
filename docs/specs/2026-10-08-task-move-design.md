# Moving one task between projects

Status: approved 2026-10-09 after review round 3; implementation plan next (tasks-bf6b9b)
Task: tasks-bf6b9b (lane tasks-a7460c; answers tasks-7ba557)
Brief: `docs/notes/2026-10-08-project-discovery-movement-brief.md`

## 1. Goal

Moving a task to another registered project today means `add --project <dst> --source
<old id>` followed by `drop <old id>`. The tracker's own records show what that loses.
Nine such moves exist across the registered projects (five `dots` → `wali`, one `wali` →
`glean`, three `mind3` → `mind6` at retirement). None of the nine destinations kept the
source's notes: each starts empty or with a hand-written `moved from <old id>`, and the
sources held one to four notes each. None had attachments, a parent, or an inbound
dependency, so the add/drop workflow was never tested on them. A drop also counts as
closed, so any task that depended on the source would have become ready without the
work being done.

This design adds `tasks move <id> --to <prefix>`. It creates the task in the
destination under a new id, carrying its history verbatim, and leaves the source as a
closed, read-only **tombstone** that points to the new id.

Success:

- After `tasks move dots-5760ef --to wali`, `wali-<new>` holds the task's fields, notes
  (original times, authors and provenance), attachments and park. `dots-5760ef` shows
  `status: moved` and `moved_to: wali-<new>`.
- A task that depended on `dots-5760ef` stays blocked until `wali-<new>` closes.
- Every write to `dots-5760ef` refuses and names the new id.
- An interrupted move resumes on rerun with the same destination id and never makes a
  second copy.

## 2. Decisions

| Question | Decision | Rejected |
|---|---|---|
| Must the old id resolve transparently? | No. The tombstone answers reads with a pointer; writes refuse (user decision, 2026-10-08). | Per-task forwarding: needs a synced authority and a cross-host contract, and silently routes writes. |
| Identity | Fresh random hex in the destination, recorded in the move journal before any write. | Keeping the hex (`wali-5760ef`): can collide, and reads like a prefix alias, which keeps the hex by definition. |
| Inbound `depends` | Not rewritten. Dependency evaluation follows the pointer; `check` warns with the repair command; `move` reports the dependents it can see. | Rewriting reachable dependents: writes uncommitted changes into N other repositories (some retired, claimed, or mid-rename) and still leaves unreachable dependents needing the pointer. |
| Parent and children | Refuse. A task with a parent or with children does not move. | Detaching or re-parenting during the move; moving subtrees. Both are out of scope until a move of a goal is needed. |
| Spec/plan links, needs | Refuse when the record does not validate in the destination. | Rewriting links across repositories, or override flags on `move`. The source is still live, so `tasks edit` settles each refusal first. |
| Old workflow | `add --source` + `drop` stays valid for a deliberate re-file (a changed task, not the same one). | Removing it. |

## 3. Behaviour

### 3.1 Command

```
tasks move <id> --to <prefix> [--explain]
```

- `<id>` is any accepted id form (alias prefix, bare suffix, trailing periods).
- `<prefix>` names a registered project; a retired-prefix alias resolves to its live
  name. The destination must differ from the source.
- `--explain` classifies the move (fresh, resume, complete, refuse) and reports what it
  would do, without locks or writes, like `rename --explain`.

`move` writes records in two repositories and commits neither. The output names the
changed paths under each root; each root gets its own commit.

### 3.2 What moves

The destination record is the source record with these changes:

| Field | Destination |
|---|---|
| `id` | `<dst>-<new hex>` |
| `moved_from` | new key: the source id |
| `updated` | the move time |
| notes | every source note, verbatim and in order (`at`, `by`, provenance), then one stamped note `moved from <src id>` |
| everything else | unchanged: title, status, priority, size, complexity, process, parallel, lane, needs, every, defer, owner, created, started, last_done, depends, tags, source, model, agent, body |

- Same-prefix `depends` become foreign references to the source project. They already
  resolve, so they stay valid, and the cycle check runs across projects as usual.
- The body and note text are not rewritten. Ids in prose keep their old spelling, as in
  a prefix rename.
- Attachments listed live in the ledger are copied to `tasks/files/<new id>/`. Their
  ledger notes are name-only, so they carry over unchanged.
- A park and an escalation keyed by the source id, in this host's claim store, move to
  the destination store under the new id. The carried park keeps its owner, session,
  host, time, next step, waiting party, reason, needs, minutes and title. Its `worktree`
  is set to the destination project's `resume_root`. The source's checkout would be the
  wrong place to resume: `list --parked` and `quiet` offer that path, and only a
  checkout of the destination project can run `tasks start <new id>`.

The tombstone is the source record with:

- `status: moved`
- `moved_to: <new id>`
- `updated` set to the move time
- one stamped note `moved to <new id>`
- one `detached: <name>: moved to <new id>` note per live attachment, with the bytes
  removed

### 3.3 What refuses

Every refusal happens before the first write and leaves no journal. Each one names the
command that settles it.

| Condition | Kind | Settled by |
|---|---|---|
| Source closed (`done`, `dropped`, `moved`) | `validation` | Nothing: closed history stays where it happened. |
| Source has a parent | `validation` | `tasks edit <id> --no-parent`; re-parent in the destination afterwards. |
| Source has children | `validation` | Move or re-parent each child first. |
| Any live claim on the source, the caller's included | `claimed` | `tasks park <id> …`. The park then travels with the task. |
| A spec, plan, or need that does not validate in the destination | `validation` (or `unknown_need`) | `tasks edit <id> --no-spec/--no-plan/--rm-need …`, or declare the need in the destination. |
| Destination holds a record with `moved_from: <src>` or `source: <src>` | `validation` | Inspect that record. It is an earlier move or a manual re-file: drop one of them, or keep the re-file and drop the source. |
| Destination unregistered, unreachable, or the same project | `config` | Register or choose another project. |
| Source or destination retired | `retired_project` | Unretire, or move before retiring. |
| A pending rename or move on either project, or on this task | `move_pending` / existing rename refusal | Resume or finish that operation. |
| Source changed under a pending journal (§4.3) | `move_pending` | Manual recovery (§4.4). |

The destination record goes through the destination's normal write validation (tags,
needs, docs, lanes, depends). Whatever that refuses, `move` refuses.

### 3.4 The tombstone and references to it

- **Reads.** `show` prints the tombstone with its pointer. `list` and `ready` treat
  `moved` as closed, so it appears only under `list --status moved`.
- **Writes.** Every write command that takes an id refuses on a tombstone with the new
  kind `moved`, naming the new id and the command to run instead: `note`, `edit`,
  `dep`, `attach`, `detach`, `start`, `park`, `shelve`, status changes, and `move`
  (unless it is completing its own move, §4).
- **New references.** `dep --on <tombstone>` and `--parent <tombstone>` refuse with
  `moved`. A new reference must name the live id.
- **Dependency evaluation** (`done`, `ready`, `next`, `prime`, halts): a dependency on a
  tombstone evaluates the record its `moved_to` names, following the chain. If the chain
  revisits an id or reaches an unreachable record, the dependency counts as open and
  carries a warning, as an unreachable dependency does today. This is one-way forwarding
  for reads; nothing routes writes.
- **Cycle detection follows the same pointer.** In the dependency graph, a tombstone's
  only edge is to its `moved_to` record; its own retained `depends` are history, not
  edges. Both cycle walkers use these edges: the write-time walk (`dep --on`,
  `add`/`edit --depends`, `ensure_acyclic`) and `check`'s. Otherwise, with B depending
  on A and A moved to C, `dep C --on B` would pass. The walk would follow A's retained
  `depends` instead of A → C, so C → B → A → C would be left permanently blocked. A move
  never creates a cycle itself: the graph after the move is the graph before it, with
  A renamed to C.
- **`check`** warns `moved_dep` on every record that depends on a tombstone. The warning
  names the swap: `tasks dep <id> --on <new> --rm <old>`.
- **`move`'s output** lists `inbound`: the records in reachable registered projects that
  depend on the source, under any spelling. Dependents in projects this host cannot
  reach are not found. They are still correct, because they follow the pointer once the
  destination is reachable from where they are evaluated.
- **`tasks resolve`** is unchanged. It maps prefixes and paths, not individual tasks;
  tools that need the pointer read `moved_to` from `show`.

### 3.5 Output

```json
{"id": "wali-b0882b", "from": "dots-5760ef", "recovery": "fresh",
 "attachments": ["shot.png"], "park": true, "inbound": ["ops-1a2b3c"],
 "paths": {"<dots root>": ["tasks/dots-5760ef.md", "tasks/files/dots-5760ef/shot.png"],
           "<wali root>": ["tasks/wali-b0882b.md", "tasks/files/wali-b0882b/shot.png"]},
 "warnings": []}
```

`recovery` is `fresh`, `resumed`, or `complete` (a rerun after success, which writes
nothing). `--explain` returns the same shape with `recovery` set to the verdict,
including `refuse` with the reason in `warnings`.

The task JSON gains optional `moved_to` and `moved_from`, and `status` gains the value
`moved`. These are the JSON contract changes this task approves.

## 4. Write protocol

A move writes two repositories, the claim store, and attachment bytes. None of these can
be written atomically together, so the move follows the prefix rename's pattern: a
host-local journal, a pure classifier, and forward recovery.

### 4.1 Journal

The journal lives at `$XDG_STATE_HOME/tasks/move/<src id>.toml`: outside both
repositories, so `git checkout .` cannot erase it. It records:

- `source`, `target` (the full new id), `source_root`, `target_root`
- `source_text` and `source_digest`: the exact bytes of the source file at preflight
  and their sha256. Undo restores from this text, never from git, whose committed copy
  can differ from the file the move read.
- `target_text` and `tombstone_text`: the exact bytes to write, rendered at preflight
  with one move time, so a resume writes identical files
- `attachments`: `{name, sha256}` for each live attachment
- `source_park` and `source_escalation`: the source store's entries for the source
  id, verbatim, if any. The park keeps its original `worktree`, which may be a code
  worktree of the source project. Undo returns exactly these.
- `target_park` and `target_escalation`: the entries M5 writes under the new id,
  rendered at preflight from the source entries. The park's `worktree` is replaced with
  the destination's `resume_root` (§3.2), and nothing else changes. M5 compares against
  these, so a resume writes identical entries.

### 4.2 Phases

Every phase runs under three locks, taken in the order `rename` uses:

1. both prefixes' mutation locks, in sorted order;
2. the registry lock.

Under them the move reloads the registry and re-resolves both roots. If either root
differs from the one the move resolved before locking (or, on resume, from the
journal's), it releases the locks and retries, up to four times. Then it fails with
`io`, as rename does. A resume whose registered roots no longer match the journal
refuses rather than writing to a project that has moved.

| Phase | Action | Idempotent because |
|---|---|---|
| M1 | Preflight (§3.3); no writes | — |
| M2 | Write the journal (first mutation) | The journal's presence decides fresh versus resume. |
| M3 | Copy each attachment to `tasks/files/<new>/`; verify its sha256 | A file that is present with the right digest is skipped. |
| M4 | Create the destination record exclusively at the journal's id from `target_text` | A file that is present with that digest is skipped. |
| M5 | Write `target_park`/`target_escalation` into the destination store under the new id | An entry that is present and equal is skipped. |
| M6 | Write the tombstone from `tombstone_text` | A file that is present with that digest is skipped. |
| M7 | Remove the source attachment bytes and the source store entry | Absence is success. |
| M8 | Remove the journal (last) | — |

`TASKS_MOVE_STOP_AFTER=journal|attachments|destination|store|tombstone|cleanup` stops
after a phase for tests, as `TASKS_RENAME_STOP_AFTER` does.

### 4.3 Classification and freeze

`classify(snapshot)` is pure. It reads the journal, both record files (their digests),
the attachment files, and the store entries, and returns one of `Fresh`, `Resume(phase)`,
`Complete`, or `Refuse(reason)`. The table is closed: a state it does not name refuses.
In particular:

- The source digest differs from both `source_digest` and the tombstone's digest
  (someone edited the source by hand mid-move): refuse.
- The destination file exists with a digest other than the target's: refuse.
- A journal attachment has no copy with its journal sha256, in either the source or the
  destination directory: refuse, naming the attachment. The journal holds digests, not
  bytes, so no rerun can rebuild it. The journal stays, and with it the freeze, until a
  verified backup is restored (§4.4).
- No journal, a tombstone whose `moved_to` names a destination record whose `moved_from`
  points back: `Complete`.

While a journal names a task, every write to the source id or the new id refuses with
`move_pending`. Reads proceed, and `show` and `check` warn that a move is pending.

The journal also freezes both projects' lifecycle. Each command that can change a
project's registration, root, prefix, or writability refuses with `move_pending` while
a journal on this host names that prefix as source or destination:

- `rename` (either side, including `--adopt`);
- `retire` and `unretire`;
- `unregister`;
- `init --force` re-pointing the prefix.

Without the freeze, these would leave the resume facing states §3.3 refuses. A source
stopped after M6 has no open task left, so `retire` would succeed and M7's cleanup
would then hit `retired_project`. `unregister` or a re-pointed root would leave the
journal naming a root the registry no longer routes to. These are the commands that
call `reject_pending_rename` today. They share one pending-operation check that reads
both rename inventories and move journals.

### 4.4 Recovery limits

- **Process interruption.** Rerun the same `tasks move`. It resumes with the journal's
  id and texts.
- **Other hosts.** The journal is host-local, but records sync. `check` compares each
  `moved_from` and `moved_to` with its counterpart record, without reading a journal.
  It separates a counterpart that is absent from one it cannot reach:

  | Counterpart | Finding |
  |---|---|
  | Project reachable, record present and pointing back | none |
  | Project reachable, record missing, or present but not pointing back (a destination whose source is not a tombstone naming it, or a tombstone naming a missing destination) | error `move_incomplete`, naming both ids: finish the move on the host that started it |
  | Project unregistered or unreachable on this host, or its record unparsable | warning `move_counterpart_unreachable`, naming the project; nothing is claimed about the move |

  A successful move whose destination is later unregistered therefore warns, and does
  not report an incomplete move. Dependencies on the tombstone stay blocked meanwhile
  (§3.4).
- **Manual undo** (the journal still present, after an interrupted move). Restore
  first, delete last, and verify every restoration before deleting anything:
  1. Restore the source record from the journal's `source_text`, and confirm its
     sha256 equals `source_digest`.
  2. Make each journal attachment present in `tasks/files/<src id>/` with its journal
     sha256. Copy it back from the destination copy when the source copy is gone. If
     neither copy has the journal's digest, stop: keep both directories and the
     journal, and report the name.
  3. Return the journal's `source_park` and `source_escalation` verbatim into the
     source store, keyed by the source id, and confirm
     it reads back equal.
  4. Only now delete the destination store entry, the destination attachment
     directory, and the destination record.
  5. Remove the journal last.

  If any step fails, stop with everything in place and keep the journal. A rerun of
  `tasks move` can complete the move forward instead, but only when every journal
  attachment still has a copy with its journal digest. If step 2 stopped because
  neither copy matches, neither direction can finish from what is on disk. Restore that
  file from a verified backup (git history, another host's synced copy) whose sha256
  equals the journal's. Until then, a rerun refuses, the journal stays, and both
  projects stay frozen.
- **Power loss** is out of scope, as it is for rename (no fsync).
- **Parks on other hosts.** Claim stores are per host. A park for the source id recorded
  on another host stays keyed to the tombstone there. This is the same limit as
  tasks-912279 (a park surviving a close on another host) and is solved there, not here.

## 5. Compatibility

- **Old binaries.** A binary older than this change fails to parse a record with
  `status: moved`, `moved_to`, or `moved_from`, as it does with note provenance. Every
  host that reads the synced records needs a current `tasks` before the first move.
- **The retire procedure.** The skill's "close every task (drop, or re-file in the
  successor with `--source <old id>`)" gains `tasks move <id> --to <successor>` as the
  way to keep an open task. A retired project refuses writes, so moves happen before
  `retire`.
- **The skill.** The tasks skill documents `move`, the tombstone, `moved_dep`, and that
  inbound references are repaired by their owners with the swap `check` prints.

## 6. Acceptance cases

Each case runs in temporary repositories with scratch `XDG_CONFIG_HOME` and
`XDG_STATE_HOME`.

1. **Inbound foreign dependency and attachment.** `ops-x` depends on `dots-a`, and
   `dots-a` has `shot.png`. Run `move dots-a --to wali`.
   - `wali-<n>` carries all notes byte-identically, plus `moved from dots-a`.
   - `wali/tasks/files/wali-<n>/shot.png` has the source digest.
   - The `dots-a` tombstone carries `detached: shot.png: moved to wali-<n>`, and its
     bytes are gone.
   - `inbound` is `["ops-x"]`. The `ops-x` file is unchanged.
   - `ops-x` is not ready, and becomes ready once `wali-<n>` is done.
   - `check` in `ops` warns `moved_dep` with the swap command.
   - `check` passes in `dots` and in `wali`.
2. **Parented task.** `dots-c` has parent `dots-g`.
   - `move` refuses with `validation`, naming `dots-g` and
     `tasks edit dots-c --no-parent`. Nothing is written and no journal exists.
   - After that edit, the move succeeds.
   - `move dots-g` refuses while `dots-g` has children.
3. **Live claim.**
   - Another live session's claim refuses with `claimed`, and nothing is written.
   - The caller's own claim refuses too, naming `tasks park`.
   - After a park, the move carries the park. `prime` and `list --parked` in `wali` list
     `wali-<n>` with the same next step, and the `dots` store no longer holds it.
   - The carried park's `worktree` is `wali`'s `resume_root`, not `dots`'s.
   - `quiet` names the `wali` checkout as the place to resume.
   - `tasks start wali-<n>` there resumes the task.
4. **Destination collision.** `wali` holds a record with `source: dots-a` (an earlier
   manual re-file). `move dots-a --to wali` refuses with `validation`, naming that record.
   Nothing is written.
5. **Interrupted retry.** Set `TASKS_MOVE_STOP_AFTER=destination`.
   - After the stop, the destination exists, the source is unchanged, and the journal
     exists.
   - `note` on either id refuses with `move_pending`, and `move --explain` reports
     `resume`.
   - With scratch state but the same records, `check` reports `move_incomplete`.
   - A rerun completes with the same new id.
   - A second rerun reports `complete` and writes nothing.
   - A hand edit to the source before the rerun makes the rerun refuse.
6. **Tombstone writes.**
   - `note`, `edit`, `start`, and `dep --on` on `dots-a` refuse with `moved`, naming
     `wali-<n>`.
   - `show dots-a` prints the pointer.
7. **Unavailable counterpart after a completed move.** Unregister `wali` after the
   move.
   - `ops-x` stays not ready, with the unreachable-dependency warning, and nothing
     treats the tombstone as satisfied.
   - `check` in `dots` warns `move_counterpart_unreachable` naming `wali`, and reports
     no `move_incomplete`.
   - After re-registering `wali`, both findings are gone.
8. **Cycle through a tombstone.** `dots-b` depends on `dots-a`; move `dots-a` to
   `wali-<n>`.
   - `dep wali-<n> --on dots-b` refuses with `cycle`, naming
     `wali-<n> -> dots-b -> dots-a -> wali-<n>`.
   - A record written by hand with that edge makes `check` report the same cycle.
9. **Lifecycle frozen by an interrupted move.** `dots` holds only `dots-a`; set
   `TASKS_MOVE_STOP_AFTER=tombstone`.
   - `retire` in `dots` refuses with `move_pending`, although `dots` has no open task.
   - `unregister wali`, `unregister dots`, `init --prefix wali --force` from another
     directory, and `rename dots dx` each refuse with `move_pending`.
   - Neither the registry nor either project changed.
   - A rerun completes the move, and `retire` in `dots` then succeeds.

10. **Undo restores the original park.** Park `dots-a` from a code worktree of `dots`
    (`dots/.worktrees/x`), then move it with `TASKS_MOVE_STOP_AFTER=store`.
    - The journal's `source_park.worktree` is the `dots/.worktrees/x` path, and its
      `target_park.worktree` is `wali`'s `resume_root`.
    - The destination store holds `target_park`.
    - Run the §4.4 undo steps from the journal. The `dots` store then holds a park for
      `dots-a` equal to the original, `worktree` included, and the `wali` record,
      directory, and store entry are gone.
11. **Lost attachment bytes.** Stop with `TASKS_MOVE_STOP_AFTER=cleanup`, after M7
    removed the source bytes, then delete the destination copy too.
    - A rerun and `--explain` both refuse, naming the attachment.
    - The journal remains, and writes to both ids and `retire` still refuse with
      `move_pending`.
    - Restore the file with the journal's digest, rerun, and the move completes.

## 7. Out of scope

- Moving a goal, a subtree, or several tasks as one operation (a shell loop over
  single-task moves serves the five-task case in tasks-7ba557).
- Moving closed tasks.
- Rewriting references in other projects, in prose, or in documents.
- Moving or copying spec and plan documents.
- A synced, cross-host move journal.
