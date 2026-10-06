---
id: tasks-0e432f
title: Record authority and recovery
status: todo
priority: 3
size: l
complexity: high
process: direct
lane: true
created: 2026-10-06T16:49:54Z
updated: 2026-10-06T16:54:21Z
depends: []
tags: []
source: "user:2026-10-06 tasks lane organization"
agent: codex
---

Make record readers, audit writers, and recovery guidance agree about authority without losing history or concurrent work. First milestone: tasks-e068a2 fixes historical-plan reads in the shared describe path. Finish registered-record publication and stale-copy guidance from the existing probes in parallel, including the later Git branch merge; resolve remaining closed-park questions separately.

Preserve tasks-984769 and tasks-0e7216 as child goals with their current children and acceptance checks. Also group tasks-e068a2's historical-plan reads and tasks-9de6aa's worktree warning here. Coordinate the halt-audit design with the same authority and write-ownership rules, while keeping its audit ordering and incident visibility intact.

Dependencies sequence real inputs; ordinary sibling fixes can proceed independently. Preserve registered dependency authority, stale-copy refusal, active ownership, notes, and closed-history readability. Review status remains on each existing artifact and task; lane adoption grants no design or plan approval. Coordinate changes to src/commands/mod.rs, claims, and resolution code with the location and agent lanes. Done when every retained child has completed or received an evidence-backed disposition.

## Notes

- 2026-10-06T16:49:59Z (main): organization: created from the user's approved 2026-10-06 lane proposal; coordination uses existing tasks and preserves nested goals and child review requirements.
