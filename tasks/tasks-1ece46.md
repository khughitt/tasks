---
id: tasks-1ece46
title: Determine why delegated Codex claims lose their session identity
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: investigate/tasks-1ece46
created: 2026-09-30T09:58:25Z
updated: 2026-10-06T10:59:45Z
started: 2026-10-06T09:58:07Z
completed: 2026-10-06T10:10:01Z
depends: []
parent: tasks-2fa8c6
tags: []
model: gpt-6.1-sol
agent: codex
---

Question: In one current controller/worker pair, are native IDs absent, conflicting through inheritance, or correct but resolved incorrectly, and which owner must change the contract so claims remain live and separately owned?

Where to start: docs/notes/2026-09-30-delegated-identity-brief.md; src/claims.rs::identity_from and liveness_with; src/provenance.rs::resolve_from; src/relay/resolve.rs::hint_for and resolve; tests/cli.rs::a_codex_claim_outlives_the_command_that_made_it. Recover the original ai-2f1271 capture if available before treating its account as a fresh reproduction.

Bound: one controller and one delegated worker, using disposable tasks projects with isolated XDG_CONFIG_HOME, XDG_STATE_HOME and RELAY_STATE_DIR. Record executable paths/versions, relevant identity variables (publish only presence and equality/parent-child relationships), configured relay mode, the nearest recognized process and any matching registry row, then start/show/note/park across separate command invocations. In that same pair, attempt done on a controller-claimed disposable task from the worker and record the exact refusal, effective identity, claim owner, and whether the command reaches tasks or is stopped by a harness/controller gate; have the controller clean up. Compare with a worker-owned disposable task. Do not take over a live controller claim to conceal the failure. Compare claim owner, PID/proof, liveness after command exit, ready/next exclusion, continuation ownership, and provenance independently. Use the existing codex-filter tests for synthetic missing/conflicting/agreed-input behavior; only add a targeted temporary probe if an uncovered case needs it. Do not change host settings, shared launchers, identity precedence, or product code, and do not launch a broad harness sweep. If the required worker or relay mode cannot be exercised, record that evidence gap rather than fabricating a verdict. Any spawned scratch processes must be recorded and cleaned up.

Expected result: a compact table distinguishing observations from injected controls, mapping each report to the demonstrated resolution path; a recommendation for tasks, harness configuration/adapter, relay, or the controller hook owner, with a clear regression check. Explicitly settle whether child and controller share a process and whether the available signals distinguish their logical sessions. Map tasks-290fa3, tasks-6e2d15 and tasks-f537a8 to the conflicting-ID cases, and tasks-f27fe3 and tasks-673b21 to delegated closure; a reported shared key is not proof of the same effective identity. Keep claim identity, note provenance and controller completion gating separate; do not infer that fixing one fixes all three. Update the brief. A new ownership/precedence contract requires planned work after this investigation, not an implementation hidden inside it.

Ideas it wakes: on completion add the finding to tasks-0c2c39, tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-f27fe3, tasks-f537a8, tasks-290fa3, tasks-6e2d15, tasks-673b21 with tasks note in the same commit, then rerun scope on them.

## Investigation evidence and limits

One current controller and one `spawn_agent` worker were captured on 2026-10-06,
using four disposable tasks in one project. Every invocation isolated
`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `RELAY_STATE_DIR`; native identity variables
were left unchanged. Evidence is attached to tasks-1ece46 at
`tasks/files/tasks-1ece46/delegated-identity-evidence.json`. IDs are redacted to
relationships; command PIDs are retained as process evidence.

Executables: `tasks 0.2.0` at `$HOME/.cargo/bin/tasks`; `codex-cli 0.160.1`
through `$HOME/.local/bin/codex`, resolving to the standalone release's
`bin/codex` under `$CODEX_HOME/packages/standalone/releases/0.160.1-x86_64-unknown-linux-musl/`.
The capture used the installed CLI; source checks ran at `6edc5c5` in the task
worktree. Both participants' shell commands reached the same installed tasks binary.
The host tasks config has no identity table (relay defaults off); its Codex shell
policy sets only `TASKS_FORMAT` and `TASKS_AGENT`, with neither native ID overridden.
No host relay registry was present. No host setting, launcher, desktop, or product
code changed.

| Check | Controller observation | Worker observation |
| --- | --- | --- |
| Native inputs | Both Codex variables present and equal; both equal controller rollout ID | Both present; session equals controller; thread differs and equals child rollout ID; child metadata identifies controller as parent |
| Other identity inputs | Claude variables and explicit tasks session/PID absent | Same absence |
| Process | Nearest recognized `codex` PID 2084866, start 14894621 | Exactly the same harness PID/start; logical sessions differ |
| Command session | A new Unix session leader for each invocation | A new Unix session leader for each invocation, distinct from the controller's commands |
| Registry | No matching row; host registry absent | Same absence; no real relay success demonstrated |
| Native claim after start exits | Raw native controller ID, no PID/start proof; boot ID recorded; live by four-hour TTL | `sid:<start command PID>`; PID/start/boot recorded; PID already gone; `live:false` |
| Continuation | Later stamped note refreshes the same live identity; park and close succeed | Stamped note lands with omitted provenance; stale claim is pruned rather than refreshed; park records a new command SID |
| Delegated close | Own task remains live until controller cleanup | Controller-owned close exits 1, `claimed`; worker-owned close exits 0 only after automatic stale-claim takeover |

The worker's final proof capture used command PID 3690366. Its stored `pid_start`
and `boot_id` were present, but `/proc/3690366` did not exist by the next invocation.
The tasks JSON claim view does not expose those two proof fields; the isolated store
was inspected separately. The controller claim has no PID or start proof, so sharing
its harness process does not establish worker ownership of that native claim.

The native `ready`/`next` calls returned the two unstarted relay-control tasks and
omitted both doing tasks. That does **not** prove liveness-based exclusion, because
the local records already said `doing`; `tasks claims` independently showed the
controller live and the worker dead. The existing cross-checkout and rival-start
fixtures cover live claim exclusion. There was no second controller or worker.

The exact native delegated refusal, with identity and scratch path redacted, was:

```json
{"error":{"detail":"probe-441b81 is claimed by session <controller-native> (owner keith, host <host>, worktree <scratch>/project, since 2026-10-06T09:59:44Z, age 37s, live)","kind":"claimed"}}
```

This reached tasks; no harness/controller gate prevented execution. The worker's
effective identity was its current command SID, not either exported Codex ID.
`src/commands/mod.rs::ownership` requires matching effective identity or valid
relay-mode process proof; `claim_guard` refuses a foreign live claim. Plain-language
permission from the controller does not supply either. Worker-owned closure warned
that the previous command PID was gone and used the stale-claim takeover path. Its
success is not evidence of stable continuation ownership.

| Injected control | Result and limit |
| --- | --- |
| Scratch relay enabled, real native/process inputs, empty schema-2 registry | Worker start exits 1, `config`: conflicting session/controller and thread/worker hints, with explicit-session recovery instruction. Worker delegated close reports the same conflict plus the held controller identity. Hints are checked before registry loading. Controller start exits 1, `config`: no agent matches PID 2084866. This is not a captured real relay match. |
| Existing Codex fixtures | `just test-one codex`: six tests pass, covering agreeing/thread-only inputs, conflict fallback, claim survival, and relay hint disagreement. Missing-input Unix fallback is covered by the fast suite. These are synthetic inputs, not extra live worker captures. |
| Existing provenance fixtures | `just test-one provenance`: eleven tests pass, including missing/agreeing/conflicting inputs, lifecycle metadata, and opt-in stamped notes. `just test-fast` also passes. |
| Pure controller Stop-policy check | `ops/hooks/claim-guard::decide`, injected Stop events and captured live/dead claim rows: controller blocked on its own live claim, child allowed with no matching live claim. This tests the decision function, not actual Stop delivery or controller wake. |

Current resolution is demonstrated, rather than inferred from the reports:

- `src/claims.rs::identity_from` skips the native Codex level when the pair conflicts;
  Unix fallback records the command session leader. `liveness_with` sees that PID gone
  after exit and immediately marks the claim stale. The agreeing controller uses TTL
  liveness. This is the existing deliberate contract, including the native level from
  `903f04a` (tasks-3190fb).
- `src/provenance.rs::resolve_from` independently rejects the same conflicting pair;
  lifecycle and `note --stamp` metadata are omitted with warnings. Plain notes are
  unstamped by design. Explicit claim overrides do not provide native provenance.
  The independent note-stamping capability in tasks-0005a6 is done.
- `src/relay/resolve.rs::hint_for` rejects the worker pair before loading the registry.
  `resolve` also requires exactly one qualifying process row. Shared process proof
  cannot select two logical sessions by itself; the native child thread signal can
  distinguish this pair, but how to trust/deliver it requires a reviewed contract.
- `ops/hooks/claim-guard::held` filters by live claim and exact/known-prefixed Stop
  session ID. A separately live child identity would still not match a controller's
  Stop ID. Native identity repair alone therefore does not solve controller aggregation.

| Reports | Demonstrated path and remaining historical limit |
| --- | --- |
| tasks-0c2c39 (`ai-2f1271`, now tack-2f1271) | Original controller transcript recovered: tool-result line 1174 records `sid:1112013`, `live:false`; line 1178 removes the scratch capture. No actual native-variable presence capture survived. Missing IDs remain the reporter's inference; current reproduction demonstrates conflicting inherited IDs instead. |
| tasks-5745bb, tasks-616cd7, tasks-cf8687 | Current pair reproduces conflict → Unix fallback → dead claim, plus independent omitted provenance. Their historical pair/config inputs remain uncaptured; the reported shared Unix identity is not established by this pair's distinct command SIDs. |
| tasks-290fa3, tasks-6e2d15, tasks-f537a8 | Same current conflict path; parent session versus child thread is now demonstrated for this pair. This maps the symptoms, without proving every earlier shell used this exact environment. |
| tasks-f27fe3, tasks-673b21 | Current delegated close is a tasks `claimed` refusal against a live controller identity. Sharing an exported parent key or harness process does not mean matching effective identity. Historical refusals were not captured; another layer may have refused those original calls. |

All disposable tasks were closed/dropped by their appropriate owner or controller
cleanup; final isolated `tasks claims` was empty. No detached scratch process was
launched. The original evidence scratch was already deleted by its own controller.

## Notes

- 2026-10-02T14:37:26Z (main): scope follow-up: tasks-cf8687 joins the existing conflict/provenance investigation and Ideas it wakes list; tasks-616cd7 recurred without capture evidence; task remains one bounded controller/worker capture
- 2026-10-06T09:45:47Z (main): scope follow-up: five newer worker reports join the existing bounded capture; record delegated done evidence independently of conflicting native IDs; wake every processed report on completion
- 2026-10-06T09:58:07Z (main): started
  provenance: {"harness_session":"codex:01a110a5-ab41-7552-baa2-c9410cb5dce6","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T09:58:16Z (main): Direct investigation; capture one current controller/worker pair in disposable isolated projects, trace identity/liveness/provenance/closure separately, and update the brief in .worktrees/tasks-1ece46 without product changes.
- 2026-10-06T09:58:24Z (investigate/tasks-1ece46): resumed
  provenance: {"harness_session":"codex:01a110a5-ab41-7552-baa2-c9410cb5dce6","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T10:06:35Z (investigate/tasks-1ece46): attached: delegated-identity-evidence.json (24019 bytes): One controller/worker capture; native equality relationships, command exits, process proofs, injected controls, original-source limits, cleanup.
- 2026-10-06T10:07:12Z (investigate/tasks-1ece46): Capture reproduced inherited controller session plus child thread in one shared Codex process; native claim dead after command exit, native provenance omitted, foreign live close refused by tasks, own close succeeds through stale takeover. Relay/Stop controls explicitly synthetic; host registry absent. Nine report findings and scope audits recorded; planned tasks-5e2472 filed and existing tasks-56b450 reused; relay-60ae9e recurrence filed.
- 2026-10-06T10:08:21Z (investigate/tasks-1ece46): Verification: just test-one codex (6 passed), just test-one provenance (11 passed), just test-fast and just check passed; attached JSON validated for native-ID redaction and empty scratch claims. No product code changed.
- 2026-10-06T10:10:01Z (investigate/tasks-1ece46): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-06T10:10:01Z (investigate/tasks-1ece46): done
  provenance: {"harness_session":"codex:01a110a5-ab41-7552-baa2-c9410cb5dce6","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T10:10:01Z (investigate/tasks-1ece46): Captured one Codex controller/worker pair: inherited parent session conflicts with child thread, causing dead Unix claims and omitted provenance; tasks rejects delegated live-controller closure. Sanitized evidence attached, brief updated, nine ideas awakened and scoped, planned tasks-5e2472 filed; focused tests, fast suite, checks and review pass.
  provenance: {"harness_session":"codex:01a110a5-ab41-7552-baa2-c9410cb5dce6","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T10:41:53Z (main): review: impl round 2 — verdict: revise; findings: Important 1, Minor 1; reviewer: human
- 2026-10-06T10:41:53Z (main): Review correction: the capture establishes parent-session/child-thread disagreement and tasks fallback, not defective Codex delivery. Full evidence tables now live on this task alongside the existing sanitized attachment; source semantics must be checked before recommending an upstream repair.
- 2026-10-06T10:59:44Z (main): review: impl round 3 — verdict: accept; findings: none; reviewer: unspecified
- 2026-10-06T10:59:44Z (main): Evidence limit correction: the injected child Stop used its own child session_id, whereas documented subagent hooks carry parent session_id and worker events carry agent_id. The review identifies SubagentStop, not child Stop, as the real completion event. Preserve the injected result solely as a pure policy control; it does not establish subagent event delivery or controller/worker lifecycle coverage. tasks-c0bad3 verifies exact-release dispatch and agent_id/thread equality.
