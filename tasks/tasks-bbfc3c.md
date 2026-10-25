---
id: tasks-bbfc3c
title: Make evidence handoffs lossless and portable
status: todo
priority: 2
size: m
complexity: high
process: direct
created: 2026-10-25T15:48:45Z
updated: 2026-10-25T15:48:45Z
depends: []
parent: tasks-0e432f
tags: []
source: docs/notes/2026-10-25-evidence-handoffs-brief.md
agent: codex
origin: agent
---

Keep task evidence usable across size limits, concurrent record commits and project boundaries. First milestone: settle the multipart transport and commit-isolation questions, while reusing the existing cross-project reference design.

Why: [ref:brief] connects three evidence-handoff reports and distinguishes their unresolved contracts from already scoped multi-file attach.

Done: each member receives the evidence-backed outcome of its named follow-up; any resulting design and implementation receives its own required reviews. Preserve source reports, attachment caps and safety, hook-derived files and staged work. No new artifact store or lane.

Where to look: [ref:brief], src/commands/attach.rs, src/attachments.rs, src/record_commit.rs, src/refs.rs; coordinate tasks-b9b2e2 and tasks-d09e87 without duplicating them.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-evidence-handoffs-brief.md — evidence, questions and follow-up boundaries

## Notes

- 2026-10-25T15:48:45Z (add): created
  provenance: {"harness_session":"codex:01a173bc-eec8-72f3-afcd-4de019909d0d","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
