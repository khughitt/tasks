---
id: tasks-1ece46
title: Determine why delegated Codex claims lose their session identity
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-30T09:58:25Z
updated: 2026-10-06T09:45:48Z
depends: []
parent: tasks-2fa8c6
tags: []
agent: codex
---

Question: In one current controller/worker pair, are native IDs absent, conflicting through inheritance, or correct but resolved incorrectly, and which owner must change the contract so claims remain live and separately owned?

Where to start: docs/notes/2026-09-30-delegated-identity-brief.md; src/claims.rs::identity_from and liveness_with; src/provenance.rs::resolve_from; src/relay/resolve.rs::hint_for and resolve; tests/cli.rs::a_codex_claim_outlives_the_command_that_made_it. Recover the original ai-2f1271 capture if available before treating its account as a fresh reproduction.

Bound: one controller and one delegated worker, using disposable tasks projects with isolated XDG_CONFIG_HOME, XDG_STATE_HOME and RELAY_STATE_DIR. Record executable paths/versions, relevant identity variables (publish only presence and equality/parent-child relationships), configured relay mode, the nearest recognized process and any matching registry row, then start/show/note/park across separate command invocations. In that same pair, attempt done on a controller-claimed disposable task from the worker and record the exact refusal, effective identity, claim owner, and whether the command reaches tasks or is stopped by a harness/controller gate; have the controller clean up. Compare with a worker-owned disposable task. Do not take over a live controller claim to conceal the failure. Compare claim owner, PID/proof, liveness after command exit, ready/next exclusion, continuation ownership, and provenance independently. Use the existing codex-filter tests for synthetic missing/conflicting/agreed-input behavior; only add a targeted temporary probe if an uncovered case needs it. Do not change host settings, shared launchers, identity precedence, or product code, and do not launch a broad harness sweep. If the required worker or relay mode cannot be exercised, record that evidence gap rather than fabricating a verdict. Any spawned scratch processes must be recorded and cleaned up.

Expected result: a compact table distinguishing observations from injected controls, mapping each report to the demonstrated resolution path; a recommendation for tasks, harness configuration/adapter, relay, or the controller hook owner, with a clear regression check. Explicitly settle whether child and controller share a process and whether the available signals distinguish their logical sessions. Map tasks-290fa3, tasks-6e2d15 and tasks-f537a8 to the conflicting-ID cases, and tasks-f27fe3 and tasks-673b21 to delegated closure; a reported shared key is not proof of the same effective identity. Keep claim identity, note provenance and controller completion gating separate; do not infer that fixing one fixes all three. Update the brief. A new ownership/precedence contract requires planned work after this investigation, not an implementation hidden inside it.

Ideas it wakes: on completion add the finding to tasks-0c2c39, tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-f27fe3, tasks-f537a8, tasks-290fa3, tasks-6e2d15, tasks-673b21 with tasks note in the same commit, then rerun scope on them.

## Notes

- 2026-10-02T14:37:26Z (main): scope follow-up: tasks-cf8687 joins the existing conflict/provenance investigation and Ideas it wakes list; tasks-616cd7 recurred without capture evidence; task remains one bounded controller/worker capture
- 2026-10-06T09:45:47Z (main): scope follow-up: five newer worker reports join the existing bounded capture; record delegated done evidence independently of conflicting native IDs; wake every processed report on completion
