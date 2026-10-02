# Worktree audits and blocked bootstrap

## Problem

Task worktrees should retain the latest record and support isolated repairs. This
batch contrasts a successful stale-copy handoff (tasks-c1636f, since dropped) with two gaps:
halt-override audit writes into a sibling checkout (tasks-7ad96f), and a pre-existing
content failure preventing the task-record commit required before isolation
(tasks-850488). Goal: tasks-0e7216.

## Current behaviour and evidence

- `load` and `refuse_stale_copy` in `src/commands/mod.rs` reject newer siblings and
  equal-stamp/different-content forks, naming the checkout and remedy. This landed
  under tasks-9949f3 in `7054f6f`. tasks-c1636f reports that the named session and
  exact rerun resolved a refusal in one step; it requests no additional behavior.
- `src/halt.rs::snapshot` reads registered-checkout authority. In
  `src/commands/status.rs::start`, override notes go directly through
  `snapshot.authority().write_task`, explicitly bypassing the sibling guard. Code
  confirms the write path; this pass did not reproduce the reported active-halt
  failure or same-stamp fork. Existing tests cover authority-only audit records
  and failed target writes, but not the reported newer active halt copy.
- `.githooks/pre-commit` selects a task/guide-only recipe by staged paths. The recipe
  retains hygiene and task consistency; `tools/ops-check` examines tracked content.
  Thus a task-only commit does not necessarily escape an unrelated content failure.
  The incident came from ops (tasks-850488 is `from:ops`), whose classifier is wider:
  ops `docs_paths` is `*.md docs/* tasks/*` and its docs-only recipe runs
  `bin/ops-check`, `bin/ops-docs check` and `tasks check`, so a task-only commit
  there pays `ops-docs check` over every tracked doc. That is the likeliest incident
  gate. The original incident command and gate output are unavailable.
- `skills/tasks/SKILL.md` requires committing the record before worktree creation.
  The older fresh-worktree brief recommended that order and rejected CLI copying.
  tasks-5a46b9 delivered it. The reported staged-patch workaround is evidence of
  friction, not an approved general exception.

## Constraints

Keep registered-checkout halt decisions, durable attempted-audit notes before a
claimed start, mutation locking, and stale-copy refusal. `time::after` already fixed
ordinary sequential same-second handoffs in `ec41eb1`; changing stamps alone does
not settle a writer that bypasses sibling comparison.

Preserve exact task provenance, existing notes, and unrelated working changes during
bootstrap. No hook bypass, silent task copying, or undocumented setup command.
Shared hooks belong to ops, global instructions to tack, and the shipped workflow
to tasks; this pass writes only local handoffs.

The earlier record-home and bootstrap goals are closed. Their briefs contain
historical evidence. The record-home spec header, which still said round 3 review,
was corrected in this pass to accepted and implemented; current code and completed
task records establish the delivered behavior here.
No overlapping open research or design follow-up was found. tasks-9b0a2e's linked
project layout is related future context, not a prerequisite.

## Alternatives

1. **Keep the contracts and preflight secondary audit writes.** Refuse safely when
   registered authority is behind. This is the smallest halt option to evaluate,
   but its remedy must permit progress without discarding the halt owner's notes.
2. **Separate halt authority reads from audit write ownership.** Select the current
   record explicitly while preserving incident visibility and audit ordering. This
   needs reviewed routing and failure semantics; it cannot be a silent fallback.
3. **Retain normal bootstrap and define one evidenced exception.** First establish
   whether a compliant existing path works, starting with the route the instructions
   already allow: on the user's explicit work-in-place instruction, repair the content
   failure in place, commit it, then commit the record and branch. If none does, propose a narrowly reviewed
   isolated-repair handoff rather than disabling gates or adding a copying service.

Prefer existing safeguards and the smallest explicit changes that satisfy both
contracts. The halt and bootstrap questions can progress independently.

## Unanswered questions

- Where should an attempted audit land when the halt's active copy differs from
  authority, and what does a safe refusal tell each owner to do? tasks-10c968 will
  propose the ownership and failure contract for user review.
- Which check blocked the original bootstrap, and is a compliant recovery already
  available? Answered by tasks-cee8d9 (below); the original incident gate remains unknown
  without its capture.

## Bootstrap finding (tasks-cee8d9)

Hook selection and content failure are separate. In both repositories a commit staging
only `tasks/*.md` selects the docs-only recipe: here `tools/ops-check && tasks check`;
in ops `stage_docs_cmd`, then `bin/ops-check`, `bin/ops-docs check` and `tasks check`.
Selection only drops the code checks. The retained checks read the **working tree** and,
for cross-project references, the **host's tasks registry**, never the staged commit. A
record-only commit can therefore be refused for three different reasons:

- **A. Content already committed in HEAD** (for example a dead backticked path in a guide).
  Any worktree branched from HEAD inherits it, so the repair has to land anyway.
- **B. Unrelated uncommitted edits** in tracked files the checks read (a person's draft
  README). HEAD is clean; the commit is refused for content it does not contain.
- **C. Host environment.** `ops-check` resolves ``<prefix> `path` `` through the tasks
  registry; with the prefix unregistered it resolves against the current repository and
  reports `` `path` does not exist `` without naming the project. Seen incidentally when
  the scratch clone ran with an empty registry: ops's README references into tack and
  tasks failed until the real registry was supplied. Filed to ops as ops-d50aa5.

Controlled run: a disposable clone of ops at `3808a07`, isolated `XDG_CONFIG_HOME` and
`XDG_STATE_HOME` (registry copied from the host, `ops` re-pointed at the clone),
`core.hooksPath=.githooks`, one CLI-created record (`tasks add`, `start`, `note`), and an
unrelated unstaged edit to `bin/vendored`. The planted failures were committed with hooks
disabled as setup only.

    # A: README.md carries a dead `docs/specs/no-such-design.md` in HEAD
    $ git add tasks/ops-f12d86.md && git commit -m "chore(tasks): start ops-f12d86"
    README.md:318: `docs/specs/no-such-design.md` does not exist
    error: recipe `hook-pre-commit-docs` failed          # no commit; record stays staged
    $ <repair README.md in place>
    $ git commit -m "fix(docs): drop broken reference" -- README.md   # pathspec: record stays staged
    $ git commit -m "chore(tasks): start ops-f12d86"                  # record alone, passes
    $ git worktree add ../wt -b task/ops-f12d86 && cd ../wt && tasks start ops-f12d86
    # record sha256 unchanged; notes started, probe note, resumed; claim worktree = ../wt;
    # bin/vendored still modified and unstaged in the main checkout

    # B: HEAD clean; an uncommitted README.md draft carries a dead reference
    $ git add tasks/ops-e8049a.md && git commit -m "chore(tasks): start ops-e8049a"
    README.md:318: `docs/specs/draft-not-written-yet.md` does not exist   # refused
    $ git stash push -m "person's README draft" -- README.md
    $ git commit -m "chore(tasks): start ops-e8049a"   # passes; commit holds only the record
    $ git stash pop                                    # draft diff restored byte-identical

Recommendation: no workflow exception and no record-carrying mechanism. The staged-patch
workaround is unnecessary for every case reproduced.

- **A** is covered by the instructions as written: with the user's explicit work-in-place
  instruction, repair in place and commit the repair by pathspec (`git commit -- <paths>`)
  so the already staged record stays out of it, then commit the record, then
  `git worktree add` and `tasks start` there. Exact bytes, notes, unrelated changes and the
  claim handoff all hold. Without that instruction there is no compliant route (a repair
  task's own record meets the same gate): park `--waiting-on user --reason decision`,
  naming the failing check and paths.
- **B** needs the user too, because the remedy moves their uncommitted work: ask, and on
  consent stash only the offending paths around the record commit and pop after (verified
  exact). Never edit or commit their draft. In ops, check that the record commit holds
  only the record: `stage_docs_cmd` regenerates and stages README.md/AGENTS.md whenever
  they carry no unstaged edits, which a stash creates.
- **C** is not a repository repair: register or sync the referenced project on this host
  (or work where it is reachable), then commit. ops-d50aa5 asks for a finding that names
  the unregistered prefix.

The smallest instruction change is a short "commit refused by a content gate" paragraph
in `skills/tasks/SKILL.md` (owner: tasks) naming these three routes. The global rule
already lets an explicit work-in-place instruction win, so tack needs no change, and the
hooks need none from ops beyond ops-d50aa5's message. tasks-850488 carries that change.

## Proposed decomposition

- tasks-7ad96f — **briefed**, remains an idea; waits on tasks-10c968,
  P2/m/high/planned design, then reviewed implementation plan.
- tasks-850488 — **re-scoped** after tasks-cee8d9 to a todo: document the three
  blocked-bootstrap routes in `skills/tasks/SKILL.md`; P2/xs/low/direct.
- tasks-c1636f — **dropped** on user acceptance. Supporting delivery: tasks-9949f3
  and `7054f6f`; the positive report stays in history.
- tasks-10c968 ends at the user-reviewed spec and plan; updating any superseded spec
  status belongs to the re-scoped tasks-7ad96f when its implementation lands.

All remaining members and both follow-ups are children of tasks-0e7216. Follow-up completion
updates this brief and records a finding on its waiting idea in the same commit.
The new follow-ups record their relationship to the delivered halt and bootstrap work
through `concerns:` notes; they do not reopen the closed goals.
