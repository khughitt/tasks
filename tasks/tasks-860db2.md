---
id: tasks-860db2
title: Judge claim liveness only from the PID view that recorded it
status: doing
priority: 1
size: m
complexity: mid
process: planned
owner: fix/claim-pid-view
created: 2026-10-14T19:05:20Z
updated: 2026-10-14T22:05:13Z
started: 2026-10-14T19:36:56Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-fe6c59
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-14-claim-pid-view-design.md
plan: docs/plans/2026-10-14-claim-pid-view.md
---

Defect found by tasks-fe6c59 (evidence: tasks/files/tasks-fe6c59/pidns-proof-evidence.json, cases prune-*, codex-*, ns-explicit-pid1, host-view-of-ns-pid).

`claims::liveness_with` reads `/proc/<claim.pid>/stat` in the caller's own PID view, and nothing records which view the pid came from. Every claim write calls `ClaimStore::prune_dead`. From a command in a child PID namespace with a private /proc (Codex workspace-write sandbox shape), a live host-view claim reads "pid gone". Any claim write there deletes it silently: no warning, no takeover note. The task record stays doing, and another session then starts it without --force. The reverse also holds. A pid recorded inside a namespace (explicit TASKS_SESSION_PID=1) reads as "reused" from the host, which takes it over without --force. Affected: relay claims, explicit-pid claims, CLAUDE_PID claims and sid:<pid> claims. Pid-less TTL claims are not affected. Reproduced with unshare and inside the genuine `codex sandbox` (0.162.1).

Exposure: a host whose Codex permission profile makes the claim store writable from the sandbox, while Claude Code claims carry CLAUDE_PID host pids. This repository's author host has both. Not yet observed in a real model session.

Design questions to settle (revised after review of tasks-fe6c59):

- Verdict when views differ or are unknown. Falling back to the TTL is not fail-closed. The TTL expires four hours after `seen`, while a verified live process stays live indefinitely, and a Stale verdict permits takeover without --force (`claim_guard`), pruning (`prune_dead`) and the release of exclusive holds (`holds.rs`). A host session running one long command would lose protection from a sandboxed caller after four hours, even with pruning disabled. A mismatched or unknown PID view must therefore be insufficient evidence for automatic takeover, pruning or hold release. It is neither Live-by-proof nor Stale, which probably means a third liveness verdict. Explicit `--force` stays the takeover path; decide whether the recording view alone reaps such claims. Test: a live claim whose `seen` is older than four hours, judged from a different view, is neither taken over, pruned nor released.
- Which view a PID belongs to. `/proc/self/ns/pid` names the caller's namespace, not the /proc view. Linux numbers /proc by the PID namespace of the procfs mount. Case ns-plain-host-proc shows a child-namespace caller reading host-numbered PIDs through an inherited /proc (`NSpid: <host> 3`). Establish separately (a) which namespace numbered the recorded PID: relay's handle comes from the producer's procfs view, `CLAUDE_PID`/`TASKS_SESSION_PID` from whichever process exported them, and `sid:` from the caller's `/proc/self/stat`; and (b) which namespace supplies the /proc lookup at evaluation time. Candidate signal to verify: the number of `NSpid` entries in `/proc/self/status`, where one entry means the procfs numbers PIDs in the caller's own namespace and more means an ancestor's. Compare like with like, so compatible views are not misclassified and incompatible ones are not compared. Cases: same view; inherited host /proc in a child namespace (compatible with host-numbered PIDs); private /proc (incompatible); and a PID recorded inside a namespace evaluated from the host.
- Identity for a view. Namespace inodes are reused once a namespace dies, so pair any view identity with the start-time check. Settle legacy claims without a recorded view under the same rule: an unknown view is not evidence of death.
- Whether to refuse `TASKS_SESSION_PID` that names a PID outside the evaluating views' common numbering.
- Whether a relay walk that ends Outside at a pid 1 in a non-initial namespace should refuse instead of silently falling to the native level (truncated ancestry; case ns-plain-private-proc).

Tests: inject the view the way `liveness_with` injects stat.

## Notes

- 2026-10-14T19:35:16Z (main): revised from review of tasks-fe6c59: a cross-view or unknown view is insufficient evidence for takeover, prune or hold release (TTL fallback rejected; a live >4h claim must survive); PID view is the procfs mount's numbering, not /proc/self/ns/pid; inherited-host-/proc and private-/proc cases required
- 2026-10-14T19:36:56Z (main): started
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T19:37:06Z (fix/claim-pid-view): resumed
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T19:42:14Z (fix/claim-pid-view): parked (waiting on user, review): user reviews docs/specs/2026-10-14-claim-pid-view-design.md in .worktrees/claim-pid-view; on approval the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T20:06:38Z (fix/claim-pid-view): review: spec round 1 — verdict: revise; findings: must-fix 2, should-fix 1; reviewer: human
- 2026-10-14T20:06:40Z (fix/claim-pid-view): resumed
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T20:08:09Z (fix/claim-pid-view): spec revision 2: view = procfs's pid namespace (nsfs dev:ino via NSpid==1), unknown otherwise; exported pids establish a view only through the ancestry walk; heartbeats keep established evidence; mixed versions documented unsafe
- 2026-10-14T20:08:10Z (fix/claim-pid-view): parked (waiting on user, review): user reviews spec revision 2 (docs/specs/2026-10-14-claim-pid-view-design.md in .worktrees/claim-pid-view); on approval the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T20:41:11Z (fix/claim-pid-view): review: spec round 2 — verdict: revise; findings: P1 1; reviewer: human
- 2026-10-14T20:41:12Z (fix/claim-pid-view): resumed
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T20:42:12Z (fix/claim-pid-view): spec revision 3: exported pids establish a view only when the writer's known view is the initial PID namespace (PROC_PID_INIT_INO), never by numeric/ancestry match; colliding-ancestor acceptance case added
- 2026-10-14T20:42:13Z (fix/claim-pid-view): parked (waiting on user, review): user reviews spec revision 3 (docs/specs/2026-10-14-claim-pid-view-design.md in .worktrees/claim-pid-view); on approval the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T21:48:59Z (fix/claim-pid-view): review: spec round 3 — verdict: accept; findings: none; reviewer: human
- 2026-10-14T21:49:00Z (fix/claim-pid-view): resumed
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T22:05:13Z (fix/claim-pid-view): parked (waiting on user, review): user reviews docs/plans/2026-10-14-claim-pid-view.md in .worktrees/claim-pid-view and picks the execution method; then the agent runs the Setup baseline and starts tasks-56d39d (Task 1)
  provenance: {"harness_session":"claude-code:fb19dd4f-635e-4934-86ce-7f6186000af8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
