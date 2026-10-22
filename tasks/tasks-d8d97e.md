---
id: tasks-d8d97e
title: "After the harness resumes a conversation under a new native session id, that conversation's own live claims read as another session's: park refuses and only start --force with a reason recovers them"
status: idea
priority: 2
created: 2026-10-21T18:17:58Z
updated: 2026-10-22T23:11:06Z
depends: [tasks-ea0f60]
parent: tasks-2fa8c6
tags: [feedback, friction, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Claude Code resumed a long session with a new CLAUDE_CODE_SESSION_ID while the old process was still alive; claims recorded under the earlier id blocked park with 'claimed by session …' until start --force took them over.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-session-continuity-brief.md — evidence, constraints and open questions
