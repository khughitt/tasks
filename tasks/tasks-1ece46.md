---
id: tasks-1ece46
title: Determine why delegated Codex claims lose their session identity
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-30T09:58:25Z
updated: 2026-09-30T09:58:25Z
depends: []
parent: tasks-2fa8c6
tags: []
agent: codex
---

Question: In one current controller/worker pair, are native IDs absent, conflicting through inheritance, or correct but resolved incorrectly, and which owner must change the contract so claims remain live and separately owned?

Where to start: docs/notes/2026-09-30-delegated-identity-brief.md; src/claims.rs::identity_from and liveness_with; src/provenance.rs::resolve_from; src/relay/resolve.rs::hint_for and resolve; tests/cli.rs::a_codex_claim_outlives_the_command_that_made_it. Recover the original ai-2f1271 capture if available before treating its account as a fresh reproduction.

Bound: one controller and one delegated worker, using disposable tasks projects with isolated XDG_CONFIG_HOME, XDG_STATE_HOME and RELAY_STATE_DIR. Record executable paths/versions, relevant identity variables (publish only presence and equality/parent-child relationships), configured relay mode, the nearest recognized process and any matching registry row, then start/show/note/park across separate command invocations. Compare claim owner, PID/proof, liveness after command exit, ready/next exclusion, continuation ownership, and provenance independently. Use the existing codex-filter tests for synthetic missing/conflicting/agreed-input behavior; only add a targeted temporary probe if an uncovered case needs it. Do not change host settings, shared launchers, identity precedence, or product code, and do not launch a broad harness sweep. If the required worker or relay mode cannot be exercised, record that evidence gap rather than fabricating a verdict. Any spawned scratch processes must be recorded and cleaned up.

Expected result: a compact table distinguishing observations from injected controls, mapping each report to the demonstrated resolution path; a recommendation for tasks, harness configuration/adapter, relay, or the controller hook owner, with a clear regression check. Explicitly settle whether child and controller share a process and whether the available signals distinguish their logical sessions. Keep claim identity, note provenance and controller completion gating separate; do not infer that fixing one fixes all three. Update the brief. A new ownership/precedence contract requires planned work after this investigation, not an implementation hidden inside it.

Ideas it wakes: on completion add the finding to tasks-0c2c39, tasks-5745bb and tasks-616cd7 with tasks note in the same commit, then rerun scope on them.
