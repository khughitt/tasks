# Task references across projects, versions, and attachment filing

## Problem

The first reference increment landed, but a task still cannot cite another project's
artifact structurally (tasks-b841bc), identify the version of evidence actually reviewed
(tasks-d4cf54), or be filed with an already-existing input attachment and its citation in
one call (tasks-efb6d8). These share the reference contract, but need separate decisions.

## Current behaviour and evidence

Checked against the main checkout on 2026-10-19:

- `src/refs.rs::location` rejects a leading `@`; `parse_entry` accepts no revision
  attribute. `src/resolve.rs::locate_entry` reads the current checkout first and main
  only when the local target is absent. Commits `d242e98`, `7e8f4f0`, and `3d35e51`
  implement storage, resolution, and CLI reads/writes. The implemented
  `docs/specs/2026-10-19-task-references-design.md` §§9–11 explicitly defer both
  cross-project targets and pins; this is extension work, not a missed implementation.
- `src/refs.rs::require_targets` requires input/context attachment bytes and a live
  ledger entry. `src/commands/add.rs::run` validates before creating the task;
  `src/commands/attach.rs::attach` first loads an existing task, then writes bytes and
  its ledger note with cleanup on save failure. Filing therefore takes add, attach,
  then edit of references/body. Pending output references are allowed, but relabeling
  existing input evidence as output would misstate its role.
- `tests/refs.rs` covers dangling-citation refusal, attachment bytes plus ledger,
  main-checkout precedence, and missing-output completion. These tests were read,
  not run in this scope pass. The reported add/attachment combination was traced in
  code; a disposable end-to-end reproduction remains in the follow-up.
- No existing brief, attached draft, or open follow-up covers these three extensions.
  Related open work was checked across the local goal tree before filing follow-ups.

Additional evidence checked at 3813d1d on 2026-10-25: tasks-041759 reports a
cross-project scope follow-up unable to declare the authoritative brief as required
input; a dated attachment snapshot was the workaround. The current resolver still
rejects absolute paths and reserves `@`. This adds an acceptance case to tasks-d09e87,
not a separate design. See docs/notes/2026-10-25-evidence-handoffs-brief.md for the
report's provenance limitation and adjacent attachment/commit questions.

## Constraints

Keep handles stable and preserve strict input validation, attachment safety/caps,
explicit output semantics, and typed missing/unavailable diagnostics. A hash alone
does not retain bytes, and a Git revision alone does not guarantee future availability.
Closed references are historical and are not routinely audited by `check`.

Coordinate with tasks-dc16eb (records/document homes), tasks-a73dce and its research
tasks-82bd08/design tasks-6360ac (branch-only and linked roots), and tasks-91c622
(cross-project scope workflow and handoff placement). Reuse their outcomes; do not
repeat their investigations or assume their entire migrations must precede design.
tasks-b9b2e2 already specifies multiple-file attach; it does not cover creation of a
task with input citations. Preserve that task's batch semantics when recommending a
filing interface. Any new CLI flags begin in ops's vocabulary and are adopted here.

## Alternatives

1. **Keep existing workflows.** Use prose/external references across projects, attach
   copies for fixed evidence, and add/attach/edit for filing. This is available now;
   document its limits without claiming copies are immutable.
2. **Extend the current resolver and filing commands narrowly.** Design project-qualified
   resolution; first establish when snapshots are insufficient and what attachment
   filing transaction is needed. This is the lean: reuse the existing mechanisms,
   retaining separate approval gates for any resulting designs.
3. **Introduce a general artifact store or batch transaction framework.** The reports
   do not establish a need for either. Do not expand the follow-ups into these systems.

## Unanswered questions

- Which project identity and document root does a qualified reference select, including
  aliases, missing projects, linked checkouts, and rename/move behavior? The cross-project
  design author proposes one contract for user review, coordinating the existing root
  and scope designs. Writing, inventory, doc, check, and exact list filtering must agree.
- Which concrete review evidence requires more than an attached snapshot, and how are
  bytes retained and retrieved? A bounded investigation compares one tracked report,
  one untracked artifact, and one external document; it recommends the smallest useful
  support boundary before any pinning design is filed.
- Can task creation, attachment storage, ledger, and input reference validation share
  one safe operation? A bounded filing investigation traces failure/retry and sourced-add
  reuse, then recommends an interface or the existing workflow. No validation bypass.

## Proposed decomposition

Goal **tasks-7102c1** is an ordinary child of the existing Record authority and recovery
lane (`tasks-0e432f`). It holds the three briefed ideas and their follow-ups:

- **tasks-d09e87** — design cross-project reference resolution, P3/m/high/planned;
  **tasks-b841bc** and **tasks-041759** wait on it. The latter stays under
  tasks-bbfc3c; the design retains this brief and also reads the evidence-handoff
  brief. Cover authoritative required inputs on reused foreign follow-ups without
  duplicate snapshots. Review the written spec and plan before implementation.
- **tasks-c8ad5b** — establish the smallest reliable way to retain reviewed evidence
  versions, P3/s/mid/direct; **tasks-d4cf54** waits on it. A separate pinning design is
  filed only if the investigation establishes the need and boundary.
- **tasks-4216a2** — establish a safe filing operation for tasks with input attachments,
  P2/s/mid/direct; **tasks-efb6d8** waits on it. Do not duplicate tasks-b9b2e2's batch
  attachment implementation.

The waits are dependency edges. Each follow-up updates this brief and records its
finding on its waiting idea with `tasks note <id> --no-reviewed` before its branch
merges. Sources and original idea bodies are preserved. No implementation is authorized
by this brief.
