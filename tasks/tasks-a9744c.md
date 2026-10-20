---
id: tasks-a9744c
title: Establish whether a candidate list at filing finds paraphrased existing work
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-20T14:26:02Z
updated: 2026-10-20T14:40:47Z
depends: []
parent: tasks-9b1c2b
tags: []
source: "tasks-fb861e:candidate-evaluation"
agent: codex
---

Question: Can a small local candidate list across ordinary and feedback tasks recover paraphrased existing work at filing time without excessive unrelated matches, and at what point can the filer act on it?

Where to start: [ref:brief]; tasks-fb861e; src/commands/feedback.rs and src/similarity.rs; docs/specs/2026-09-03-feedback-design.md; the tasks skill's pre-filing duplicate check (`tasks search "<phrase>" --all-projects`); audit labels and method attached to tasks-fa3525.

Bound: Evaluate at most 12 fixed cases from records as they stood at filing: paraphrased feedback, ordinary covering tasks, known related-but-distinct asks and no-match controls. Compare the current matcher, the documented `tasks search` check (a literal, case-insensitive substring over open titles and bodies, ordinary tasks included), broader local lexical ranking and agent-assisted search; record top-3 retrieval, false suggestions and cost. Note whether the global feedback rule's "one command, then carry on" leaves the filer room for a search first; lore owns that rule. Do not implement a production matcher, add an embedding service, or repeat the full audit. If historical evidence cannot be reconstructed, report that limitation rather than use later corrections as filing-time evidence.

Expected result: Record cases, outcomes and the smallest supported recommendation on this task and in [ref:brief]. Explain whether suggestions precede creation or arrive after it, the safe note-versus-recur route for ordinary tasks, and preservation of unique-exact/ambiguous/--new behavior. Name any remaining design decision instead of silently broadening the CLI or JSON contract.

Ideas it wakes: On completion, run `tasks note tasks-fb861e --no-reviewed "<finding>"` before this result branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — evidence, distinct asks, alternatives and ownership boundaries
