---
id: tasks-29f71f
title: Establish a supported recovery from merged stale task records
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: docs/tasks-29f71f
created: 2026-10-06T09:45:48Z
updated: 2026-10-06T11:20:30Z
started: 2026-10-06T11:13:59Z
completed: 2026-10-06T11:19:01Z
depends: []
parent: tasks-984769
tags: []
source: tasks-a20a64
agent: codex
---

Question: Which exact merge operation makes a divergent record writable without losing either note history or bypassing stale-copy checks?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/stale.rs::compare/refusal, src/commands/mod.rs::load/refuse_stale_copy, src/time.rs::after and record-home spec sections 3.1-3.2.

Bound: one disposable git main/worktree pair with isolated config/state; use CLI notes to create divergence, compare normal git integration of the authoritative record with a union merge retaining the source stamp, and record the retry result. Task files are written only by the CLI or git merge/conflict resolution under the repository protocol; do not prescribe a hand-edited timestamp, add an override or change product code.

Expected result: a reproducible, lossless supported recovery and precise diagnostic/doc recommendation, or a concrete contract gap requiring reviewed design; update the brief.

Ideas it wakes: on completion add the finding to tasks-a20a64 with tasks note in the same commit, update the brief, then rerun scope on them.

## Notes

- 2026-10-06T09:45:48Z (main): concerns: tasks-9949f3 defect — Reported merge recovery can remain refused despite preserving the newer notes; reproduce the diagnostic gap before prescribing a fix.
- 2026-10-06T11:13:59Z (main): started
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:14:09Z (docs/tasks-29f71f): resumed
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:17:13Z (docs/tasks-29f71f): attached: tasks-29f71f-recovery.py (6076 bytes): Runnable disposable Git main/worktree recovery probe; CLI-authored histories and Git conflict resolution, no stamp edits.
- 2026-10-06T11:17:13Z (docs/tasks-29f71f): attached: tasks-29f71f-recovery-result.json (16263 bytes): Current binary transcript: clean handoff passes, source-stamped union refuses, identical resolved siblings permit lossless CLI retry.
- 2026-10-06T11:17:13Z (docs/tasks-29f71f): Observed lossless recovery: fast-forward a behind-only copy; for divergent histories retain the source header verbatim and union notes during Git conflict resolution, then integrate that resolved record into both equal-stamp siblings before CLI retry. The CLI advances updated and retains every earlier note. A one-sided union remains stale_copy; no fresh hand-edited stamp or override is necessary.
- 2026-10-06T11:19:01Z (docs/tasks-29f71f): review: impl round 1 — verdict: accept; findings: none; reviewer: codex
- 2026-10-06T11:19:01Z (docs/tasks-29f71f): done
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:19:01Z (docs/tasks-29f71f): Established lossless recovery with an isolated CLI/Git probe: ordinary handoff passes; one-sided source-stamped union refuses unchanged; integrating resolved bytes in both siblings permits CLI retry with original notes intact and updated advanced. Attached script/transcript, updated brief and scoped tasks-a20a64 guidance follow-up. just check and independent review passed; product code unchanged.
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:19:43Z (docs/tasks-29f71f): detached: tasks-29f71f-recovery.py: Add transcript redaction for work-link configured checkout root.
- 2026-10-06T11:19:43Z (docs/tasks-29f71f): attached: tasks-29f71f-recovery.py (6302 bytes): Runnable isolated recovery probe; fixture paths and work-link host root redacted in exported transcript.
- 2026-10-06T11:19:43Z (docs/tasks-29f71f): detached: tasks-29f71f-recovery-result.json: Replace raw work-link host root with a descriptive redaction; repeat all recovery assertions.
- 2026-10-06T11:19:43Z (docs/tasks-29f71f): attached: tasks-29f71f-recovery-result.json (16273 bytes): Repeated passing recovery probe, with host checkout-root value redacted.
- 2026-10-06T11:19:43Z (docs/tasks-29f71f): Commit hygiene caught a host checkout-root path in work-link output; redacted that host-only value, repeated the passing probe and replaced both attachments through the CLI. Tool feedback: ops-1164e6.
- 2026-10-06T11:20:30Z (docs/tasks-29f71f): review: impl round 2 — verdict: accept; findings: none; reviewer: codex
