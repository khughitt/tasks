---
id: tasks-236f4c
title: Make record-commit warnings point to named recovery guidance
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-20T11:42:26Z
updated: 2026-10-20T14:27:23Z
depends: []
tags: [feedback, friction, "from:lore"]
agent: claude-code
---

Why: commit_failed and the tasks skill point to unnamed "record-commit routes". Inspection at 88c242e finds the three recovery routes already present under Process and workspace, introduced by f62e840; the report overstates their absence, but the destination is hard to find.

Done: give that existing guidance a stable, descriptive heading and make src/record_commit.rs::failed and the skill's Record commits paragraph name/link that exact destination. Keep the current routes for committed HEAD failures, unrelated drafts and unreachable cross-project references intact; do not change hook bypass or approval policy. Keep the wording check beside existing record-commit warning tests if the emitted warning changes. Review both references against the heading, then run just test-one record_commit and just test-fast for a source change, and the normal content check. No new command or option is needed.

Where to look: skills/tasks/SKILL.md (Process and workspace; Recording work / Record commits), src/record_commit.rs::failed and its existing tests. [ref:brief] records the evidence and why tasks-dd1225 (which fixes the project root in recovery commands) is related but does not cover this destination wording.

Original report (preserved): commit_failed from a refusing hook tells the agent to use 'the record-commit routes in the tasks skill', but the skill has no such section; its only mention points to 'the record-commit routes above', which do not exist

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — evidence, distinct asks, alternatives and ownership boundaries
