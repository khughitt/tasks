---
id: tasks-e4ccb6
title: Establish the cross-project listing bottleneck and smallest remedy
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-20T02:03:07Z
updated: 2026-10-20T02:03:07Z
depends: []
parent: tasks-96c00c
tags: []
source: tasks-bc5efc
agent: codex
---

Question: What accounts for the reported roughly 3.1 s for `tasks list --all-projects --status todo --status doing --status blocked`, and which smallest change would materially improve that consumer's refresh?
Where to start: [ref:brief], src/commands/list.rs::list, src/commands/mod.rs::ReadCtx::scan_with_claims, src/repo.rs::Project::scan, src/output.rs::TaskSummary::of and src/commands/list.rs::review_deps.
Bound: Establish the target host, binary and dataset; take a small repeatable set of end-to-end timings and isolate scan, dependency/claim, row-construction and serialization costs. Compare one representative local read. Record conditions and measurement overhead; stop once a dominant cost or a clear non-reproduction is established. No cache, persistent index, daemon, new CLI or consumer implementation. Respect any applicable measurement-path gate on tasks-282666.
Expected result: Record timings, an evidence-backed bottleneck or non-reproduction, and the smallest recommended next scope on this task and in the brief; explain how a later change would preserve freshness, warnings, selection and malformed-record errors. Do not choose caching from the report alone.
Ideas it wakes: On completion, run `tasks note tasks-bc5efc --no-reviewed "<measurements and recommended remedy>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — evidence and question this follow-up must resolve
