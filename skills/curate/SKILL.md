---
name: curate
description: Use when asked to curate, review, tidy, or audit the task corpus (`/curate`), or to sweep a whole project's open tasks (`/curate --all`, `/curate --resume`). Samples random open tasks and runs one bounded maintenance pass over them, or reviews every open task of one project in clusters; never part of the session protocol.
---

# curate

One pass improves the tasks it draws. It creates no tasks and changes no priority. It
refines wording, links and metadata in place, closes a stale or duplicate task itself
under the guards in
[Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks), and proposes
new tasks, priority changes, open questions, missing children and guarded closures to
the human at the end. Invoke deliberately:

    /curate [n] [--project <prefix> | --group <name> | --all-projects]
    /curate --all [--project <prefix>]
    /curate --resume <sweep-task>

Default: three tasks from the current project. The last two forms are the whole-project
sweep: for either, read [sweep.md](sweep.md) beside this file and follow it; a sampled
pass does not read it.

## 1. Sample

    tasks sample --limit <n> [--project <prefix> | --group <name> | --all-projects]

The pool is already the right one: `idea`, `todo`, or `blocked`; no live claim; no
proposal still awaiting the human; not updated in the last 7 days. Read the warnings: a
shortfall names the pool size, each live-claim omission names the holder, and each
`<id> pending: <proposal>` line is a decision the human still owes (relay it in the
summary; do not open the task).

## 2. One root per task

`sample --project`, `--group`, and `--all-projects` read registered roots, but `show`,
`edit`, and `note` prefer the current checkout when the id's prefix matches it. From a
worktree you could sample one copy of a task and rewrite another. So, before touching a
sampled task, fix its root and run **every** later command for it as `tasks -C <root> ...`:

- `sample` ran unscoped: run `tasks where` to discover the project, even without a
  config in code. For `kind: "in-tree"`, use `.project.root`, including a project
  nested below the git top level. For `detached` or `linked` invoked in its code
  worktree, use that worktree's top level (from `git rev-parse --show-toplevel`);
  otherwise use `.project.root`. Keeping the code worktree as the command context
  preserves detached document fallback there.
- `sample` ran with `--project`, `--group`, or `--all-projects`: the root is the path
  that `tasks --pretty root <id>` prints. The default JSON form is an object; its `root`
  field is the same path. Never pass the JSON to `-C`. Run `tasks -C <root> where`.

Keep `.project.records` and `.project.checkout` from `where` separately. Grep code
and run `git log` in each sampled task's `checkout`. For an unscoped sample invoked
in a detached or linked project's code worktree, use the chosen `<root>` instead.
Working documents live under `records`; read their resolved paths from
`show`, or use `tasks -C <root> doc <id> --spec|--plan|<handle>`. A detached project's
retained reference document may resolve in the code tree instead.

## 3. Per task

1. **Read.** `tasks -C <root> show <id>`; keep `task.updated`. Read the spec, plan, and
   parent it links to. `tasks -C <root> tree <id>` when it has children.
2. **Evidence.** Grep the code and docs the task names. Does the described thing already
   exist in the tree? If so, find the commit (`git log -S` or `git log --grep`). Search
   for a probable duplicate with a distinctive phrase from the task:
   `tasks -C <root> search "<phrase>"` matches open titles and bodies.
   For a goal, check that its children cover it.
3. **Verdict**, exactly one:
   - `keep`: clear, current, correctly linked; nothing to change.
   - `refined`: prose or links improved; meaning unchanged.
   - `stale`: *Delivered* (the described thing is in the tree; evidence: the commit) or
     *Premise gone* (what the task assumed is no longer true, such as a removed command or
     a replaced design; evidence: the commit, closed task or document that removed the
     premise). Closed, not proposed, unless a guard in
     [Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks) holds. A
     task delivered only in part is not stale: rewrite it to the remainder, with the
     delivered part and its commit stated, as a `refined` edit. An ask that another task or a design explicitly declined, or placed out of
     scope, is neither delivered nor covered: on that ground the task is not `stale` and
     not a `duplicate`.
   - `duplicate`: another open task in the same project covers every ask of this one.
     Closed after the fold-in, unless a guard in that section holds. Two tasks that
     only overlap are not duplicates: say so in both bodies and keep both (the other
     body only when the pass may write that task; otherwise name the overlap in the
     summary). A design task and the task that implements it are not duplicates.
   - `decision`: not actionable without a choice the human owns. Write the questions into
     the record (step 5); the summary asks.
   - `decompose`: a goal is missing children it needs. Proposal: the children, one line
     each.
4. **Revalidate**, immediately before the first write: `tasks -C <root> show <id>` again.
   Skip the task, writing nothing, and report the reason if any of these hold:
   - `task.status` is not `idea`, `todo`, or `blocked` (became active);
   - `claim` is present with `live: true` (claimed; a stale claim does not count);
   - `task.updated` differs from the stamp taken in step 1 (changed).
   This is a check, not a lock. A session that starts the task in the seconds between
   the recheck and your write gets an edit or a closure on a task it holds. Accepted.
   A duplicate closure revalidates more than the sampled task: immediately before its
   first write it re-reads the closing task, the survivor and each open dependent, and
   if any fails, nothing is written and the closure is skipped with the reason (see
   [Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks)).
5. **Edit**, only through the CLI, only these:
   - `--title`, `--body`: rewrite for clarity and brevity. Keep every fact. State the
     assumptions the text implies. Collect questions you cannot answer under one
     `## Open questions` heading at the end of the body; reuse an existing one. Notes
     are stored apart from the body and survive the rewrite.
   - `--spec`, `--plan <topic> --step "<heading>"`, `--parent`: fix when the linked
     thing exists and the link is missing or wrong. The document may live under
     `records` (see [document resolution](../../docs/documents.md#working-documents-and-resolution));
     keep paths such as `docs/specs/<name>.md` and `docs/plans/<name>.md`
     relative to that directory. A missing dependency is added with
     `--depends`; a wrong one is replaced with `tasks dep <id> --on <right> --rm <wrong>`.
   - `--ref` with `--body`: promote an existing prose path to a reference entry and
     rewrite the prose to cite its handle as `[ref:<handle>]`, preserving why the
     document matters. This is a bounded `refined` edit; use the entry syntax and
     relations in the tasks skill, reusing an existing handle when present.
   - `--size`: set or correct.
   - `--complexity`: set or correct against the rubric in the tasks skill (low: approach
     established, context identified, clear check; mid: bounded choices remain, scope
     clear; high: discovery, interacting behaviour, or an open architectural call).
   - `--process`: fill a missing choice with evidence. Use direct when the task or
     reviewed plan settles outcome, approach, and verification; planned when written
     design and plan reviews are needed. Never infer it from size, complexity,
     parentage, or document presence; a link is not evidence of approval. Record the
     reason in the `curate: refined` process note (step 6). This does not change
     status or expand the sample, and an unassessed todo does not require a backfill
     sweep.
   - `--tag`: add a tag the project already uses (`tasks -C <root> tags`) when it
     clearly applies. Never invent one.
   Never: status, priority, `--parallel`, `--source`, `add`, or `tasks/*.md` by hand.
   `drop` only as [Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks)
   says, and `--origin` only on a duplicate's survivor, as that section says.
6. **Note.** Only what a later reader of the task needs. `<pass>` is the sweep task's id
   in a sweep and the words `sampled pass` otherwise. Run each form as
   `tasks -C <root> ...` (§2).
   - `keep` writes nothing.
   - `refined` makes its edit and writes no note. The exception is a filled `--process`,
     whose reason the tasks skill requires on the record:

         tasks note <id> "curate: refined; <pass>; process <direct|planned>: <reason>"

   - A closed `stale` or `duplicate` task's evidence is its drop message; a survivor gets
     one absorb note (see [Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks)).
   - `decision`, `decompose`, and a `stale` or `duplicate` task a guard keeps open (other
     than `doing`, claimed or parked tasks, which stay in the ledger/summary only with no
     write to the task) open a named decision instead of a legacy `proposal:` note:

         tasks -C <root> decision open <id> --kind <drop|suggestion> --origin curate --text "<verdict>; <pass>; <what changed, or guard: <which>>; <proposal>"

     Use `--kind drop` for a guarded drop proposal (`stale` premise gone, guarded duplicate)
     and `--kind suggestion` for `decision`, `decompose`, or a "close as delivered"
     suggestion. Reuse the same `--id <uuid>` on retry.

   No ordinary note or drop message contains a dedicated `proposal:` segment.

## Closing stale and duplicate tasks

Closing is `tasks drop <id> "<message>"`, never an edit of status. The message starts
`curate: stale;` or `curate: duplicate;`, names the pass, and carries the evidence, so
the reason is in the record. Run each form as `tasks -C <root> ...` (§2):

    tasks drop <id> "curate: stale; <pass>; delivered: <commit> <one line>"
    tasks drop <id> "curate: stale; <pass>; premise gone: <commit, task id or document> <one line>"
    tasks drop <id> "curate: duplicate; <pass>; of <survivor-id>; folded in: <what, or nothing>"

A pass closes only a task it drew: in a sampled pass, a sampled task.

Before a *premise gone* or *duplicate* closure, read
`tasks show <id> --all-projects` for the closing task. Its `dependents` field and any
warning that a registered project is unreachable decide the guards below and the
re-point step.

Before the drop, record the task's status, review marks (`reviewed`, `awaiting_user`)
and deferral date for the undo: the drop clears them.

**A duplicate.** Before the drop:

- Pick the survivor: the task with more settled structure (scoped, linked to a spec or
  plan, parented, depended on) stays; on a tie, the older. A task that carries
  attachments is the survivor regardless: attachments and `files:` references resolve
  only on their own task, and curate does not attach. When the survivor is the task
  the pass drew and the other was not drawn, the pass closes nothing: the drawn
  task's verdict is `keep` or `refined`, and the other task is named in the summary's
  **proposed closures** with `not sampled` in place of a guard.
- Fold in anything only the closing task says: facts, a reproduction, the original
  report text, as a `--body` edit of the survivor. A duplicate feedback report folds in
  as a recurrence: its `from:` tag (`--tag`) and a one-line note on the survivor, which
  is the absorb note below.
- Re-point every open task that depends on the closing one to the survivor:
  `tasks dep <dependent> --on <survivor-id> --rm <closed-id>`. Dependents are the
  `dependents` field of `tasks show <id> --all-projects` read above; `search` does not
  read dependencies. That field lists closed dependents too. They are history and are left
  as they are: re-pointing one would rewrite a finished record and leave it depending
  on open work.
- Keep the user's ask visible. When the closing task carries the user's words (read as
  the last guard below reads them) and the survivor's origin is `agent` or unset, set
  the survivor's origin to what the merged record now is by the tasks skill's
  definition: `user` when every ask in it is the user's (a survivor whose origin is
  unset only because it predates the field, and which holds no ask an agent added),
  `mixed` otherwise. Write `tasks edit <survivor-id> --origin <user|mixed>` with the
  stamped evidence note the tasks skill requires for an origin correction (below).
  Without this the ask leaves the goals report, and a later pass could close the
  survivor on a "premise gone" judgment the guard was meant to stop. This is one more
  permitted edit, for this case only.
- Revalidate everything the closure writes. Take each one's `task.updated` when you
  first read it. Immediately before its first write the pass re-reads the closing task,
  the survivor and each open dependent, and applies §3 step 4 to each (status — `shelved`
  is also allowed on the survivor and open dependents — live claim, unchanged since read;
  a claim that cannot be verified reads as live). If any fails, nothing is written and
  the closure is skipped with the reason; a sweep retries it at its close.

Then write, in this order: the fold-in, the origin edit and its note, the re-pointed
dependents, the absorb note, and the drop last:

    tasks note <survivor-id> "origin: <user|mixed> after absorbing <closed-id>; <pass>; <evidence>" --stamp
    tasks note <survivor-id> "curate: absorbed <closed-id>; <pass>; <what was folded in>; dependents re-pointed: <ids, or none>"

**Guards.** The pass does not close, and records a proposal instead, when:

- the task is `doing`, has a live claim or a park, has open children, is a lane, is
  tagged `halt`, or recurs (`every`);
- the task has unsettled retained custody: off-tree evidence, a plan workspace or a
  worktree it still owns, or incoming custody another task named it to receive. `drop`
  does not refuse such a task and `check` stays clean, so the closure would leave the
  item without an open owner. Read the state with
  `tasks -C <root> retained state <id> --all-projects` before a closure, not by searching
  notes for `kept`: an item kept in one note may be released in a later one, and another
  task (across any registered or retired project, including across `moved_from`/`moved_to`
  chains, retired-prefix aliases, `[predecessor]` displaced suffixes and
  `[retention_lineage]`) can owe `kept` custody naming this task as `next_owner` with no
  `kept` note on `<id>` itself. Bare `retained state <id>` (`incoming.scope == "project"`)
  never clears curation closure. The guard holds whenever
  `tasks -C <root> retained state <id> --all-projects` refuses, `complete` is `false`,
  `incoming.scope` is not `"all_projects"`, `incoming.complete` is not `true`, `items`
  contains an effective `kept` item, `incoming.items` is non-empty, or `operations` has
  an unfinished retention operation. If `tasks -C <root> retained state <id> --all-projects`
  refuses (including an unrecognized `--all-projects` option on a secondary host that
  synced `tasks` `main` before rebuilding), treat custody as unresolved (the guard holds)
  and rebuild the host's binary with `just install` in the `tasks` main checkout before
  retrying. The guard holds for a duplicate too: moving custody to the survivor is a
  retained transfer, which a curation pass does not do;
- closing a duplicate would need a write to a survivor or open dependent that is
  `doing`, live-claimed or parked, or to an open dependent in another project, or to a
  survivor that is itself held: it has an open named decision (`decisions` with
  `state: "open"`), its latest format-1 note is a pending proposal, or it is an idea
  waiting on the user. The absorb note would interfere with that pending decision;
- both tasks of a duplicate pair carry attachments;
- the verdict is *premise gone* and the task has open dependents. A dropped dependency
  counts as satisfied, so the drop would offer agents work whose prerequisite will
  never exist. *Delivered* with dependents is closed: releasing them is correct;
- the verdict is *duplicate* or *premise gone* and `show --all-projects` warned that a
  registered project is unreachable. It still exits 0, and the dependents it lists may
  then be incomplete;
- the verdict is *premise gone* and the task carries the user's words: origin `user`
  or `mixed`, or, where origin is unset, an occurrence note labelled `user` or `mixed`,
  which is the tasks skill's reading rule. Whether the user still wants something they
  asked for is theirs to say. Delivered work and exact duplicates of such tasks are
  still closed.

Where the proposal is recorded: for a task that is `doing`, claimed or parked, in the
sweep's ledger or the pass's summary only, and nothing is written to the task. For
every other guard, open a named decision on the task (§3 step 6) with `guard: <which>`
in its text.

**Undo.** A closed task reopens only to `todo`, and any status change clears its review
and deferral fields. The summary, or a sweep's report, prints for each closed task the
lines that apply, in this order, from what the pass recorded before the drop:

- `tasks edit <id> --status todo`;
- for a task that was an idea or blocked, then `tasks edit <id> --status idea` or
  `--status blocked` (a closed task cannot go to either directly);
- for an idea that was reviewed, then `tasks note <id> "<why it was reopened>"
  --reviewed`, with `--awaiting-user` if it had that mark;
- for a task that was deferred, then `tasks edit <id> --defer <date>`.

For a duplicate it also lists the dependents re-pointed to the survivor and what the
survivor absorbed: undoing the closure does not move those back. The original review
date cannot be restored.

Nothing is closed for being low value, unclear, old or unlikely. Those are not verdicts
the user granted.

## 4. Commit per touched project

For each project whose records the pass edited:

- Under `[git] commit = true`, the CLI commits each write as it lands; read every
  `commit_deferred:` or `commit_failed:` warning and resolve it before finishing with
  `tasks -C <root> commit <id>...` for the affected task IDs (avoid bare `tasks commit`
  when unrelated uncommitted tasks exist).
- Otherwise, commit only the task records the pass wrote in that project's records
  repository with `tasks -C <root> commit <id>... -m "chore(tasks): curate pass"`,
  unless that project's profile or own instructions decide commits differently, in which
  case leave them uncommitted and list the uncommitted paths in the summary.
- Never push.

## Pending proposals

`sample` never draws a task with an open named decision (format 2) or a format-1 task
whose most recent note is a `curate:` or `scope:` note carrying a dedicated `proposal:`
segment; it reports the task as `pending` with the proposal text instead. `tasks list
--pending` lists the full queue of named decisions and legacy waits. Relay those lines
in the summary and move on. Named decisions survive incidental notes and attachments and
are answered with `tasks decision answer <id> <uuid> --accept|--decline ...` (with
`--seen-scope <token>` for drop acceptance) or withdrawn with `tasks decision withdraw`.
On legacy format-1 records, any later note clears a latest-note proposal (`--no-reviewed`
on an idea so scope looks again). A `refined` edit moves the task out of the pool for
the age window, as any write does. A kept task is unwritten and can be drawn again
before the age window passes; reviewing it again is the maintenance.

A curate note does not readmit an idea for scope: only `tasks note <id> --no-reviewed`
does, so write that form only when the note is new evidence that warrants another
scope pass.

## Bounds

- Zero new tasks per pass. Curation improves what exists; adding is a proposal.
- Prefer shorter. A rewrite that grows the body without adding a fact is wrong.
- Touch only the sampled tasks, with one exception: closing a sampled duplicate writes
  to the survivor and to open dependents, which may lie outside the sample, and only as
  [Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks) says. It never
  closes an unsampled task: a duplicate found in passing, including one whose survivor
  is the sampled task, is named in the summary, not edited.

## Summary to the human

One line per sampled task: id, verdict (or `skipped: <reason>`), one phrase of what
changed. It is the only trace of `keep` and of a `refined` edit that wrote no note. Then
one `pending: <proposal>` line per pending warning `sample` printed; then each touched
project's commit or why none was made (listing any uncommitted paths left by
the project's profile or instructions). Then what the pass closed and the decisions that
are the human's, grouped, omitting empty groups:

- **closed**: id, stale or duplicate, the evidence, the survivor of a duplicate, and
  the undo lines (see [Closing stale and duplicate tasks](#closing-stale-and-duplicate-tasks));
- **proposed closures**: id, stale or duplicate, the evidence, and the guard that kept
  it open (or `not sampled`);
- **priority changes**: id, from, to, why;
- **children to add**: the goal, then one line per child;
- **questions**: id, then the open questions a `decision` verdict wrote into the record.

Nothing else. A pass with nothing in those groups says so in one line.

## What a good task reads like

- The title names the outcome, not the activity.
- The body answers why, what done looks like, and where to look. It repeats nothing the
  linked spec already says.
- Assumptions are sentences, not implications. Unknowns are open questions, not hedges.
- A body is as short as those three answers allow.

## Kinds

Passes so far have met three task shapes. Each adds to the three answers above; a task
that fits none is held to those alone. A shape is not a verdict: stale and duplicate
are findings about a task of any shape.

- **Defect**: something behaves wrongly now. The body says what happens against what
  should, with a reproduction and the date it was last seen true, and where in the code
  it happens. Done names the behaviour change, including any command that succeeds
  today and will fail, and the test that pins it. A feedback report is a defect whose
  premise is unverified: reproduce it on the current tree first, then keep the original
  report verbatim at the end of the body. If a later commit already fixed the premise,
  any remaining gap (such as an error message that does not name the fix) is the task;
  otherwise it is stale.
- **Decision**: the work waits on a choice the human owns. The body names the options
  and what each costs or implies in the code. Alternatives already rejected keep their
  reasons. The questions the human must answer go under `## Open questions`. Once
  decided, the body is rewritten into the shape the outcome takes.
- **Open idea**: a thought not yet framed as options. The body says why it matters and
  who would use it first, what already exists to build on, and what is still unknown, as
  open questions.

Two checks apply to every shape. Facts that drift (counts, sizes, word totals, "still
true") carry the date they were measured; re-measure them and keep the old figure dated
beside the new one. A wait written in prose ("once X lands") belongs in a `depends`
edge too: add it when missing, and when the awaited work has closed, say so in the body
instead of leaving the wait standing.

A research task filed by a scope pass already has its shape: keep the five fields of
[the scope skill's §5](../scope/SKILL.md#5-make-follow-ups-answerable) intact. Goals and
plan steps are left out: no pass has drawn one yet, and plan headings already template a
step.

When recommending a new continuation follow-up for scope, name its explicit follows
target and actual filing source separately, using the tasks skill's relation table.
A citation alone remains related. This adds no maintenance write permission: do not
backfill/retype links, and leave filing or relation changes to the scoped owner.
