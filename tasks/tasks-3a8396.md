---
id: tasks-3a8396
title: Address the blockers report's deferred review minors
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-26T01:55:16Z
updated: 2026-10-26T01:55:16Z
depends: []
tags: [cli]
source: tasks-e637a1
agent: claude-code/claude-opus-5-5
origin: agent
---

Why: the final review of tasks blockers (tasks-e637a1, merged 074672f) left eight Minor findings deferred; they are recorded only in tasks-e637a1's execution-ledger.md and review report attachments. Separately, [ref:spec] still says "Status: proposed for user review" although it was approved and shipped.

Done:
- blockers --help states that blocking-since and confirmed-pass metrics are unavailable (spec: stated in warnings and help); check parser_surface_equals_table compares only the summary.
- unresolved diagnostics keep every stored spelling of a missing reference (spec: retain original reference IDs) while counts stay per canonical id; update the single-diagnostic expectation in blockers_identity_and_lifecycle with it.
- one scan-first lookup helper shared by commands/blockers.rs and list::resolve_dependency, ideally map-backed.
- isolated_worktree, write_claim and own_pid_view move into tests/common instead of being copied between tests/cli.rs and tests/blockers.rs.
- pretty assertions tied to their row, the footer check asserts "history: blocking since" rather than "unknown", and the 120-column width check also runs on the fuller label fixture.
- the spec's status line records approval and shipment (decision record, not rewritten as reference).
Verification: just test-one --test blockers and blockers_ green, just test-fast; JSON shapes unchanged except added diagnostic rows.

## References

- spec (context): docs/specs/2026-10-25-blocker-ranking-design.md — approved design whose status line still reads proposed

## Notes

- 2026-10-26T01:55:16Z (add): created
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
