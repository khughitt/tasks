# Linking a spec or plan that exists only on its task's branch

Scope pass, 2026-10-15. Ideas: tasks-3eda29 (from nexc), tasks-cefa7d (from mind6).

## Problem

Planned work writes its spec and plan in the task's worktree, where they stay until the
branch merges. In both reports, the tracker refused to link a document that existed only
on that unmerged branch. A planned task then either holds no spec or plan link until
integration, or its step children are filed with no `--plan`/`--step`. Then
`check`'s drift checks and `list --plan` cannot see the plan the steps execute.

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

## Proposed decomposition

- Goal: tasks-a73dce, *Link a spec or plan that exists only on its task's branch* (under
  the Record authority lane, tasks-0e432f).
- Research: tasks-82bd08, *Establish where a branch-only spec or plan can be linked and
  checked*, exercised on an in-tree project. tasks-cefa7d waits on it.
- Design: tasks-6360ac, *Design overlay-side document roots in a linked project's code
  checkout* (planned; depends on tasks-82bd08). tasks-3eda29 waits on it.
- A design task for the in-tree case is filed only if the research shows alternative 1
  cannot serve it.

The note-routing reports from the same scope pass need no brief. tasks-16a48c is scoped
under tasks-984769 (the write error names the checkout its claim or park records).
tasks-ae03ed duplicated it and was dropped.
