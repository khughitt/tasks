---
id: tasks-4ca7bd
title: "tasks prime's default JSON runs to ~250 KB on a project with ~230 open tasks (every ready row in full), past an agent tool's output limit, so the session-start read lands in an overflow file; a capped or summary default would fit one read"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T02:03:30Z
created: 2026-10-19T11:09:12Z
updated: 2026-10-20T14:27:34Z
depends: [tasks-e7424b]
parent: tasks-96c00c
tags: [feedback, friction, "from:mind6", "from:tasks"]
---

Scope handoff: [ref:brief] records the current code, unresolved contract or measurement question, and the follow-up that must answer it before implementation is scoped.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — current evidence, constraints and next questions

## Notes

- 2026-10-20T01:39:05Z (feedback): feedback from tasks: The default prime JSON exceeded the agent tool output limit; extracting summary fields was needed to finish reading session state.
- 2026-10-20T01:53:01Z (feedback): feedback from tasks: The default prime JSON exceeded the agent tool output budget; filtering its summary fields was required to read the session state.
- 2026-10-20T01:55:14Z (feedback): feedback from tasks: tasks prime JSON exceeds an agent tool's output budget, so the session-start read is truncated and requires a separate filtered read
- 2026-10-20T02:03:30Z (main): scope: briefed; local response measured 101001 bytes, dominated by roadmap and ready; summary/default contract needs design; waits on tasks-e7424b; evidence: 89c58a7 and tasks 0.4.0; brief: docs/notes/2026-10-19-cli-consumer-reads-brief.md
- 2026-10-20T02:03:37Z (feedback): feedback from tasks: Default prime JSON was truncated at session start; a filtered read was needed to inspect session state.
- 2026-10-20T09:12:42Z (feedback): feedback from tasks: The default tasks prime JSON exceeded the agent tool output limit during session startup
- 2026-10-20T09:12:42Z (feedback): detail from tasks: The session-start tool response was truncated at roughly 26,000 tokens; a locally filtered summary was needed to inspect counts, active work, closeout and warnings.
- 2026-10-20T14:27:34Z (feedback): feedback from tasks: prime's default JSON exceeded the tool response limit during session startup; a jq projection was needed to read the status and warnings
