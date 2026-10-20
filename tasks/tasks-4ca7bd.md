---
id: tasks-4ca7bd
title: "tasks prime's default JSON runs to ~250 KB on a project with ~230 open tasks (every ready row in full), past an agent tool's output limit, so the session-start read lands in an overflow file; a capped or summary default would fit one read"
status: idea
priority: 2
created: 2026-10-19T11:09:12Z
updated: 2026-10-20T02:03:26Z
depends: []
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
