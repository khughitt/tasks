---
id: tasks-d4e73d
title: Determine whether completion follow-ups need native task hooks
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-08T10:58:20Z
updated: 2026-10-08T10:58:20Z
depends: []
parent: tasks-46d207
tags: []
source: tasks-d40e8e
agent: codex
---

Question: Does the captured completion-follow-up workflow require a native tasks lifecycle hook, or can an explicit agent closeout step and separate Git integration cover it?
Where to start: docs/notes/2026-10-08-completion-followups-brief.md; tasks-d40e8e; src/commands/status.rs::close, src/commands/mod.rs::transition/save, src/commands/edit.rs; docs/specs/2026-09-09-periodic-design.md section 2.3; README lifecycle-note contract; existing close/retry tests in tests/cli.rs; repository agent closeout guidance and .githooks/. The original Mindful reference returned no matching thought, so use the preserved task capture.
Bound: trace successful completion, failed save, already-done retry, a recurring occurrence and edit-to-done. Identify the durable boundary and whether a follow-up prompt would duplicate or misreport work. Compare explicit closeout guidance, an external wrapper/Git event, and a native post-save hook for the one motivating question: are follow-up tasks needed? A merge is a separate event and may occur outside tasks. No hook framework, new config, process execution, live Git-hook installation, host changes or implementation. Do not reopen the settled recurrence design.
Expected result: record a small event/coverage table and recommend the smallest sufficient home for this workflow. Name any concrete unmet event/delivery requirement. If native hooks remain necessary, enumerate payload, failure, retry, reentrancy, trust and mutation-authorization decisions for a later planned design; otherwise identify the narrow guidance/integration follow-up. Update the brief and this task with observed evidence. P2/s/mid/direct reflects the bounded trace, not the complexity of implementing hooks.
Ideas it wakes: on completion, run tasks note on tasks-d40e8e with the finding in the same commit as the result and brief update.
