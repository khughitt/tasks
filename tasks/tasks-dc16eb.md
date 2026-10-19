---
id: tasks-dc16eb
title: Decide whether task records and design docs leave project repositories for a per-user records root
status: todo
priority: 1
size: m
complexity: high
process: planned
created: 2026-10-19T01:24:43Z
updated: 2026-10-19T01:24:43Z
depends: []
tags: []
source: "user:2026-10-19 ops publication"
agent: claude-code/claude-opus-5-5
---

Why: task records in code repositories add clutter (2026-10-19: v6 957 records, ops 757, tasks 616, beliefs 601, niri-material 473), distract from the code, and are the likeliest place for personal details to leak (machine layout, usernames, host stats, private project names). The user now questions keeping them in GitHub repositories at all, for ops and every other tasks project. This revisits hq-d7b5b2's 2026-10-02 answer (only briefs, handoffs and plans leave the repository; task records and specs stay; its brief lore:docs/notes/2026-10-02-agent-working-docs-brief.md, alternative 3). It blocks publishing ops as a clean repository (ops-741c5e).

Alternatives to settle in a spec:
1. Linked projects for every project: records (and specs and plans) in one private per-user records repository, one directory per prefix, code checkouts holding none. The mechanism already exists and runs for the work repositories (tasks-9b0a2e, ops docs/specs/2026-10-01-linked-checkouts-design.md); backup and history come from that repository and its private remote. Lean.
2. Gitignore tasks/ in every project: no tool change, but records lose history, git worktrees do not receive ignored files, and backup rests on Dropbox alone.
3. Status quo with pruning.

Questions: what replaces flow's 'the record lands with the code' rule (flow skill, Where the record lives) and tasks' stale_copy model when records no longer branch with code; the open linked-project gaps (tasks-6360ac overlay doc roots, ops-ba81cf guide roots, ops-c3a7e0, obs-29a062, obs-ba704d); obs's reads of review and concerns notes; flow-trial-1 runs to 2027-01-10 and a move changes its measured environment; the migration of existing records and their history; whether specs follow the records.

Done: a reviewed spec choosing one alternative, with the migration order and the follow-up pieces filed per affected project (tasks, flows, obs, ops, lore).
