# Worktree audits and blocked bootstrap

## Problem

Task worktrees should retain the latest record and support isolated repairs. This
batch contrasts a successful stale-copy handoff (tasks-c1636f) with two gaps:
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
  The original incident command and gate output are unavailable.
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
historical evidence, and the record-home spec header still says review revision;
current code and completed task records establish the delivered behavior here.
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
   whether a compliant existing path works. If none does, propose a narrowly reviewed
   isolated-repair handoff rather than disabling gates or adding a copying service.

Prefer existing safeguards and the smallest explicit changes that satisfy both
contracts. The halt and bootstrap questions can progress independently.

## Unanswered questions

- Where should an attempted audit land when the halt's active copy differs from
  authority, and what does a safe refusal tell each owner to do? tasks-10c968 will
  propose the ownership and failure contract for user review.
- Which check blocked the original bootstrap, and is a compliant recovery already
  available? tasks-cee8d9 will trace the local gates and run one controlled scratch
  case; the original incident remains unknown without its capture.

## Proposed decomposition

- tasks-7ad96f — **briefed**, remains an idea; waits on tasks-10c968,
  P2/m/high/planned design, then reviewed implementation plan.
- tasks-850488 — **briefed**, remains an idea; waits on tasks-cee8d9,
  P2/s/mid/direct investigation with an explicit bound and result.
- tasks-c1636f — **drop proposed**, remains an idea pending disposition. Supporting
  delivery: tasks-9949f3 and `7054f6f`; retain the positive report in history.

All members and both follow-ups are children of tasks-0e7216. Follow-up completion
updates this brief and records a finding on its waiting idea in the same commit.
The new follow-ups record their relationship to the delivered halt and bootstrap work
through `concerns:` notes; they do not reopen the closed goals.
