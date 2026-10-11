---
id: tasks-91f63b
title: "External document references: scheme-prefixed plan/spec, fetched per scheme from host config"
status: doing
priority: 1
size: l
complexity: high
process: planned
owner: feat/tasks-91f63b-external-docs
created: 2026-10-10T23:02:26Z
updated: 2026-10-11T11:23:37Z
started: 2026-10-11T09:51:00Z
depends: []
tags: []
source: "hq:docs/specs/2026-10-10-agent-docs-collection-design.md"
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-11-external-document-references-design.md
---

Implements §4 of the hq spec (spec round 4 accepted): plan/spec values whose first path segment contains a colon are external references (`mindful:thought:<32 hex>`); parse succeeds without scheme config; the host config `[docs.schemes] <scheme> = "<command>"` names the fetcher, run as `sh -c "<cmd> \"\$1\"" -- <rest>` returning `{"id","body"}` JSON on stdout (capped, 30 s); one `Resolver::read(kind, value)` behind step_exists, check reads and historical plans; `tasks doc <id> --plan|--spec` prints the body; `tasks list --plan <value>` exact filter with rows carrying `step`; errors Config/DocNotFound, check warning `doc_unreachable`, show warns on an unavailable historical plan; tests per §4.6. The resolver is a design decision in this project's terms, hence planned. Install the binary on both hosts.

## Notes

- 2026-10-11T09:51:00Z (main): started
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T09:54:09Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T09:54:09Z (feat/tasks-91f63b-external-docs): process: planned; workspace .worktrees/tasks-91f63b-external-docs; draft the local resolver design from accepted hq spec section 4 before implementation. No setup recipe; baseline tests deferred to plan execution.
- 2026-10-11T09:56:32Z (feat/tasks-91f63b-external-docs): design evidence: hq-5a3e12 records spec round 4 accepted and merged despite its stale draft header. Local scope includes editor canonicalization, bounded fetch/process-group cleanup, raw tasks doc output, exact list --plan with step rows, shared check reads, and preserved historical/path error behavior.
- 2026-10-11T10:05:52Z (feat/tasks-91f63b-external-docs): spec drafted and self-reviewed: shared resolver; host-only trusted fetch commands; canonical flag/editor writes; 2 MiB stdout, 200-byte stderr and 30-second deadline with process-group cleanup; raw doc export; exact list/search plan filter and optional step rows. Shared CLI inventory must land in ops before adoption. No product code or tests changed.
- 2026-10-11T10:05:52Z (feat/tasks-91f63b-external-docs): parked (waiting on user, review): User reviews .worktrees/tasks-91f63b-external-docs/docs/specs/2026-10-11-external-document-references-design.md; agent records the review, revises if needed, then writes the implementation plan for user review in this worktree.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T10:36:10Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T10:36:10Z (feat/tasks-91f63b-external-docs): review: spec round 1 — verdict: revise; findings: important 4, moderate 1, minor 3; reviewer: claude-code/claude-opus-5-5
- 2026-10-11T10:42:15Z (feat/tasks-91f63b-external-docs): spec round 1 revision: addressed all 8 findings after checking the cited callers. External link/step reads depend on changed values; show/next return warning plus unknown step_found when unavailable; DocRead/UnavailableCause separate availability from fatal errors; check and optional display reads have fixed 5-second budgets. No-step external display does not fetch, exit-0 fetches terminate pipe-holding descendants before EOF, and doc roots reserve first-segment colons. Binary-before-config and both-binaries-before-records rollout constraint recorded on ops-721511.
- 2026-10-11T10:43:22Z (feat/tasks-91f63b-external-docs): parked (waiting on user, review): User re-reviews .worktrees/tasks-91f63b-external-docs/docs/specs/2026-10-11-external-document-references-design.md after the spec round 1 corrections; agent records round 2, revises if needed, then writes the implementation plan after spec acceptance.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T11:18:30Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T11:18:30Z (feat/tasks-91f63b-external-docs): review: spec round 2 — verdict: revise; findings: important 1, minor 2; reviewer: claude-code/claude-opus-5-5
- 2026-10-11T11:21:14Z (feat/tasks-91f63b-external-docs): latency pilot before planning: mindful-doc is not installed. Measured installed mindful --json show plus in-process id/body projection of one existing thought (warm repeats; excludes future wrapper startup): first read 0.459 s; 20 serial reads 9.476 s, median 0.472 s; 20 reads with concurrency 4 took 2.889 s, median per-call 0.580 s. Concurrency works in this sample; actual wrapper, varied backlog and second-host timings remain rollout gates. CLI Show takes one id, so three separate descriptions can consume 15 s, not one supported multi-id command.
- 2026-10-11T11:23:36Z (feat/tasks-91f63b-external-docs): spec round 2 revision: bounded concurrency 4 replaces serial check fetching; dispatch stepped open plans, other open plans, open specs, then closed-only plans. Budget-incomplete references get one doc_unchecked summary rather than per-task doc_unreachable. Real-wrapper whole-backlog timing (at least 20 varied references, 3 complete runs per host) gates activation, with ops direct-file fallback if needed; requirement sent to ops-721511. Per-description display budget is explicit; current show accepts one id. No implementation started.
- 2026-10-11T11:23:37Z (feat/tasks-91f63b-external-docs): parked (waiting on user, review): User reviews .worktrees/tasks-91f63b-external-docs/docs/specs/2026-10-11-external-document-references-design.md after spec round 2 corrections; agent records round 3, revises if needed, then writes the implementation plan after spec acceptance.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
