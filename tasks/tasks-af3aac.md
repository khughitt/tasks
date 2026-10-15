---
id: tasks-af3aac
title: "Accept relay Codex worker rows: worker scope and thread-hint tie-break"
status: doing
priority: 2
size: s
complexity: mid
process: planned
owner: feat/codex-worker-rows
created: 2026-10-15T10:02:07Z
updated: 2026-10-15T10:50:23Z
started: 2026-10-15T10:35:35Z
depends: []
parent: tasks-2fa8c6
tags: []
source: relay-6825f7
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-06-delegated-identity-design.md
---

Why: relay will publish a registry row per Codex worker (relay docs/specs/2026-10-15-codex-worker-rows.md, approved 2026-10-15): id codex:<W>, sessionId W (the worker's CODEX_THREAD_ID), scope "worker", root codex:<R>, sharing the controller's process handle. Today tasks refuses such a snapshot (scope must be session for Codex) and, on a shared handle, refuses every qualifying row ('several relay agents match'), locking out the controller too.
Change: (1) the snapshot validator accepts scope "worker" for Codex rows (root may be ignored); (2) in relay::resolve, after the process filters and before the cardinality refusal, when the harness is Codex, a hint is present and more than one row qualifies, keep the single qualifying row whose sessionId equals the hint; none or several still refuse. This is the tie-break the delegated-identity design §4 deferred until relay's worker contract existed; that spec is now the contract. Amend §4 accordingly.
Done: tests over relay's shared fixture test/fixtures/registry-workers.json (vendored beside ancestry.json and handles.json) select the root row for the controller hint and each worker row for its hint, and refuse a hint matching no row; the §4 amendment is reviewed. Must be installed on every relay-identity host before relay's worker routing ships (relay spec, Rollout step 1).

## Notes

- 2026-10-15T10:02:09Z (main): concerns: relay-6825f7 extension — tasks side of relay's Codex worker-row contract
- 2026-10-15T10:02:24Z (main): correction: the concerns line above is mistaken (relay-6825f7 is still open; this piece comes from its approved spec, not from closed work falling short) — disregard it
- 2026-10-15T10:35:35Z (main): started
  provenance: {"harness_session":"claude-code:956af17d-221f-45e8-b1e0-bac730bba1be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T10:35:45Z (feat/codex-worker-rows): resumed
  provenance: {"harness_session":"claude-code:956af17d-221f-45e8-b1e0-bac730bba1be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T10:39:21Z (feat/codex-worker-rows): parked (waiting on user, review): User: review the 'Relay worker rows (2026-10-15 amendment)' section of .worktrees/codex-worker-rows/docs/specs/2026-10-06-delegated-identity-design.md (plus the §4 and §6 item 4 edits). Agent then records the spec review round and writes the implementation plan in docs/plans/.
  provenance: {"harness_session":"claude-code:956af17d-221f-45e8-b1e0-bac730bba1be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T10:49:49Z (feat/codex-worker-rows): resumed
  provenance: {"harness_session":"claude-code:956af17d-221f-45e8-b1e0-bac730bba1be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T10:49:50Z (feat/codex-worker-rows): review: spec round 1 — verdict: revise; findings: P2 1; reviewer: unverified (pasted by user)
- 2026-10-15T10:50:23Z (feat/codex-worker-rows): Spec round 1 fix: a scope-worker row is accepted only with a matching CODEX_THREAD_ID, sole or tie-broken; CLI control asserts start/park/done refuse without mutation. Keith approved the spec after this fix (2026-10-15).
