---
id: tasks-45e400
title: "dep --on and --rm are mutually exclusive, so adding and removing a dependency takes two invocations"
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: feat/dep-swap
created: 2026-09-19T00:50:15Z
updated: 2026-10-02T15:22:14Z
started: 2026-10-02T15:20:05Z
completed: 2026-10-02T15:22:14Z
depends: []
parent: tasks-671956
tags: [feedback, friction, "from:material"]
model: claude-opus-5-5
agent: "claude-code/claude-opus-5[1m]"
---

Why: Swapping dependencies currently requires two writes because clap makes --on and --rm mutually exclusive and dep::run chooses only one branch.

Done: Accept --on and --rm together. Canonicalize input IDs with the existing parser; reject an ID present in both sets, including alias-equivalent spellings, with a typed validation error before saving. Apply removals and additions to one candidate and save once. When additions are present, validate the final graph with ensure_acyclic; retain the existing removal-only path without graph traversal so unrelated broken references do not prevent cleanup. Preserve add-only deduplication, remove-only missing-edge errors, and removal of stored unreachable references. Any invalid ID, absent removal, unresolved addition, overlap, or cycle leaves the original record unchanged. Preserve unrelated dependencies and their spelling.

Where to look: src/cli.rs (Commands::Dep), src/commands/dep.rs::run and ensure_acyclic, src/commands/mod.rs::parse_id and save, tests/cli.rs dependency and alias tests. Update the dep syntax in docs/specs/2026-08-29-tasks-design.md, README.md, and skills/tasks/SKILL.md when implementation lands.

Verification: Add focused CLI regression coverage for a successful swap preserving an unrelated edge; reject alias-equivalent overlap and a failing batch without changing the record; retain existing add-only/remove-only and cross-project cycle checks. Run just test-one --test cli <filter>, then just test-fast.

Original report: tasks dep <id> --on <a> --rm <b> fails with 'cannot be used with'; expected one call to apply both edits

## Notes

- 2026-10-02T14:22:40Z (main): scope: scoped; one-save dependency swaps with canonical overlap rejection and final-graph validation; P2/s/mid/direct; original report preserved
- 2026-10-02T15:20:05Z (main): started
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:20:06Z (feat/dep-swap): resumed
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:22:14Z (feat/dep-swap): done
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:22:14Z (feat/dep-swap): dep accepts --on and --rm together: canonical overlap is a validation error, removals then additions apply to one candidate saved once, additions validate the final graph, removal-only skips graph traversal; any failure leaves the record unchanged; spec, README and skill updated
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
