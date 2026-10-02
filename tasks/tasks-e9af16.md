---
id: tasks-e9af16
title: Design safe edit --depends semantics from the dependency-editing brief
status: done
priority: 2
size: m
complexity: high
process: planned
owner: design/edit-depends
created: 2026-10-02T14:22:40Z
updated: 2026-10-02T15:46:50Z
started: 2026-10-02T15:24:43Z
completed: 2026-10-02T15:46:48Z
depends: []
parent: tasks-671956
tags: []
model: claude-opus-5-5
agent: codex
spec: docs/specs/2026-10-02-edit-depends-design.md
---

Why: Silent replacement can erase existing edges, but changing replacement to append alters the CLI contract.
Done: Inspect replacement use cases in this checkout and tests; compare explicit replacement documentation with additive edits plus an explicit clear/replace operation. Recommend the smallest safe contract, including add/edit/editor differences, clear-all behavior, alias deduplication, validation failures, and focused acceptance checks. Produce a user-reviewed spec and then a user-reviewed plan before implementation. Recommendation: follow additive tags with explicit replacement, unless evidence establishes intentional replacement consumers that need a different contract.
Where to look: docs/notes/2026-10-02-dependency-editing-brief.md; src/cli.rs::TaskFields, src/commands/mod.rs::apply_fields, src/commands/edit.rs::run, tests/cli.rs, README.md, skills/tasks/SKILL.md.
Ideas it wakes: On design completion, add the approved finding to tasks-8efda8 in the same commit, then re-scope it against that contract. No additional swap or alias work belongs in this design task.

## Notes

- 2026-10-02T15:24:43Z (main): started
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:24:48Z (design/edit-depends): resumed
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:26:20Z (design/edit-depends): spec drafted: additive edit --depends with --no-depends clear/replace (mirrors --tag/--no-tags); no replacement callers found in checkout, sibling tooling, or corpus
- 2026-10-02T15:26:37Z (design/edit-depends): parked (waiting on user, review): user reviews docs/specs/2026-10-02-edit-depends-design.md in .worktrees/edit-depends; on approval, write the implementation plan there
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:29:15Z (design/edit-depends): review: spec round 1 — verdict: accept; findings: P3 2; reviewer: codex/gpt-6
- 2026-10-02T15:29:15Z (design/edit-depends): Spec review nits: section 3 says cycle validation runs when anything was added, while section 4 runs it for any nonempty input; recommend stating that duplicate-only requests also validate the full graph, matching dep --on, and covering that case. Section 1 overstates the rename issue: apply_fields canonicalizes explicitly supplied replacement ids, not all stored references; the preservation rule is in rename spec section 4, not section 3.
- 2026-10-02T15:30:09Z (design/edit-depends): spec revised for review round 1: validation runs whenever --depends is given (duplicate-only + unreachable edge check added); rename rationale corrected to §4 and to what replacement actually does
- 2026-10-02T15:30:58Z (design/edit-depends): resumed
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:30:58Z (design/edit-depends): spec approved by user
- 2026-10-02T15:33:19Z (design/edit-depends): plan drafted: docs/plans/2026-10-02-edit-depends.md (2 tasks: shared add helper + additive --depends; --no-depends + docs); linking it and filing step children under tasks-8efda8 waits for plan approval so pickers do not offer unapproved steps
- 2026-10-02T15:33:19Z (design/edit-depends): parked (waiting on user, review): user reviews docs/plans/2026-10-02-edit-depends.md in .worktrees/edit-depends and picks an execution method; then close e9af16, re-scope tasks-8efda8 against the plan, link the plan and file its two step children
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:38:13Z (design/edit-depends): review: plan round 1 — verdict: accept; findings: P3 1; reviewer: codex/gpt-6
- 2026-10-02T15:38:13Z (design/edit-depends): Plan review nit: Task 2 tests successful replacement and clearing, but not failed replacement after clearing. Add a case for --no-depends --depends <valid> --depends <unresolvable> and assert unresolvable_id plus byte-identical original file, including its old dependencies. No blocking implementation issues found; the immutable helper with returned warnings fits the existing resolver borrow in apply_fields.
- 2026-10-02T15:39:10Z (design/edit-depends): plan revised for review round 1: Task 2 test adds a failed replacement (--no-depends with an unresolvable --depends) that leaves the file byte-identical
- 2026-10-02T15:46:48Z (design/edit-depends): resumed
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:46:48Z (design/edit-depends): plan approved by user; execution: native
- 2026-10-02T15:46:48Z (design/edit-depends): done
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:46:48Z (design/edit-depends): spec and plan approved: additive edit --depends with --no-depends clear/replace; tasks-8efda8 re-scoped as the implementation with two plan steps
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
