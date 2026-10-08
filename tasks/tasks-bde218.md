---
id: tasks-bde218
title: Preserve typed Mindful references when scope reads sources
status: doing
priority: 1
size: s
complexity: low
process: direct
owner: main
created: 2026-10-06T19:05:48Z
updated: 2026-10-08T11:10:03Z
started: 2026-10-08T11:10:02Z
depends: []
parent: tasks-6ed79b
tags: [feedback, gap, "from:material"]
agent: codex
---

Why: scope's source lookup instructions omit the typed-reference rule, so an agent can turn an accessible source into a false unavailable-context finding. Current skills/scope/SKILL.md only says to confirm the CLI form. Quick-add already preserves thought: after stripping the optional mindful: wrapper.

Done: State the normalization explicitly in scope: mindful:thought:<uid> becomes thought:<uid>; an already typed reference remains intact. Require a canonical-reference lookup before calling context unavailable and distinguish a lookup failure from missing or inaccessible source evidence. Keep source strings on task records unchanged. Do not add a parser, new retrieval service or general Mindful workflow.

Where to look: skills/scope/SKILL.md evidence-gathering section; docs/notes/2026-09-13-scope-skill-validation.md; docs/notes/2026-10-02-cross-project-review-brief.md and docs/notes/2026-09-30-work-selection-brief.md; docs/specs/2026-09-12-scope-pass-design.md. The two latter briefs contain bare-UID misses for tasks-c591da and tasks-77dbc6 despite canonical typed sources on those records.

Verification: Exercise a bounded skill scenario where bare UID fails but the canonical typed ref succeeds (including an archived source); the result must use the available source and must not report it unavailable. Check both wrapped and already typed inputs. Re-read the two identified sources and correct only their false unavailable-context statements in the existing briefs, preserving historical commands and distinguishing new evidence from past observations; do not reopen their delivered work or redesign those efforts. Read-only checks on 2026-10-08 confirmed both canonical lookups exit 0 and return the expected thought IDs. Record skill validation evidence without copying private thought content.

Process: direct; normalization, affected guidance and acceptance checks are established. P1 preserves the reporter's explicit requested priority.

Original report and prior context (preserved):
Observed during /scope: task source fields retained canonical mindful:thought:<uid> references. The agent removed both the mindful: wrapper and the thought: type prefix before invoking mindful --json show <uid>; all three lookups exited 1 with no thought matching. Repeating the same lookups as mindful --json show thought:<uid> succeeded for all three, including archived thoughts. This was an agent reference-normalization error, not missing source IDs, an access failure, or evidence of migration loss.

The scope skill says to confirm CLI form and use mindful --json show, but does not state which prefix to preserve. The quick-add guidance already says to strip only the optional mindful: wrapper and retain the full thought: prefix. Make scope source handling equally explicit, and require checking the canonical typed reference before recording a source as unavailable. Acceptance: a task source mindful:thought:<uid> is read as thought:<uid>; a reproducible bare-UID miss cannot be reported as unavailable context when the canonical reference resolves. User requested P1.

## Notes

- 2026-10-08T09:50:35Z (main): scope: scoped; P1/s/low/direct; preserve typed Mindful source references and verify canonical lookup before reporting unavailable context; two local historical misses now resolve by typed reference; retain sources and correct those evidence statements during delivery
- 2026-10-08T11:10:02Z (main): started
  provenance: {"harness_session":"codex:01a11b2e-c42e-7073-8fc5-a8a568d4e759","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T11:10:03Z (main): Scope confirmed: source normalization guidance in skills/scope/SKILL.md, validation evidence, and historical lookup corrections in the two named briefs; reuse quick-add rule, retain task source strings and historical commands, no product parser or retrieval changes. Main work-selection brief has pre-existing uncommitted scope edits; preserve those during integration.
