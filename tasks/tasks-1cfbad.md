---
id: tasks-1cfbad
title: Verify relay identity against a real agents.json before recommending the opt-in
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: main
created: 2026-09-22T16:41:10Z
updated: 2026-09-23T11:22:14Z
started: 2026-09-23T11:22:14Z
depends: []
tags: []
agent: "claude-code/claude-opus-5[1m]"
---

Everything in the relay-ancestry work was validated against a synthetic registry written by a shell shim (tests/common/mod.rs WRITE_REGISTRY). The reader mirrors relay's validators field for field and the matcher agrees with relay's own resolveAgentPid on which process it names, but no tasks command has ever matched a registry relay actually produced.

What to establish, on a host where relay is running:
- turn on [identity] relay = true and run tasks start inside a Claude Code and a Codex session; confirm the claim is keyed <harness>:<sessionId> and carries pid_start and boot_id
- confirm the published process.pid is the process whose comm is claude/codex, as relay's adapters imply, and not a wrapper
- confirm the start token relay writes is the same /proc starttime tick count the ancestry walk reads
- exercise a headless session (no controlling terminal) and record what the refusal says; see tasks-962300

Note for the write-up: on a host where the shell itself runs under an agent session, every tasks command is in scope once the opt-in is on, so the switch should only go on where relay actually publishes agents.json. Turning it on without relay running refuses every acquisition, by design (spec section 6.5).

Design: docs/specs/2026-09-22-relay-ancestry-identity-design.md

## Notes

- 2026-09-23T11:22:14Z (main): started
  provenance: {"harness_session":"claude-code:22eb583d-5fc2-4364-a75c-e302907e2e4f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
