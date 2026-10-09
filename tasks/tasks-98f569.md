---
id: tasks-98f569
title: graph --format is a hand-parsed value set with no completion
status: idea
priority: 3
created: 2026-09-05T23:00:36Z
updated: 2026-10-08T10:16:45Z
depends: []
tags: [cli]
spec: docs/specs/2026-09-05-shell-completions-design.md
---

graph --format accepts mermaid|dot, validated in query.rs, but has no ArgValueCandidates. The completions spec's Problem section names exactly this class of argument -- hand-parsed String value sets where a typo is a runtime error -- but its scope table omits graph --format and its Non-goals does not mention it, so this is a gap in the spec rather than a deliberate exclusion. Trivial to add alongside sorts()/colors(). Found by the final review of the completions branch, 2026-09-05.

## Notes

- 2026-10-08T10:16:45Z (main): scope: drop; delivered in 59246c3: graph --format wires ArgValueCandidates to complete::graph_formats using GraphFormat::ALL, with ValueSet parse-time validation; current installed CLI probes returned mermaid and dot in both Bash and Zsh; original report and spec link retained; proposal: drop as already delivered by 59246c3
