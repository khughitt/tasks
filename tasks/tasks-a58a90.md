---
id: tasks-a58a90
title: Design feedback occurrence provenance from the brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-20T14:40:20Z
updated: 2026-10-20T14:40:20Z
depends: []
parent: tasks-9b1c2b
tags: []
source: "tasks-516ab4:provenance-design"
agent: claude-code/claude-opus-5-5
---

Why: [ref:brief] establishes that feedback creation and recurrence are unstamped and that a scope pass reconstructs a report's origin by hand; occurrence records, origin distinctions and retrieval need one design. Split from tasks-441019 on 2026-10-20 so the drop-step fix, which the audit shows carries most of the losses, does not wait on retrieval work.

Done: produce a user-reviewed design and implementation plan that (1) stamps each creation and recurrence separately while distinguishing reporting session, older origin session and inferred call anchors, preserving a short tool-only summary; (2) visibly marks, at filing, a report that relays the user's own words, coordinating lore-c01bc8's capture guidance; (3) defines scope retrieval of request/action/result/later correction and checks live, archived, subagent-parent and other-host retrieval or explicit unavailable limitations, coordinating hq-c411df rather than duplicating its reader work; (4) retains the blind at-filing pilot: a fresh agent restricted to the reporting session, against records as they stood at filing, recovers each distinct ask and names the task that covers it. obs-c5c599 owns cross-tool tracing. tasks-441019 owns the drop step and the post-change audit and does not wait on this design; it judges user relay from report text until the marker in (2) exists. Do not assume session stamps alone solve drop-time losses.

Where to look: [ref:brief]; tasks-516ab4 notes (retrieval workarounds from the audit: 9 origins found only through the CLI's created output, 6 Codex subagents through their parent thread, 1 unrecoverable); src/commands/feedback.rs, src/commands/mod.rs::append_stamped_note, src/provenance.rs; skills/scope/SKILL.md's evidence step; the owner records named above. No implementation before written design and plan reviews.

Ideas it wakes: On completion, run `tasks note tasks-516ab4 --no-reviewed "<finding>"` before this result branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — evidence, distinct asks, alternatives and ownership boundaries
