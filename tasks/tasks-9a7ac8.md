---
id: tasks-9a7ac8
title: Large evidence manifests require manual multipart attachment and reconstruction
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-25T15:50:04Z
created: 2026-10-24T19:19:16Z
updated: 2026-10-25T15:50:04Z
depends: [tasks-b81063]
parent: tasks-bbfc3c
tags: [feedback, gap, "from:ops"]
agent: codex
---

A frozen evidence manifest exceeded the configured attachment byte cap. Keeping every row and running normal checks requires manually splitting it into bounded JSON parts, writing a catalog with hashes, and verifying lossless reconstruction. A supported multipart attachment/export route would preserve exact evidence identity without raising the cap or hiding text in an archive.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-evidence-handoffs-brief.md — evidence, questions and follow-up boundaries

## Notes

- 2026-10-25T15:50:04Z (main): scope: briefed; retained the single oversized-manifest request; filed bounded lossless multipart/cap investigation tasks-b81063; atomic multi-file attach tasks-b9b2e2 does not cover reconstruction; evidence: main 3813d1d, src/commands/attach.rs, src/record_commit.rs, src/refs.rs and approved attachment/record-commit/reference designs; origin: filing event unavailable (show has no creation note or provenance stamp); no origin_session names an older request; origin remains unknown; brief: docs/notes/2026-10-25-evidence-handoffs-brief.md
  provenance: {"harness_session":"codex:01a173bc-eec8-72f3-afcd-4de019909d0d","harness_session_source":"CODEX_THREAD_ID"}
