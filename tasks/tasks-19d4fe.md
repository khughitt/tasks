---
id: tasks-19d4fe
title: "The scope skill has no verdict for a misfiled report whose remedy belongs to another project: it can only propose a drop, so the record stays open and a refile is left to a later turn. Proposal: add a refile verdict that runs 'tasks move --to <owner>' (nothing is lost, so no approval needed) or files the remedy in the owner project and proposes the drop; keep drops of landed or duplicate work as proposals."
status: idea
priority: 2
size: s
complexity: mid
process: direct
reviewed: true
created: 2026-10-12T10:50:38Z
updated: 2026-10-13T09:55:24Z
depends: []
parent: tasks-5408fe
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: a scope pass that finds a report whose remedy belongs to another project can only propose a drop, so the record stays open in the wrong tracker and the refile waits for a later turn.
Ruling (user, 2026-10-13): a scope pass may move a record into the owning project without asking first.
Done: skills/scope/SKILL.md gains a `refile` verdict. When the whole remedy belongs to one other registered project whose `[feedback] scope` (in its tasks/.config.toml) covers it, the pass runs `tasks -C <root> move --explain --to <owner> <id>`, then the move, and records the new id in its summary. When only part of the remedy belongs elsewhere, it files that part in the owner project (`tasks add --project <owner>`, keeping the source) and treats the rest by the existing verdicts, proposing a drop when nothing remains. Drops of landed or duplicate work stay proposals. Say how the audit note is kept: the moved record carries it, written before the move, as a plain note since move takes it out of `idea` here. State that the move is the one write a pass makes outside its fixed root. Update the summary format (a refiles group: old id → new id) and keep the curate skill unchanged unless it already promises the same thing.
Check: read the verdict table and §6 against `tasks move --help` and one `--explain` run on a real misfiled idea; tasks-91c622's cross-project contract does not contradict it.
Where to look: docs/notes/2026-10-13-review-pass-followthrough-brief.md; skills/scope/SKILL.md §3, §6, §7; src/commands/move_.rs; tasks-91c622. Coordinate with tasks-4d7a32, which edits the same skill.

Original report: The scope skill has no verdict for a misfiled report whose remedy belongs to another project: it can only propose a drop, so the record stays open and a refile is left to a later turn. Proposal: add a refile verdict that runs 'tasks move --to <owner>' (nothing is lost, so no approval needed) or files the remedy in the owner project and proposes the drop; keep drops of landed or duplicate work as proposals.

## Notes

- 2026-10-13T09:21:13Z (main): scope: briefed; parented under goal tasks-5408fe; brief frames a refile verdict via tasks move --to <owner> gated on the owner's [feedback] scope (lean) against a refile proposal under the fixed root, and needs the user's ruling consistent with tasks-91c622's cross-project contract; brief: docs/notes/2026-10-13-review-pass-followthrough-brief.md
