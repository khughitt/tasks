---
id: tasks-c62927
title: Conform tasks to the shared CLI vocabulary and check the clap surface against tools/cli.toml
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: tasks-c62927
created: 2026-09-20T11:30:32Z
updated: 2026-09-20T13:48:45Z
started: 2026-09-20T13:10:56Z
completed: 2026-09-20T13:48:45Z
depends: []
tags: [cli, cross-project]
agent: claude-code/claude-opus-5
---

Adopt the shared CLI vocabulary: vendor tools/cli.toml (and tools/cli_surface.py), make the parser conform, add the conformance test. The steps are Task 2 in the ops plan docs/plans/2026-09-20-cli-conventions.md (spec docs/specs/2026-09-20-cli-conventions-design.md). Waits for the ops table task ops-c9ecf0 to land on ops main; the ops step ops-575b11 tracks this piece.

## Notes

- 2026-09-20T13:10:56Z (main): started
  provenance: {"harness_session":"claude-code:20e55bde-4aed-4a6f-993d-b44b058f509f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-20T13:38:51Z (tasks-c62927): parked (waiting on agent, dependency): re-vendor tools/cli.toml once the table marks --depends repeatable on add and edit and feedback --category required, rerun just test, then tasks done
  provenance: {"harness_session":"claude-code:20e55bde-4aed-4a6f-993d-b44b058f509f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-20T13:48:10Z (tasks-c62927): resumed
  provenance: {"harness_session":"claude-code:20e55bde-4aed-4a6f-993d-b44b058f509f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-20T13:48:45Z (tasks-c62927): done
  provenance: {"harness_session":"claude-code:20e55bde-4aed-4a6f-993d-b44b058f509f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-20T13:48:45Z (tasks-c62927): tools/cli.toml vendored and checked against the clap surface (src/surface.rs); parser conforms: --json, ValueSet-enforced closed sets (exit 2), sample --limit and --older-than <age>, value-name kinds, sort defaults, two-line usage errors; behaviour tests in tests/cli.rs; docs updated
  provenance: {"harness_session":"claude-code:20e55bde-4aed-4a6f-993d-b44b058f509f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
