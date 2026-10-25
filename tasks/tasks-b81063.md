---
id: tasks-b81063
title: Establish a lossless capped workflow for oversized evidence manifests
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-25T15:49:15Z
updated: 2026-10-25T15:49:15Z
depends: []
parent: tasks-bbfc3c
tags: []
source: "tasks-9a7ac8:multipart-investigation"
agent: codex
origin: agent
---

Question: What smallest supported workflow preserves and reconstructs one oversized evidence manifest exactly while retaining per-file caps and normal content checks?

Where to start: [ref:brief], src/commands/attach.rs::attach, src/attachments.rs::read_capped, tests/attachments.rs and docs/specs/2026-09-28-task-attachments-design.md; compare tasks-b9b2e2's already scoped atomic multi-file attach.

Bound: Trace current size, ledger and retry behavior; use one synthetic over-cap JSON manifest in an isolated fixture to compare manual bounded parts plus a hash catalog with a narrow supported multipart attach/export route. Check exact reconstruction, ordering, absent or altered parts, interruption/retry, and how total size remains bounded rather than bypassing policy. Do not raise caps, hide text in archives, add an artifact store or implement a new CLI. Record any unresolved policy decision instead of silently choosing it.

Expected result: Record evidence and a recommendation here and in [ref:brief], with acceptance checks for any proposed support. Distinguish one oversized file from several independently small files; coordinate with tasks-b9b2e2 and file a design only if the recommendation needs it.

Ideas it wakes: On completion, run `tasks note tasks-9a7ac8 --no-reviewed "<finding>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-evidence-handoffs-brief.md — evidence, questions and follow-up boundaries

## Notes

- 2026-10-25T15:49:15Z (add): created
  provenance: {"harness_session":"codex:01a173bc-eec8-72f3-afcd-4de019909d0d","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
