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
updated: 2026-10-11T13:33:18Z
started: 2026-10-11T09:51:00Z
depends: []
tags: []
source: "hq:docs/specs/2026-10-10-agent-docs-collection-design.md"
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-11-external-document-references-design.md
plan: docs/plans/2026-10-11-external-document-references.md
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
- 2026-10-11T11:49:39Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T11:49:39Z (feat/tasks-91f63b-external-docs): review: spec round 3 — verdict: accept; findings: minor 2; reviewer: claude-code/claude-opus-5-5
- 2026-10-11T11:49:39Z (feat/tasks-91f63b-external-docs): spec accepted by Keith; planning next. Minor follow-through: document doc_unchecked in the README output contract for consumers such as obs/tui; real-wrapper pilot evidence is linked from ops-721511 without rewriting the dated design measurement.
- 2026-10-11T12:11:37Z (feat/tasks-91f63b-external-docs): plan drafted at .worktrees/tasks-91f63b-external-docs/docs/plans/2026-10-11-external-document-references.md; seven sequential deliverables cover reader/supervision, changed-link writes/move, concurrent check, tolerant display, CLI export/filter, docs/review, and approved two-host installation. Inline execution recommended. Draft deliberately remains unlinked and has no executable children until user review. Self-review keeps read as the one body reader, isolates test config, includes production deadline cases once outside the fast suite, and preserves the shared-inventory/live-install gates.
- 2026-10-11T12:20:39Z (feat/tasks-91f63b-external-docs): plan self-review complete: added both apply_fields callers to Task 2, kept one read entry point with a per-description budget setter, supplied isolated fixture/install-smoke code, covered output-contract updates and the doc-root rollout recheck, and made children sequential. Verified ops-721511 depends on tasks-91f63b and mind6-646ca1: actual wrapper pilot remains an ops handoff after binary-task closure, avoiding a reverse dependency. No product edits, baseline tests or step-task creation before plan review.
- 2026-10-11T12:20:39Z (feat/tasks-91f63b-external-docs): parked (waiting on user, review): User reviews .worktrees/tasks-91f63b-external-docs/docs/plans/2026-10-11-external-document-references.md; agent records the plan review, revises if needed, then files/links the approved sequential steps, runs the deferred worktree baseline, and executes inline unless the user overrides.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T12:45:24Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T12:45:24Z (feat/tasks-91f63b-external-docs): review: plan round 1 — verdict: revise; findings: important 1, moderate 2, minor 2; reviewer: claude-code/claude-opus-5-5
- 2026-10-11T12:57:09Z (feat/tasks-91f63b-external-docs): plan round 1 revision: exit observation uses waitid WEXITED|WNOHANG|WNOWAIT (verified Linux documentation and installed libc bindings), keeps shell/PGID pinned until group kill, then reaps/disarms the guard; kernel-backed ordering test added. Ops inventory preparation/validation and async integration approval request move to execution setup. Rust-suite fetch stubs use sh with Rust-encoded JSON bodies; the documented shell stub passed five body/quoted-path cases in a disposable manual check. Check and display share set_fetch_budget plus one resolver deadline; intermediate check/show limitations are recorded in the execution ledger. Inline execution preference preserved; no product code changed.
- 2026-10-11T12:58:34Z (feat/tasks-91f63b-external-docs): parked (waiting on user, review): User re-reviews .worktrees/tasks-91f63b-external-docs/docs/plans/2026-10-11-external-document-references.md after plan round 1 corrections; agent records round 2 and, after acceptance, runs the deferred baseline, prepares/validates ops inventory and requests its integration approval at setup, then executes the sequential plan inline.
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T13:27:07Z (feat/tasks-91f63b-external-docs): resumed
  provenance: {"harness_session":"codex:01a12a32-51fc-79a0-b2db-a72d9a089cc1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-11T13:27:07Z (feat/tasks-91f63b-external-docs): review: plan round 2 — verdict: accept; findings: none; reviewer: human
