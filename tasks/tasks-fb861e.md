---
id: tasks-fb861e
title: "tasks feedback's duplicate check misses paraphrased duplicates and ignores ordinary tasks, so most coverage drops start as reports the filer could have attached to an existing task while it still had the context"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T14:27:16Z
created: 2026-10-20T12:44:29Z
updated: 2026-10-20T14:27:16Z
depends: [tasks-a9744c]
parent: tasks-9b1c2b
tags: [feedback, gap, "from:lore"]
agent: claude-code
---

In the coverage-drop audit (attached to tasks-fa3525), 47 drop links pointed to an earlier open-or-recent feedback task in the same project whose title paraphrased the new one (token similarity found no match), and 11 drops had only an ordinary covering task that predated the report, which the check never considers. Proposal: on filing, print the few most similar open tasks in the owner project, feedback or not, by a looser or semantic match, without blocking; the filer then recurs or adds its distinct ask as a note on the right one, or files new. Coverage drops were 72 of 117 drop proposals, and 8 of 71 audited coverage drops lost an ask.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — evidence, distinct asks, alternatives and ownership boundaries

## Notes

- 2026-10-20T14:27:16Z (main): scope: briefed; checked 88c242e: feedback scans open feedback only, title Jaccard threshold 0.6, unique-exact recurrence and ambiguous refusal. Broader or semantic matching is not yet justified; tasks-a9744c compares at most 12 historical cases, ordinary-task candidates, false matches and when suggestions can prevent creation; brief: docs/notes/2026-10-20-feedback-ask-preservation-brief.md
