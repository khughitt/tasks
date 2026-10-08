---
id: tasks-586d78
title: "Review, approve and activate the coordinated cutover"
status: done
priority: 1
size: m
complexity: mid
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:34Z
updated: 2026-10-08T22:58:55Z
started: 2026-10-08T21:11:37Z
completed: 2026-10-08T22:58:55Z
depends: [tasks-50f850, tasks-468bc7, tasks-1ffde6, tasks-7b4938]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-6"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 6: Review, approve and activate the coordinated cutover"
---

Execute Task 6: Review, approve and activate the coordinated cutover only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:34Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T21:11:37Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T21:16:15Z (docs/claim-guard-design): Final owner verification exits 0: ops RealFactsTests 4 pass/no skips, fast 1276 with one existing skip, hygiene; hq full gate 226 agents +553 hook/tools and live facts validation; tasks fast 1159 with one ignored and hygiene. Corrected native pilot A–E and both selected native-home sources pass. Prepare fresh whole-branch review package; live merges and home trust remain unapproved.
- 2026-10-08T21:22:11Z (docs/claim-guard-design): Final review correction required: RealFactsTests copies current hq values then pins Codex false/unknown and mutates literal current strings. Normalize explicit disposable scenarios independent of source values/status, preserve real validator/reader, regress valid changed source facts. Minor detached-warning fragment and previously deferred matcher regression remain deferred. Production/lifecycle review found no blocker.
- 2026-10-08T21:35:30Z (docs/claim-guard-design): Fresh corrective re-review accepted ops bd514215 with no findings: real-reader tests now source-independent, actual registered default still validated, RED two failures then 88 focused incl5 real/no skips and1277 fast/existing skip1 plus hygiene green. Runtime guard unchanged and native pilot remains applicable. Both Minor findings deferred; prepare exact live cutover approval packet with corrected ops tip.
- 2026-10-08T21:36:26Z (docs/claim-guard-design): Concrete cutover packet ready at docs/notes/2026-10-08-claim-guard-cutover.md: hq mirror a3662f6+c9c787b, ops corrected tip bd514215, hq wiring116b0d7; both JSON diffs, unchanged facts hash, current mains, resolved live paths and coordinated rollback. Corrected ops merge-tree clean; relevant main files/heads unchanged and no conflicting draft. Both actual native homes show hooks/multi_agent enabled. Fresh final and scoped corrective reviews accepted. Live merge/trust approval remains the only external gate.
- 2026-10-08T21:44:27Z (docs/claim-guard-design): parked (waiting on user, approval): User approves .worktrees/claim-guard-design/docs/notes/2026-10-08-claim-guard-cutover.md; agent rechecks mains/facts and applies hq mirror phase, ops bd514215, hq worker wiring, then normal native trust in both homes, activated gates, record closeout and owned cleanup.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T22:38:45Z (docs/claim-guard-design): resumed
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T22:39:15Z (docs/claim-guard-design): Cutover phase 1 applied: hq main fast-forwarded to c9c787b, including mirror removal a3662f6 and corrected owner metadata. Explicit hashes confirm capability facts and both Stop-only hook files unchanged; ops live guard remains old. No capability edits or worker activation during this interval.
- 2026-10-08T22:39:27Z (docs/claim-guard-design): Cutover phase 2 applied: normal ops main merge integrates bd514215 (runtime91984f5 plus reviewed tests), preserving unrelated main commits. Live guard bytes match reviewed candidate, installed facts endpoint resolves to hq main and capability hash is unchanged. Worker wiring still Stop-only until phase 3.
- 2026-10-08T22:39:46Z (docs/claim-guard-design): Cutover phase 3 applied: hq main fast-forwarded to reviewed wiring116b0d7. Both actual personal/work installed files resolve to hq main and match candidate bytes; each has one Stop guard and one no-matcher SubagentStop guard, other hooks unchanged. Establish ordinary exact-definition native trust in both homes before claiming activation.
- 2026-10-08T22:49:06Z (docs/claim-guard-design): Both ordinary live-home native browsers trusted only the new SubagentStop command after showing one new hook each. Personal source ~/.codex/hooks.json and work source ~/.codex-work/hooks.json display trusted/active worker1/1 and existing Stop3/3 active; other trust settings retained. Both existing auth routes ready, no login flow/model task, browsers exited and groups reaped. Explicit-main synthetic CLI-generated controls pass worker own/root own+related selection, both retries, unchanged claims, empty inventory; disposable registry/state removed. Live heads ops62556f3/hq116b0d7, facts hash unchanged. Activated owner gates still tracked.
- 2026-10-08T22:57:13Z (docs/claim-guard-design): Approved activation verified: hq mirror c9c787b then ops merge62556f3 then hq wiring116b0d7; personal/work ordinary native trust active and current saved hashes verified. Hq tracked personal UI-generated hash persisted9c7f432 per policy; work local settings untracked. Facts unchanged, endpoints resolve mains, explicit-main synthetic controls pass. Merged gates: ops88 focused inclreal5/no skips, fast1279/existing skip1+hygiene; hq226+553. No rollback, model task/login flow or retained native process. Ledger copied to docs/notes/2026-10-08-claim-guard-execution-ledger.md before cleanup.
- 2026-10-08T22:58:55Z (docs/claim-guard-design): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T22:58:55Z (docs/claim-guard-design): Approved coordinated cutover applied and trusted in both native homes; merged gates, unchanged facts/paths and explicit-main controls verified; reviewed rollback retained and owned cleanup checked.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
