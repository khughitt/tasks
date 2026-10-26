# Linking a spec or plan that exists only on its task's branch

Scope pass, 2026-10-15. Ideas: tasks-3eda29 (from nexc), tasks-cefa7d (from mind6).
Updated 2026-10-26 for tasks-85e8cd. The original in-tree and linked-project
findings below describe the October 15 code; the detached-project finding names
the newer implementation. This remains a brief, not an approved design.

## Problem

Planned work writes its spec and plan in the task's worktree, where they stay until the
branch merges. In both reports, the tracker refused to link a document that existed only
on that unmerged branch. A planned task then either holds no spec or plan link until
integration, or its step children are filed with no `--plan`/`--step`. Then
`check`'s drift checks and `list --plan` cannot see the plan the steps execute.

The detached-project report adds a different boundary: linking from a code worktree
succeeds, but the automatic record commit validates from the records repository and
cannot find the branch-only document. New working documents belong in records;
the unresolved case is an existing tracked document revised only on a code branch.

## Current behaviour and evidence

- `Resolver::locate` and `resolve_doc` (`src/resolve.rs`) look under the writing
  checkout's project root, then, from a linked worktree, under the main checkout
  (tasks-ace27b, dca3e62). The fallback runs one way only: main never looks in a
  worktree, so `add`/`edit --spec|--plan` from main returns `doc_not_found` for a
  branch-only document.
- **In-tree project (cefa7d, mind6).** Records written from the worktree would find
  the plan there, but the reporter's plan execution wrote task records in the main
  checkout. mind6-3a9a27's step children (mind6-0324ef, mind6-153396, …, filed
  2026-10-13 17:43) carry no plan link. Why that execution wrote records in main is not
  recorded; candidates are the stale-copy rule (record-home spec §3), picker visibility
  of step tasks from main, or the flow's controller convention.
- **Linked project (3eda29, nexc).** The project root is the overlay
  (`overlay/nexc`), and `-C <code worktree>` opens that same overlay project
  (`parked::open_recorded`, linked-checkouts §4.1). Documents resolve under the overlay
  root by design: ops `docs/specs/2026-10-01-linked-checkouts-design.md` §168 says
  `--spec` and `--plan` resolve there. The reported spec
  (`.worktrees/nex-342/docs/superpowers/specs/2026-10-14-hap-union-render-design.md`,
  nexc-ad7d4e's `workflow-feedback-design.md` likewise) is a team document in the code
  repository. It is outside every configured root, so `-C` cannot help. The overlay has
  no worktrees, so its own working documents are never branch-only.
- `check` fails `doc_missing` for an open task whose linked file is absent from both the
  checkout and its main checkout (tasks design spec, §check). A link to a branch-only
  document written from main would therefore break main's gate until the branch merges.
- **Detached automatic commits (tasks-85e8cd, October 26).** At code revision
  `f1d45f2`, `Resolver::walk` tries records, the records main checkout, then the
  invoking code worktree and code root for a detached project (`c99900e`, `5370117`).
  `record_commit::commit_paths` runs Git in the records repository. The filing
  transcript binds the report to a successful link whose commit warning contains
  `doc_missing`; moving the new spec into records recovered. It does not establish
  that a records-root hook can validate a retained branch-only document. No new
  runtime reproduction was run in this scope pass.

## Constraints

- The cross-checkout records brief (`docs/notes/2026-09-29-cross-checkout-records-brief.md`)
  and the user's 2026-09-29 decision set the main-checkout fallback as a warning
  (`doc_in_main_checkout`), not an error. That covers only a worktree reading main.
- Record-home spec (`docs/specs/2026-09-30-record-home-design.md`) §7 rejects routing a
  write to another checkout automatically, and refuses writes from a stale copy.
- The linked-checkouts design puts agents' working documents in the overlay and the
  team's documents under the repository's own convention (nexcode's `docs/superpowers/`).
- External document references (`docs/specs/2026-10-11-external-document-references-design.md`)
  already let a link name a host-configured provider instead of a path.
- tasks-325e79 (Work discovery lane) establishes how a draft plan is linked before
  review without making its steps executable. Any workflow here must agree with it.
- The records-home design (`docs/specs/2026-10-22-records-home-design.md`) and current
  task skill put new working documents in records and retain existing tracked
  documents in code until rewritten. The new report's initial placement violated
  the former rule; its successful recovery does not settle the latter case. Keep
  commit hooks and record-only integration intact; do not silently scan sibling
  branches or repoint host configuration.

## Alternatives

1. **Link from the branch's checkout; guidance only.** Spec, plan and step records are
   written in the task's worktree and merge with the documents. Main never links an absent
   file. This needs no code change if records can be written there during plan execution.
   It fails where execution must write in main. **Current lean**, pending the research below.
2. **Look in sibling worktrees when a document is absent.** Mirror the main-checkout
   fallback in the other direction, with a warning naming the worktree. This needs a
   rule for `check` in main (warn rather than `doc_missing` while a sibling holds the
   file), and it reverses part of the 2026-09-29 decision.
3. **Linked projects: a checkout-relative root.** Let a linked project's config name
   document roots in the code checkout, or serve them through an external-reference
   scheme. **Accepted in principle by the user, 2026-10-15,** under one invariant: the
   overlay may depend on the code repository, but nothing of tasks may reach the code
   repository. Roots are declared only in the overlay's config, read-only, with no link,
   config or write in the code repository. Design: tasks-6360ac.

For detached automatic commits, first test the supported records-first workflow
and retained-document case. Prefer placement/recovery guidance if those suffice;
design explicit context propagation only if a retained-document fixture proves a
gap. Scanning arbitrary sibling worktrees would introduce an authority policy the
existing evidence does not justify.

## Unanswered questions

- Why did mind6's plan execution write step records in the main checkout, and does
  anything prevent writing them in the worktree? *Research task below; mind6's
  guidance and the flows skill can answer.*
- ~~Should a linked project be able to link a team document that lives in the code
  repository?~~ *Answered by the user, 2026-10-15: yes, from the overlay side only
  (alternative 3).*
- For alternative 3: which code worktree does a link resolve in, and what does `check`
  report for a team document that exists only on an unmerged code branch? *tasks-6360ac,
  taking tasks-82bd08's finding.*
- If alternative 2 is chosen, what does main's `check` report for an open task whose
  document is only on a branch? *Design, after the research.*
- Can automatic record commits validate an existing tracked document changed only
  in the invoking detached code worktree, including changed plan headings? The
  detached-commit research below compares direct validation and the actual commit
  hook. Its author determines whether guidance suffices or a reviewed design is
  needed; this pass does not choose a context transport.

## Proposed decomposition

- Goal: tasks-a73dce, *Link a spec or plan that exists only on its task's branch* (under
  the Record authority lane, tasks-0e432f).
- Research: tasks-82bd08, *Establish where a branch-only spec or plan can be linked and
  checked*, exercised on an in-tree project. tasks-cefa7d waits on it.
- Design: tasks-6360ac, *Design overlay-side document roots in a linked project's code
  checkout* (planned; depends on tasks-82bd08). tasks-3eda29 waits on it.
- A design task for the in-tree case is filed only if the research shows alternative 1
  cannot serve it.
- Research tasks-6644b7: establish the records-root hook behavior with one
  isolated detached project, testing both a new records-owned document and an
  existing tracked code document. This is distinct from tasks-82bd08's in-tree
  fixture and tasks-6360ac's linked overlay design. tasks-85e8cd waits on its finding.

Related findings from the October 26 pass have separate dispositions. tasks-40b49b
repeats registered-record dependency publication, already investigated by
tasks-d0d520 and assigned to tasks-70f0bc; its proposed drop requires no new research.
tasks-60cc95 is scoped separately under the migration lane: exclude moved-only plan
links from heading-coverage auditing while keeping coverage for plans still linked
by non-moved records. `check.rs` currently seeds `linked_plans` from every status,
then audits every loaded heading. The report's exact move sequence is not verified:
the current move command accepts only unparented, childless tasks. Preserve the
source plan and tombstone history, and test a shared live plan and main-checkout
document fallback. This brief supplies resolver context, not approval for a broader
move or document-routing change.

The note-routing reports from the same scope pass need no brief. tasks-16a48c is scoped
under tasks-984769 (the write error names the checkout its claim or park records).
tasks-ae03ed duplicated it and was dropped.
