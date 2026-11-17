# curate: the whole-project sweep

A sweep gives every eligible open task of one project exactly one verdict, closes the
stale and duplicate ones itself, and ends in one report. It is started by hand:

    /curate --all [--project <prefix>]
    /curate --resume <sweep-task>

Everything in [SKILL.md](SKILL.md) holds unless this file says otherwise: the verdicts
and per-task steps (§3), and
[Closing stale and duplicate tasks](SKILL.md#closing-stale-and-duplicate-tasks), whose
message forms, duplicate procedure, guards, proposal placement and undo lines this
file cites and does not restate. In every form there, `<pass>` is the sweep task's id.

Terms: the **controller** is the session running the sweep; a **worker** reviews one
cluster; the **sweep task** is the task that owns this sweep; the **pool** is the set of
tasks the snapshot printed; the **ledger** is the snapshot file the controller fills.

## The swept root

Fix the root once, before anything else, and run every later `tasks` command, and every
command in a worker brief, as `tasks -C <swept root> ...`:

- `/curate --all` with no project: apply the unscoped rule of [SKILL.md §2](SKILL.md#2-one-root-per-task)
  (`tasks where`). Then run `tasks resolve <prefix>` for the prefix `where` printed; if
  the canonical path (`realpath`) of the root §2 chose is not that of the result's
  `root`, the session is in a code worktree: stop and ask the user to rerun from the
  main checkout or with `--project <prefix>`.
- `--project <prefix>`: run `tasks resolve <prefix>`. Unless it prints exactly one
  result with `status` `resolved`, stop and report its output; never guess a root
  (`tasks root` refuses a bare prefix). The swept root is that result's `root`.
- `--resume <sweep-task>`: the swept root is what `tasks --pretty root <sweep-task>`
  prints.

Then run `tasks -C <swept root> where` and keep `.project.prefix`, `.project.records`
and `.project.checkout`. The sweep task is added, started and parked through the swept
root too, so its park names the project's main checkout and not a worktree that may be
removed.

## Open

Each sweep is its own new task in the swept project, started by the session running
it. It owns the ledger, the report and the park between sessions. A recurring curation
task is not reused: its output references would pile up across sweeps.

    tasks -C <swept root> add "Curation sweep of <prefix> <YYYY-MM-DD>" --process direct [--tag curation] --body "<body>"
    tasks -C <swept root> start <sweep-id>

- Add `--tag curation` only when the `[tags]` table of the project's config (the
  `entry.config` path in `tasks resolve <prefix>`) has a `curation` key.
- The body says the task runs curate's whole-project sweep, that process is `direct`
  because this file fixes the procedure, and where the ledger and report live.
- Start it before the snapshot: a started task is `doing` and so is never in its own
  pool.

`<sweep-id>` below is this task's id. A sweep resumed with `--resume` skips this step
and runs `tasks -C <swept root> start <sweep-id>`.

## Where the ledger and report live

Their names are fixed, with `<YYYY-MM-DD>` the UTC date of the snapshot, so they do not
change across sessions:

    docs/notes/<YYYY-MM-DD>-curation-sweep-<sweep-id>-ledger.tsv
    docs/notes/<YYYY-MM-DD>-curation-sweep-<sweep-id>-report.md

They live under records `docs/notes/` when that is an authorized private records home:

- `tasks resolve <prefix>` gives `entry.kind` `detached` or `linked`: yes. The paths
  above are relative to `.project.records`.
- `entry.kind` `in-tree` with `entry.profile` `work` or `external`: no. That records
  directory is a code repository that may not hold operational data.
- `entry.kind` `in-tree` under any other profile: yes only when the profile or the
  project's own instructions name `docs/notes/` as a home for reports; otherwise no.

When they do not live in `docs/notes/`, they live uncommitted in the sweep task's
persistent storage under the same file names
(`$WORK_ROOT/<prefix>/<sweep-id>/<YYYY-MM-DD>-curation-sweep-<sweep-id>-ledger.tsv` and
`$WORK_ROOT/<prefix>/<sweep-id>/<YYYY-MM-DD>-curation-sweep-<sweep-id>-report.md`, with
`<prefix>` = `.project.prefix`), recorded as retained items of the sweep task under the
tasks skill's
[artifact placement and retention](../tasks/SKILL.md#artifact-placement-and-retention)
rules. On a host without `WORK_ROOT`, ask the user where before the snapshot.

Output references: in the `docs/notes/` case only, add both to the sweep task once the
snapshot has fixed the date:

    tasks -C <swept root> edit <sweep-id> --ref 'ledger (output, kind=ledger): docs/notes/<YYYY-MM-DD>-curation-sweep-<sweep-id>-ledger.tsv — one row per pool task'
    tasks -C <swept root> edit <sweep-id> --ref 'report (output, kind=report): docs/notes/<YYYY-MM-DD>-curation-sweep-<sweep-id>-report.md — closures, decisions, signals against verdicts'

In the persistent-storage case add no `--ref`: the retained item's note is the link.

## Snapshot

Capture `<snapshot-stamp>` (`date -u +%Y-%m-%dT%H:%M:%SZ`) immediately before running
the signals script (using its UTC date as `<YYYY-MM-DD>`), then resolve the script
through the registry and run it once by explicit path, never from `PATH` or the current
directory:

    tasks resolve tasks        # one result, status resolved; keep its root
    python3 "<tasks root>/bin/sweep-signals" --root <swept root> > <ledger>.tmp && mv <ledger>.tmp <ledger>

It prints the ledger: a header row, then one tab-separated row per pool task. The pool
is what the script printed: every task with status `idea`, `todo` or `blocked` and no
live claim, deferred tasks included, with no age window. `doing` and `shelved` tasks are
out. A non-zero exit prints nothing on standard output and names the failing command on
standard error: delete the `.tmp` file, stop and report that command. Tasks filed after
`<snapshot-stamp>` (not in the ledger) are out of this sweep and counted in the report.

Record that pre-run stamp on the sweep task; the report and a resumed session read it:

    tasks -C <swept root> note <sweep-id> "sweep: snapshot <snapshot-stamp>; pool <n>; ledger <path>"

### The ledger's columns

The script fills the first thirteen columns; the controller fills the last five:

| Column | Meaning |
|---|---|
| `id` | the pool task |
| `status`, `updated`, `reviewed`, `awaiting_user`, `deferred_until`, `parked` | what the snapshot read: status, last update, the scope reviewed mark, the waiting-on-user mark, the deferral date, whether it carries a park |
| `dead_paths` | named paths that exist in neither the code checkout nor the records directory (trigger) |
| `cited_closed` | cited task ids closed after this task was last updated (trigger) |
| `deps_closed` | `true` when every dependency is closed (trigger) |
| `touched_commits` | commits since `updated` that touch a named path (orders clusters; never a trigger) |
| `twins` | other pool tasks whose titles share at least 0.4 of their words (a hint for clustering) |
| `triggers` | how many of `dead_paths`, `cited_closed`, `deps_closed` fired |
| `cluster` | the cluster label, written at clustering |
| `verdict` | one of `keep`, `refined`, `stale`, `duplicate`, `decision`, `decompose`, `skipped`, `left`, `pending`, `unwritten` |
| `action` | one of `none`, `edited`, `closed`, `proposed` |
| `evidence` | one line with no tab: the reason for the verdict, as below |
| `decided` | the UTC date (`YYYY-MM-DD`) the row was settled |

Signals are hints and their absence clears nothing. No task is closed, or skipped, on
a signal alone.

The controller is the ledger's only writer. It rewrites the file in place, keeps the
header and the first thirteen columns as the script printed them, and replaces any tab
or newline in a value with a space. The snapshot stamp is for coverage only: a pool
task that has changed since is read as it is now.

The next sweep: set against this ledger, a later sweep's pool shows which tasks are
unchanged since they were last judged (same `updated`, no new trigger). Carrying such a
row forward without a re-read is a later decision; every sweep under this file re-reads
its whole pool. Keep every column, so the ledger holds what that decision needs.

What each verdict writes to the row:

| Verdict | Action | Evidence |
|---|---|---|
| `keep` | `none` (`edited` when modified only to absorb an in-pool duplicate) | one phrase on why it is fine (plus `absorbed <id>` when it absorbed a duplicate; `for scope pass: <why>` when a reviewed idea needs a decision or scoping) |
| `refined` | `edited` | what changed, since a `refined` edit writes no note (plus `absorbed <id>` when it also absorbed a duplicate) |
| `stale`, `duplicate` closed | `closed` | the drop message's evidence; for a duplicate also the survivor, what was folded in and the dependents re-pointed; then `undo: <status>[ reviewed][ awaiting_user][ deferred <date>]` as recorded before the drop |
| `stale`, `duplicate` a guard kept open | `proposed` | `guard: <which>` and the proposal; for a task `doing`, claimed or parked, add `ledger only` (nothing was written to it) |
| `decision`, `decompose` | `proposed` | the proposal text of its note |
| `skipped` | `none` | the revalidation that failed |
| `left` | `none` | why it left the pool: started, claimed, parked or closed since the snapshot |
| `pending` | `none` | the pending proposal, relayed in the report |
| `unwritten` | `none` | `parked` or `idea waiting on the user` |

## Twins, then clusters

1. **Twins.** The controller reads every pool title once (`tasks -C <swept root> list --status idea --status todo --status blocked`, filtered to ids in the ledger so live-claimed and post-snapshot tasks are excluded) together with the `twins` column, and
   lists suspected duplicate pairs: script candidates it judges plausible and pairs it
   finds itself. For each pair it writes `suspect twin: <other id>` into both rows'
   `evidence`, so a resumed session still hands the pair to its worker.
2. **Clusters.** Suspected twins always share a cluster; otherwise group by lane or goal
   subtree, then by tag or subsystem, then by the files and commands the tasks name. Aim
   for 10 to 25 tasks a cluster: split larger groups, and put leftovers in mixed
   clusters. The grouping is the controller's judgment.
3. **Write it.** Write each row's label to the `cluster` column as `<nn>-<slug>`, with
   `<nn>` the cluster's run position from [Order](#order) (two digits) and `<slug>` a
   short lowercase hyphenated subject (`03-claims`).

Finding twins before clustering means no task is reworded and then closed as a
duplicate.

## Order

Clusters run in order of how many trigger signals their tasks carry, most first: the
sum of the `triggers` column over the cluster's rows. Break a tie by the sum of
`touched_commits`, most first. A sweep that stops early has then done the part most
likely to hold stale tasks.

## One cluster

One worker per cluster: a subagent where the harness has one, the controller itself
otherwise. Run one worker at a time.

The worker brief gives:

- the cluster's ledger rows, header included;
- the paths of curate's `SKILL.md` and of this file, as this session loaded them;
- the sweep task id, which is `<pass>` in every message form;
- the swept root, `.project.records` and `.project.checkout`, and the rule that every
  `tasks` command is `tasks -C <swept root> ...` and every `git log` or code search runs
  in the checkout;
- the five rules below, and the sections
  [When deeper evidence is owed](#when-deeper-evidence-is-owed), [Ideas](#ideas),
  [Tasks the sweep does not write](#tasks-the-sweep-does-not-write) and
  [Rows that left the pool](#rows-that-left-the-pool).

The worker reads every task in the cluster (`tasks -C <swept root> show <id>`, keeping
`task.updated`) and the documents they link, then follows SKILL.md §3 for each with
these five rules:

1. **Closures before rewording.** Settle duplicates and stale tasks first; reword
   second.
2. **Deeper evidence where owed**, as the next section says.
3. **One verdict** from curate's set (`keep`, `refined`, `stale`, `duplicate`,
   `decision`, `decompose`), narrowed for ideas as [Ideas](#ideas) says, acted on as
   [Closing stale and duplicate tasks](SKILL.md#closing-stale-and-duplicate-tasks) and
   SKILL.md §3 steps 5 and 6 say.
4. **Revalidate** before the first write exactly as SKILL.md §3 step 4 says, against the
   worker's own read, and skip a task that became active, claimed or changed since that
   read, with the reason in its row. A duplicate closure revalidates every task it
   writes, not only the one it closes.
5. **Never fold in twice.** Before a duplicate's fold-in, read the survivor's notes and
   finish only what an earlier session of this sweep left undone. SKILL.md writes the
   fold-in, the origin edit and its note, the re-pointed dependents and the absorb note,
   in that order, and the drop last:
   - The survivor carries `curate: absorbed <closing-id>; <sweep-id>; ...`: the fold-in,
     origin edit and earlier re-pointing are done. Re-run the guards of
     [Closing stale and duplicate tasks](SKILL.md#closing-stale-and-duplicate-tasks)
     (including `tasks -C <swept root> show <closing-id> --all-projects` and the custody
     checks): if a guard now holds, do not drop — record the guarded proposal as that
     section says (`ledger only` when `doing`, claimed or parked). Otherwise re-point any
     open dependent added since the absorb note (revalidating it and the closing task
     first), revalidate the closing task, and drop it.
   - No such absorb note, but the survivor's body already holds what only the closing
     task said: the fold-in is done; do not fold in again. Re-run those guards and
     revalidate every task the remaining steps write; if a guard holds, record the
     guarded proposal instead. Otherwise write the origin edit and its note, where
     SKILL.md calls for one, unless the survivor already carries
     `origin: <user|mixed> after absorbing <closing-id>; <sweep-id>; ...`, re-point each
     open dependent the closing task's `dependents` field still lists, then write the
     absorb note and the drop.
   - Otherwise the closure starts from the fold-in.

Scope of a worker's writes:

- The tasks it is reviewing are its cluster's pool tasks. It closes only those, and
  writes otherwise only to a duplicate's survivor and open dependents as the closing
  section allows. A pool survivor modified only to absorb a duplicate gets verdict
  `keep`, action `edited`, and `absorbed <closed-id>` in `evidence` (`refined` and
  `edited` when it also received `refined` edits of its own).
- An out-of-pool task (filed after the snapshot, `doing`, `shelved`, or live-claimed at
  the snapshot) is never closed by the sweep. When the out-of-pool task is the survivor,
  the pool duplicate is closed into it unless a guard in the closing section holds (a
  `doing`, live-claimed or parked survivor holds the guard and is recorded as
  `proposed`). When the pool task is the survivor, its verdict is `keep` or `refined` and
  the out-of-pool task goes in the report's proposed closures with `not in pool` where
  SKILL.md writes `not sampled`.
- An overlap (not a duplicate) with a task outside its cluster is said in its own
  task's body only and named in that row's `evidence`; the controller carries it into
  the report.
- A suspicion that a task's twin sits in another cluster is not acted on.

The worker does not act across clusters and does not write the ledger. It returns, in a
file the controller names (or its final message where it has no file):

1. one line per cluster row, tab-separated: `id`, `verdict`, `action`, `evidence`,
   `decided` (values as the verdict table says; `evidence` is one line with no tab);
2. its cross-cluster twin suspicions, one per line:
   `<id in this cluster> <-> <other id>: <why>`;
3. every write it made, as the exact command, in order;
4. for each closure, the guard checks it ran and their results;
5. CLI friction met, one line each, for the controller to file.

Items 3–5 give the controller the audit trail for [First closures](#first-closures) and
the report's CLI-friction list without reading worker transcripts; they add no worker
authority and are not written to the ledger.
The controller writes the rows, appends `cross-cluster twin: <id>` to the
`evidence` of each row a suspicion names, and at each cluster boundary commits the
ledger and the task records the cluster wrote:

- Ledger, `docs/notes/` case:
  `tasks -C <swept root> commit <ledger> -m "chore(curate): sweep <sweep-id> cluster <nn>"`,
  with the ledger's records-relative path, unless the project's profile or own
  instructions leave records uncommitted.
  Persistent-storage case: nothing to commit.
- Task records: as [SKILL.md §4](SKILL.md#4-commit-per-touched-project) says. Never push.

## When deeper evidence is owed

A worker gathers deeper evidence (code, `git log`, a reproduction) for a task when one
of its trigger signals fired, when it is a suspected twin, or when its premise is a
claim about current behaviour, as a `friction` or `gap` feedback report's is (a
`positive` or `idea` report claims no defect). Otherwise the record and its links are
the evidence.

## Ideas

A sweep never scopes: it does not promote, brief or mark an idea reviewed.

- For an idea the verdicts are `keep`, `stale`, `duplicate` and `refined`. `refined` is
  limited to wording, links and tags: no size, complexity or process, and no
  `decision` or `decompose`.
- A reviewed idea that needs a decision or scoping is `keep` (or `refined`), with
  `for scope pass: <why>` in its row, and is listed in the report for a scope pass.
- An unreviewed idea (`reviewed` is `false`) is read for `stale` and `duplicate` only
  and is otherwise not edited, so it stays unreviewed for scope, whose triage also
  redacts feedback reports. Its other verdict is `keep`.
- If an unreviewed idea is the survivor of a duplicate, the fold-in is the recurrence
  form only (the `from:` tag and a one-line note, which is the absorb note), with no
  rewrite of its body; its own row is `keep` with action `edited` and
  `absorbed <closed-id>` in `evidence`.

## Tasks the sweep does not write

- **Held.** A pool task that carries a park (`parked` is `true` at the snapshot and the
  park is still there at the worker's read), or an idea whose `awaiting_user` is `true`
  at the worker's read (whether marked before or after the snapshot), gets a ledger row
  with verdict `unwritten`, no write, and a line in the report. Any later note answers a
  pending proposal, so a stray sweep note would also silently clear a decision the user
  still owes.
- **Pending proposal.** A task whose latest note starts `curate:` or `scope:` and
  contains `proposal:`:
  - If the proposal is to drop it as stale or duplicate, re-check the evidence; when it
    holds and no guard applies, carry it out (verdict `stale` or `duplicate`, action
    `closed`).
  - Otherwise write nothing to the task: verdict `pending`, and the proposal is relayed
    in the report.
  - A proposal note that names this sweep's id is this sweep's own work, from a session
    that stopped: it is a verdict (`decision`, `decompose`, or a guarded `stale` or
    `duplicate`, action `proposed`), not a pending row.

## Rows that left the pool

A row whose task, at the worker's first read, has been started, claimed, parked or
closed since the snapshot gets verdict `left`, the reason in `evidence`, and no write
(an idea marked `awaiting_user` since the snapshot is **Held** above: `unwritten`; when
a survivor read in the cluster already carries `curate: absorbed <id>; <sweep-id>; ...`,
rule 5 of [One cluster](#one-cluster) records `duplicate`, `proposed`, `ledger only`
instead, and otherwise `left` likewise leaves the task untouched). A task that passes
that read but fails revalidation before the write is `skipped`. A task closed by a drop message naming
this sweep, or whose latest note is a proposal naming this sweep, has not left the pool:
it is this sweep's own verdict, recorded as
[Stopping and resuming](#stopping-and-resuming) says.

## Cross-cluster pass

When every cluster has run, the controller settles the suspicions the workers returned
(the `cross-cluster twin:` entries in `evidence`), reading the candidates in full before
acting. It reviews both tasks of each pair as a worker would, with its own read for
revalidation and rule 5 of [One cluster](#one-cluster) before any fold-in, updates both
rows (committing the ledger in the `docs/notes/` case if any row changed), and records
`tasks -C <swept root> note <sweep-id> "sweep: cross-cluster pass done"`.

## First closures

In a project's first sweep (no done task in the project has a title starting
`Curation sweep of `), the controller verifies every closure of the first cluster
before the second runs: each drop message's evidence is present and true (the commit
exists and does what it says, the cited task or document says what is claimed), and for
a duplicate the survivor holds the fold-in and the absorb note and the dependents point
to it. When the check passes (or the first cluster closed nothing), record
`tasks -C <swept root> note <sweep-id> "sweep: first closures verified; cluster <nn>"`
before stopping or starting the second cluster. A failed check stops the sweep and goes
to the user: park the sweep task with `--waiting-on user --reason decision`, naming the
closure and what failed.

## Stopping and resuming

**Stopping.** Stop only at a cluster boundary, after the ledger commit (and after
[First closures](#first-closures) on cluster `01` of a first sweep). Record the session
(including `first-cluster tokens <count, or unknown>` in session 1) and park the sweep
task with the next cluster — or `cross-cluster pass and Close` when `0` rows lack a
verdict — as its next step:

    tasks -C <swept root> note <sweep-id> "sweep: session <n>; clusters <first>-<last>[; first-cluster tokens <count, or unknown>]; tokens <count, or unknown>"
    tasks -C <swept root> park <sweep-id> "Run /curate --resume <sweep-id>: next cluster <nn>-<slug>, <k> rows without a verdict"

In the persistent-storage case, account for the ledger as a retained item at the park.

**Resuming.** The ledger is the map: the remainder is the rows with no `verdict`. Run
`tasks -C <swept root> start <sweep-id>` and find the ledger and notes on `<sweep-id>`.
In a project's first sweep, if cluster `01` is settled and `<sweep-id>` lacks a
`sweep: first closures verified` note, run [First closures](#first-closures) before
continuing. Then, if any row has no `verdict`, continue with the first such cluster;
when every row already has a `verdict`, continue at
[Cross-cluster pass](#cross-cluster-pass) if `<sweep-id>` lacks
`sweep: cross-cluster pass done`, at [Close](#close) step 1 (**Retry**) if it lacks
`sweep: retry done`, and at [Close](#close) step 2 (**Check**) otherwise.

Every cluster that has any row without a verdict is reconciled from the records before
it runs, whether some or all of its rows lack one: a worker can stop after its writes
and before returning, and a controller can stop before writing the rows it got. Every
drop message and note this sweep writes names the sweep task id, so for each row
without a verdict the controller reads the task (`tasks -C <swept root> show <id>`) and
its notes:

- a drop message naming this sweep: verdict `stale` or `duplicate`, action `closed`,
  the drop message's evidence, and an `undo:` record built from the row's snapshot
  columns (`status`, `reviewed`, `awaiting_user`, `deferred_until`), since the drop
  cleared those fields on the record;
- a latest note that is a proposal naming this sweep: its verdict (`decision`,
  `decompose`, or a guarded `stale` or `duplicate`), action `proposed`, not a pending
  row;
- anything else: no verdict yet. A half-finished duplicate closure leaves nothing on
  the closing task, and a `refined` edit leaves no note, so these rows go back to the
  worker, whose rule 5 finishes a closure whose fold-in reached the survivor without
  folding in twice.

Then the worker re-reads each remaining task of that cluster.

## Close

In this order:

1. **Retry.** Tasks skipped at revalidation get one retry: review each `skipped` row's
   task again, as a worker would; a second skip stays `skipped`. Record
   `tasks -C <swept root> note <sweep-id> "sweep: retry done"` once the pass finishes
   (committing the updated ledger in the `docs/notes/` case if any row changed).
2. **Check.** Run `tasks -C <swept root> check`. Fix a finding the sweep's own writes
   caused; name any other in the report.
3. **Report.** Write the session note (as at a stop), then the report.
4. **Commit.** In the `docs/notes/` case, commit the final ledger and the report
   together, unless the project's profile or own instructions leave records
   uncommitted:

       tasks -C <swept root> commit <ledger> <report> -m "chore(curate): sweep <sweep-id> report"

   with both paths records-relative (`docs/notes/...`). Commit the task records the
   retry wrote as [SKILL.md §4](SKILL.md#4-commit-per-touched-project) says. Never push.
   In the persistent-storage case, account for the ledger and report as retained items
   instead.
5. **Close** the sweep task:
   `tasks -C <swept root> done <sweep-id> "<counts>; report <path>"`.

## The report

At the report path. Its sections, in this order:

- **Counts:** pool size, each verdict, skipped, left, pending, and filed since the
  snapshot (tasks of any status in the project not in the ledger whose `created` is at
  or after `<snapshot-stamp>`: `tasks -C <swept root> list` with every `--status`).
- **Closed:** every closed task with its id, title, prior status, stale or duplicate,
  the evidence, the survivor where there is one, and its undo lines as
  [Closing stale and duplicate tasks](SKILL.md#closing-stale-and-duplicate-tasks)
  defines them (from the row's `undo:` record). Closures of tasks carrying the user's
  words (origin `user` or `mixed`, read as that section reads it) come first.
- **Decisions left to the user**, grouped as curate's
  [summary](SKILL.md#summary-to-the-human) groups them, with the guarded closures
  (including `not in pool`), and the relayed pending proposals.
- **Left for a scope pass and unwritten:** the unreviewed ideas, the reviewed ideas the
  sweep judged to need a decision or scoping, and the pool tasks left unwritten because
  they are parked or waiting on the user.
- **Doing without a claim or park:** `doing` tasks with neither a live claim nor a park,
  from `tasks -C <swept root> list --status doing` (rows with no `claim.live` true and
  no `park`). The pool excludes them and no other review reads them.
- **Signals against verdicts:** for each trigger (`dead_paths`, `cited_closed`,
  `deps_closed`), how many tasks it flagged and how many of those ended `stale`,
  `duplicate` or `refined`; and how many `stale`, `duplicate` or `refined` verdicts
  carried no trigger.
- **Cost:** clusters, sessions, the first cluster's per-cluster token figure, and total
  tokens where the harness showed them, from the sweep task's session notes.
- **CLI friction** met during the sweep, each also filed as feedback to `tasks` when it
  was met (`tasks feedback --project tasks "<one line>" --category friction`).

The message to the user at the end gives the counts, the decisions, and a link to the
report. It does not list every row.

## Limits

- One project per sweep. A sweep writes only its own project's records; run it once per
  project.
- A sweep is ordinary interactive use. It stays inside the session's turn limits by
  stopping at a cluster boundary and resuming.
