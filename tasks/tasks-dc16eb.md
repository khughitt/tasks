---
id: tasks-dc16eb
title: Decide whether task records and design docs leave project repositories for a per-user records root
status: todo
priority: 1
size: m
complexity: high
process: planned
created: 2026-10-19T01:24:43Z
updated: 2026-10-19T11:40:55Z
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

## Notes

- 2026-10-19T01:44:37Z (main): Scope widened by the user 2026-10-19: every working document leaves public repositories, not only task records: plans, briefs, handoffs, and specs by default. A spec worth keeping as reference is committed consciously, possibly in a minimal form. Everything moved stays findable through a CLI (list, search, and possibly create and edit docs, e.g. 'tasks doc search' or an ops/lore command). Applies to ops and its sibling projects; the verifiably family keeps its own conventions unless it opts in.
- 2026-10-19T11:40:55Z (main): From the boundary survey review (ops-630a87, revision 2, 2026-10-19), this design also settles: (1) one home for working docs, the records store or mindful; if the records store, rule on doc_fetch.rs and [docs.schemes] explicitly, reconcile ops-721511 and hq-5a3e12, and re-inventory stored external references (none on 2026-10-19) before removing code; (2) flows' fingerprint covers all retained repository content, reference: true permits presence only; (3) catalogue freshness as current/behind/unknown, since stale.rs cannot see another host's sync; (4) ops keeps its prefix as the records-only hub (rename reserves old prefixes), the public kit gets a fresh prefix; (5) the moves respect flow-trial-1 through readout 2027-02-16.
