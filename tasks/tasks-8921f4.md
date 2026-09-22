---
id: tasks-8921f4
title: Design opt-in relay ancestry identity
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: main
created: 2026-09-17T21:08:29Z
updated: 2026-09-22T13:15:42Z
started: 2026-09-22T13:09:21Z
depends: [relay-06b1da]
parent: tasks-c9199a
tags: []
source: tasks-c9199a
agent: codex
---

Approved source: ops docs/specs/2026-09-16-relay-design.md (reviewed after d7e2c33); execution brief: ops docs/plans/2026-09-16-relay-bootstrap.md (approved after bc53c51 with review corrections).

Next: write the separate tasks identity design for review, then its implementation plan.

Read tasks `src/claims.rs`, `tests/cli.rs`, and
`docs/specs/2026-09-05-work-claims-design.md`. Design opt-in configuration and
identity continuity before implementation; preserve all spec §6 tasks constraints.
Outside recognized harness ancestry, native identity wins without opening the
registry, even with host-wide relay enabled. Unknown ancestry is not a shell.
Inside it, match the nearest harness using same-host PID/start/Linux boot evidence;
do not skip an unmatched inner harness. Missing, ambiguous or unavailable proof is
an acquisition error with explicit-identity recovery. Claude hints must agree with
process proof; shared OpenCode processes do not distinguish independent subagents.

Rust reads schema 1 directly without Node. Copy relay-06b1da's versioned handle fixtures and
label the oversized Linux value as a format test. Include the real Codex case:
live harness ancestor, empty snapshot before its first turn's SessionStart hook.
Test explicit TASKS_SESSION/TASKS_SESSION_PID precedence, nested harnesses, plain
shells with corrupt/missing registry, and held-claim refresh/release after registry
loss. Keep existing Live/Stale, takeover/release retries, mutation locking and TTL.
Darwin epochs must not enter Linux pid_start fields; keep its existing native
unverifiable path. Incompatible schema changes coordinate the producer and reader.

Prerequisites: relay-06b1da. Release goal: relay-1231bf. Bootstrap ops-998bbb closure does not mean this deliverable has shipped.

## Notes

- 2026-09-22T13:09:21Z (main): started
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T13:14:59Z (main): Design decision 1: the relay-identity opt-in is a new host config file ~/.config/tasks/config.toml, not the registry, an env var, or the committed per-project tasks/.config.toml (which syncs across hosts). Decision 2: in scope, the matched relay Agent.id becomes the claim session for every recognized harness including Claude and Codex, per ops relay-design 6.2; native session vars are a hint that must agree with process proof. Boundary: identity() is reached only from claim_guard/refuse_foreign_live_claim, so ancestry walk and registry read stay on claim-mutation paths.
- 2026-09-22T13:15:42Z (main): Relay schema 1 read: Agent.id is <harness>:<sessionId> over {claude-code, codex, opencode}; process handle is {platform, host, bootId, pid, start} or null; opencode scope is 'process', others 'session'; snapshot is {schema:1, generation, revision, agents}. Registry lives at RELAY_STATE_DIR, else XDG_STATE_HOME/relay, else ~/.local/state/relay, dir 0700 and files 0600, and relay refuses a non-private path. Consequence: the matched Agent.id for Claude and Codex is the same qualified key the lifecycle notes already carry, so for those two harnesses the registry read verifies rather than discovers; only OpenCode needs discovery.
