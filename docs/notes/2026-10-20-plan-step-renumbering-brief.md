# Renumber plan steps without incomplete record commits

## Problem

Updating several plan headings leaves their task records inconsistent until every
step link has been changed. tasks-c9d4ba reports a failed auto-commit after each
intermediate edit. Find the smallest supported workflow that commits a consistent
plan and its step links, without weakening the drift check.

## Current behaviour and evidence

Inspected at 7a7c85c on 2026-10-20. src/cli.rs accepts one id per edit;
src/commands/edit.rs validates the edited step against its plan. The plan loop in
src/commands/check.rs emits a step_missing error for any remaining stale link.
The pre-commit hook runs task checks, including for record-only commits.
src/record_commit.rs commits the paths each command wrote after saving them;
a hook failure leaves the successful record write uncommitted and reports
commit_failed. This agrees with the report, but no live reproduction was run.

The approved record-commit design (docs/specs/2026-10-10-record-commit-design.md,
sections 3.1, 3.3 and 3.5) provides project opt-out and tasks commit recovery.
A staged index also defers auto-commit, preserving the staged content. There is
no explicit per-invocation commit switch in the current parser. No matching brief,
attached draft spec or open renumbering research was found. tasks-325e79 studies
linking drafts before review; it does not cover later multi-step renumbering.

## Constraints

Keep heading validation and hooks enabled. Never edit task records by hand or
change shared configuration as a test shortcut. Preserve unrelated staged and
unstaged work, claim ownership and stale-copy refusal. Record commits cover
records, whereas a final consistent change may also need the plan document.
New CLI syntax must land in ops's CLI table before adoption. No shared needs
are declared. This pass changes records and handoff documents only.

## Alternatives

1. Use a supported existing workflow to update the document and links, then commit
   the consistent set. Current lean: prove this is reliable before adding syntax;
   examine documented project opt-out and deliberate staging/deferred commits in
   an isolated fixture, including restoration and the plan's final commit.
2. Add an explicit invocation-level commit deferral with a final checked commit.
   This needs a reviewed contract for failed writes and incomplete batches.
3. Add an atomic multi-record step-remapping operation. Consider only if existing
   workflows cannot safely express the change; it adds validation and partial-write
   recovery obligations beyond a convenience flag.

## Unanswered questions

- Can existing operations express the whole change with no failed hook, loss of
  staging or persistent config change? The bounded workflow research answers this.
- Which commit contains the changed plan, and what happens on the second edit's
  failure? The same probe must inspect git state and recovery, not only exit codes.
- If a new operation is necessary, which minimal contract covers those cases?
  Research recommends a separate planned task only after demonstrating the gap.

## Proposed decomposition

tasks-e959ec is an ordinary child of Record authority and recovery (tasks-0e432f),
not another lane. tasks-c9d4ba stays briefed and waits on tasks-110ba6
(P2/s/mid/direct). The research uses a disposable project with isolated XDG config
and state, a two-step plan and a validating hook; it records observed warnings,
commits and recovery here. It updates the waiting idea with --no-reviewed before
its result branch merges. No product-code change is scoped yet.
