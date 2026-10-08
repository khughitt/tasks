---
id: tasks-bde218
title: Preserve typed Mindful references when scope reads sources
status: done
priority: 1
size: s
complexity: low
process: direct
owner: fix/tasks-bde218-typed-sources
created: 2026-10-06T19:05:48Z
updated: 2026-10-08T14:07:40Z
started: 2026-10-08T11:10:02Z
completed: 2026-10-08T14:07:39Z
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
- 2026-10-08T11:10:15Z (fix/tasks-bde218-typed-sources): resumed
  provenance: {"harness_session":"codex:01a11b2e-c42e-7073-8fc5-a8a568d4e759","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T11:13:22Z (fix/tasks-bde218-typed-sources): RED reproduction: both historical bare-UID commands exit 1 no thought matching; canonical thought:<uid> commands exit 0 with archived sources using the current main build by explicit path. PATH mindful instead fails read-model v3/expected v2; classified as tool lookup failure, not missing context, and reported ops-07a4ee. No index rebuild, data mutation or launcher changes.
- 2026-10-08T11:16:23Z (fix/tasks-bde218-typed-sources): Skill validation GREEN: fresh agent applied amended guidance to four wrapped/already-typed cases; all returned expected canonical ids and available archived content. Agent verified complete source-record hashes unchanged and classified captured tool-version failure separately. Independent assertions passed after correcting a test-only assumption about internal uid vs canonical id; validation note preserves that detail.
- 2026-10-08T11:19:05Z (fix/tasks-bde218-typed-sources): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6
- 2026-10-08T11:19:05Z (fix/tasks-bde218-typed-sources): Review disposition: earlier Prism unknown is outside this two-source correction and remains historical; no runtime/test/dependency change. Reviewer independently checked both test logs and diff; fresh-agent trial outputs reviewed from harness reports rather than persistent transcripts. Main draft preservation remains the integration check.
- 2026-10-08T11:19:05Z (fix/tasks-bde218-typed-sources): parked (waiting on user, decision): User: authorize temporary stash of only docs/notes/2026-09-30-work-selection-brief.md. Agent: preserve the existing draft, close the task in the reviewed code commit, merge fix/tasks-bde218-typed-sources, restore the draft with the dated source correction while retaining its unrelated edits, and verify the merged result.
  provenance: {"harness_session":"codex:01a11b2e-c42e-7073-8fc5-a8a568d4e759","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T14:07:11Z (fix/tasks-bde218-typed-sources): resumed
  provenance: {"harness_session":"codex:01a11b2e-c42e-7073-8fc5-a8a568d4e759","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T14:07:39Z (fix/tasks-bde218-typed-sources): User approved 2026-10-08: temporarily stash only the overlapping work-selection brief, merge the reviewed source fix, and restore the pre-existing draft with the dated correction. Preserved original draft and hashes of all 27 existing changed files before integration; main advanced independently with only tools/tt v5.
- 2026-10-08T14:07:39Z (fix/tasks-bde218-typed-sources): done
  provenance: {"harness_session":"codex:01a11b2e-c42e-7073-8fc5-a8a568d4e759","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T14:07:39Z (fix/tasks-bde218-typed-sources): Scope preserves typed Mindful references and distinguishes lookup failures from unavailable evidence; corrected two historical brief claims, aligned scope spec and recorded archived wrapped/already-typed validation; independent review accepted, all 1159 fast tests and repository checks passed.
  provenance: {"harness_session":"codex:01a11b2e-c42e-7073-8fc5-a8a568d4e759","harness_session_source":"CODEX_THREAD_ID"}
