---
id: tasks-d0d520
title: Determine which checkout cross-project dependencies should read
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: docs/tasks-d0d520
created: 2026-10-06T09:45:48Z
updated: 2026-10-06T11:29:17Z
started: 2026-10-06T11:21:32Z
completed: 2026-10-06T11:27:49Z
depends: []
parent: tasks-984769
tags: []
source: tasks-70f0bc
agent: codex
---

Question: How do cross-project dependency creation and readiness read a worktree-only or newer completed task, and is an explicit publication step sufficient?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/resolve.rs::resolve_task/resolve_registered, src/commands/dep.rs::add_dependencies/ensure_acyclic, src/commands/list.rs::resolve_dependency, and the record-home and multi-project specs.

Bound: one disposable two-project git fixture with isolated XDG_CONFIG_HOME, XDG_STATE_HOME and RELAY_STATE_DIR; create an upstream worktree-only task, attempt a downstream dependency, integrate just its record, then close upstream in its worktree and compare downstream readiness before and after record integration. Compare show with dependency resolution. No routing, JSON, host configuration or product-code change.

Expected result: an observed behavior table and recommendation choosing explicit publication or a reviewed shared resolver change, including failure/cycle/foreign-claim checks and the owner of any workflow change; update the brief.

Ideas it wakes: on completion add the finding to tasks-70f0bc, tasks-562c53 with tasks note in the same commit, update the brief, then rerun scope on them.

## Notes

- 2026-10-06T09:45:48Z (main): concerns: tasks-9949f3 extension — Establish cross-project dependency visibility beyond the delivered local-show and stale-write contract.
- 2026-10-06T11:21:32Z (main): started
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:21:45Z (docs/tasks-d0d520): resumed
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:24:23Z (docs/tasks-d0d520): attached: tasks-d0d520-publication.py (8597 bytes): Runnable isolated two-project publication probe with record-only Git integration and runtime sentinel.
- 2026-10-06T11:24:23Z (docs/tasks-d0d520): attached: tasks-d0d520-publication-result.json (25023 bytes): Observed authority table and complete passing transcript, including cycles, foreign ownership and unreachable/config failures.
- 2026-10-06T11:24:23Z (docs/tasks-d0d520): Observed: show falls back through the live worktree claim only while the registered record is absent; dep refuses worktree-only targets, then succeeds after record publication. Worktree-only completion remains invisible to downstream show/ready until its record is published. Record-only Git integration left runtime.txt live; cycles, foreign closure, missing authority and malformed configuration remained guarded. Recommend explicit publication to registered authority, not a resolver change.
- 2026-10-06T11:27:49Z (docs/tasks-d0d520): review: impl round 1 — verdict: accept; findings: none; reviewer: codex
- 2026-10-06T11:27:49Z (docs/tasks-d0d520): Verification: 64-command disposable publication probe passed against the same binary digest as recovery; just check and diff hygiene passed. Review limits match fixture scope: no original-incident reconstruction, cross-host/linked-artifact/concurrent publication or actual rollout claimed. Source ideas were revalidated as unclaimed ideas at their original stamps before finding/scope writes.
- 2026-10-06T11:27:49Z (docs/tasks-d0d520): done
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:27:49Z (docs/tasks-d0d520): Established registered-checkout dependency authority with an isolated two-project probe: show fallback does not publish; open-record publication enables linking and completion publication enables readiness without runtime activation. Attached script/transcript and updated evidence table; scoped tasks-70f0bc guidance and proposed tasks-562c53 drop as covered. Failure/cycle/foreign-claim checks, just check and independent review passed; product code unchanged.
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:28:36Z (docs/tasks-d0d520): detached: tasks-d0d520-publication.py: Extend exported diagnostic redaction to the hostname.
- 2026-10-06T11:28:36Z (docs/tasks-d0d520): attached: tasks-d0d520-publication.py (8675 bytes): Runnable isolated publication probe; fixture paths, host root and hostname redacted in exported transcript.
- 2026-10-06T11:28:36Z (docs/tasks-d0d520): detached: tasks-d0d520-publication-result.json: Replace hostnames in diagnostic captures with a host placeholder; rerun all assertions.
- 2026-10-06T11:28:36Z (docs/tasks-d0d520): attached: tasks-d0d520-publication-result.json (25027 bytes): Repeated passing publication transcript, with hostnames and host paths redacted.
- 2026-10-06T11:28:36Z (docs/tasks-d0d520): Commit hygiene caught hostnames in claim diagnostics; extended exporter redaction, repeated every passing assertion and replaced attachments via CLI. Staged artifacts are now included in hygiene verification; task findings are unchanged.
- 2026-10-06T11:29:17Z (docs/tasks-d0d520): review: impl round 2 — verdict: accept; findings: none; reviewer: codex
