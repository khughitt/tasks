---
id: tasks-a309e0
title: "scope briefs record 'X waits on research Y' in prose and wake notes, never as a depends edge, so ready/tree show the waiting idea as unblocked"
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: main
created: 2026-10-08T20:35:39Z
updated: 2026-10-13T09:58:44Z
started: 2026-10-13T09:58:44Z
depends: []
parent: tasks-5408fe
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: a scope brief records that an idea waits on a research task only in prose and in the wake note, so the wait is invisible in the record; curate later had to add the edge by hand.
Done: skills/scope/SKILL.md §5 (and the briefed row of §3) tells the pass to run `tasks -C <root> dep <idea> --on <research>` for every idea a research or design follow-up names under "Ideas it wakes", in the same pass that files the follow-up; the wake note stays, since the edge does not readmit the idea. Keep the skill wording consistent with tasks-4d7a32's edits to the same file.
Check: read the edited §5 against the brief's example; one scope pass rerun on a briefed idea shows depends_on in `tasks show`.
Where to look: docs/notes/2026-10-13-review-pass-followthrough-brief.md; skills/scope/SKILL.md §3 and §5; skills/curate/SKILL.md (it already repairs edges).
Premise correction (2026-10-13 probe): ready never lists ideas, and pretty tree/list show no dependency marker for any status, so the edge's value is in show's depends_on and in carrying the block when the idea is promoted to todo. Reverse edges in show are tasks-e8d471.

Original report: One curation pass of 5 found three ideas (two projects) whose scope brief named the research task that must land first, with no dependency recorded; curate had to add the edge. Suggest scope's briefed outcome also runs tasks dep <idea> --on <research> for each idea the research wakes.

## Notes

- 2026-10-13T09:20:49Z (main): scope: scoped; todo P2/xs/low/direct under goal tasks-5408fe; body rewritten with done/check and a premise correction (ready never lists ideas); original report kept; brief: docs/notes/2026-10-13-review-pass-followthrough-brief.md
- 2026-10-13T09:58:44Z (main): started
  provenance: {"harness_session":"claude-code:c1a5e1cd-ac8c-454a-8e68-436d7130a52e","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
