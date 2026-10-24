---
id: tasks-4ca7bd
title: "tasks prime's default JSON runs to ~250 KB on a project with ~230 open tasks (every ready row in full), past an agent tool's output limit, so the session-start read lands in an overflow file; a capped or summary default would fit one read"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T02:03:30Z
created: 2026-10-19T11:09:12Z
updated: 2026-10-24T11:41:08Z
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
- 2026-10-22T22:29:54Z (feedback): feedback from tasks: Session-start prime JSON exceeds the tool response budget
- 2026-10-22T22:29:54Z (feedback): detail from tasks: The default prime response exceeded a 6,000-token command output limit and was truncated at roughly 30,600 tokens; a second read had to select summary fields to recover session context. A bounded session-start summary remains needed; this recurrence adds the observed response size and workaround.
- 2026-10-22T23:00:45Z (feedback): feedback from tasks: tasks prime default JSON exceeds an agent tool output limit
- 2026-10-22T23:00:45Z (feedback): detail from tasks: A scope session ran tasks prime and received truncated output at roughly 30,000 tokens. Reading the session summary required a second parsed view with an explicit key allowlist. A bounded default would let the session-start read fit one tool response.
- 2026-10-22T23:09:02Z (feedback): feedback from tasks: tasks prime output exceeds an agent tool response budget before the session summary can be read
- 2026-10-22T23:09:02Z (feedback): detail from tasks: The default prime JSON was truncated at a 6000-token response budget during session startup. A bounded summary would let an agent read active work and blockers without manually projecting the full payload.
- 2026-10-22T23:20:03Z (feedback): feedback from tasks: Session-start prime output exceeds the tool response limit
- 2026-10-22T23:20:03Z (feedback): detail from tasks: A session-start prime read was truncated at roughly 31,000 tokens; removing ready still left roughly 33,000 tokens because roadmap dominates the remaining output. Extracting selected fields locally recovered a useful summary. This recurrence supports the existing bounded-summary design task; no JSON contract change was made.
- 2026-10-23T10:57:18Z (feedback): feedback from tasks: The default prime JSON exceeds an agent tool output budget
- 2026-10-23T10:57:18Z (feedback): detail from tasks: A session-start prime read produced about 30,410 tokens and was truncated at 6,500 tokens. A second read filtered through jq recovered the session summary. A bounded default should preserve the session-selection signals within one tool read.
- 2026-10-23T11:10:24Z (feedback): feedback from tasks: Session-start JSON exceeds the tool output budget
- 2026-10-23T11:10:24Z (feedback): detail from tasks: During next-work selection, the default prime response exceeded a 10,000-token output budget and was truncated. Reading its session state required a second invocation with a consumer-side filter; a bounded summary with an explicit complete-data route would make the initial read usable.
- 2026-10-24T11:41:08Z (feedback): feedback from tasks: prime default JSON exceeds the agent output limit during session startup
- 2026-10-24T11:41:08Z (feedback): detail from tasks: The required session-start prime read was truncated by the tool output limit again; a second read filtered through a JSON parser was needed to inspect lanes and blockers. A bounded session summary with an explicit full-data route would avoid the repeat read.
