---
id: tasks-b07adc
title: Stamp lifecycle notes without changing claims
status: done
priority: 2
size: m
complexity: high
process: direct
owner: feat/lifecycle-provenance
created: 2026-09-17T21:08:28Z
updated: 2026-09-17T23:04:29Z
started: 2026-09-17T22:57:08Z
completed: 2026-09-17T23:04:29Z
depends: [tasks-d51eda]
parent: tasks-c9199a
tags: []
source: tasks-c9199a
agent: codex
plan: docs/plans/2026-09-17-lifecycle-provenance.md
step: "Task 2: Stamp lifecycle notes without changing claims"
---

Wire the approved provenance pair into shared start/resume/close transitions and existing park/close-message notes; preserve claim identity, liveness, write ordering and retries. Includes full regression checks, independent diff review, integration and installed-reader verification. This is the complete note-stamping prerequisite consumed by obs-09cb62; it does not depend on relay ancestry. Plan approved; deployment is gated on reader installation and verification on titan and europa (Europa).

## Notes

- 2026-09-17T21:42:13Z (feat/lifecycle-provenance): parked (waiting on user, environment): Before writer work: Task 1 must be merged and cargo installed on every host reading synced task files; record host/revision and isolated stamped-fixture read evidence. User must confirm hosts not inspected here. Plan approved; this is a deployment gate, not another review.
- 2026-09-17T21:48:01Z (feat/lifecycle-provenance): User confirmed the reader rollout inventory: titan and europa (Europa). Task 2 remains parked until both have the reader-only revision installed and an isolated stamped-fixture read verified. This session handles titan; Europa installation/verification requires user confirmation.
- 2026-09-17T21:51:35Z (feat/lifecycle-provenance): Reader-only tasks-d51eda is merged as 57311fc and cargo installed on titan. Installed binary passed isolated stamped-fixture show/edit/show, preserving the entire note; no shared registry or synced stamped files used. Europa reader install and equivalent read evidence remain required before writer work.
- 2026-09-17T21:51:35Z (feat/lifecycle-provenance): parked (waiting on user, environment): User: install reader-only revision 57311fc or a descendant on europa (Europa) with cargo install --path ., and confirm an isolated stamped-fixture read. titan install/read checks passed. Then resume Task 2; no writer work is running.
- 2026-09-17T22:57:08Z (feat/lifecycle-provenance): Europa rollout gate passed: user confirmed installation and supplied installed show output for prb-000001 preserving codex:probe / CODEX_SESSION_ID with no warnings. titan reader was already verified. Resume approved Task 2 in existing worktree; claims remain unchanged.
- 2026-09-17T23:03:09Z (feat/lifecycle-provenance): Writer implemented through shared transitions and existing park/close-message append paths. Four new CLI scenarios failed for missing behavior before implementation, then passed; full gate passed 188 unit and 315 CLI tests. Skill reference retrieval now resolves fields/markers/rollout correctly. Independent diff review is running; claims identity and liveness code unchanged.
- 2026-09-17T23:03:57Z (feat/lifecycle-provenance): Independent review approved with no blocking findings. Nonblocking approved-schema limit: user close text can equal a lifecycle marker; pair-only metadata cannot disambiguate. README and skill now state this; obs handoff will retain uncertainty. Claims, rollback/release, editor and recurring paths reviewed.
- 2026-09-17T23:04:29Z (feat/lifecycle-provenance): done
  provenance: {"harness_session":"codex:01a0b011-5b60-75e3-9188-394f5d6dc474","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-17T23:04:29Z (feat/lifecycle-provenance): Implemented lifecycle provenance in shared start/resume/close and park paths, preserving claims identity/liveness and retries. Independent review approved; 503 tests passed. Both hosts passed reader rollout. Local integration/install verification follows.
  provenance: {"harness_session":"codex:01a0b011-5b60-75e3-9188-394f5d6dc474","harness_session_source":"CODEX_SESSION_ID"}
