---
id: tasks-980a39
title: "check skips spec, plan, and step validation on done and dropped records"
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-09-13T15:32:44Z
updated: 2026-09-24T12:26:01Z
started: 2026-09-24T12:26:01Z
depends: []
tags: [feedback, gap, "from:tasks", cli]
---

Why: `check` validates `doc_missing` and `step_missing` on every record regardless of status (`src/commands/check.rs`, the link loop at the end of the per-task pass). When a plan revision merges or removes headings, the step tasks dropped with it fail the gate forever, and the workaround (repointing a dropped task at a surviving heading) falsifies its history. Reported twice: this one (from tasks) and tasks-0f8043 (from mind6, three dropped records).

Decision: closed records (done, dropped) are history, so `check` skips link validation on them entirely. Design spec §7 already scopes the gate to "doc drift under open tasks", and the skill says removing a heading "under an open task" fails check. Rejected: downgrading to a warning, which leaves permanent noise that only link-clearing can silence, and that erases the history this change exists to keep. Shelved records stay checked, since they are open. Writes still validate links as before.

Done when: `doc_missing` and `step_missing` are not reported for done or dropped tasks; a test in `tests/cli.rs` drops a step task, removes its heading, and gets a clean `check`, and another shows an open task still fails. Update `docs/specs/2026-08-29-tasks-design.md` §7 to say the failures apply to open records.

Original report: Merged two plan headings into their neighbours and dropped the two step tasks with tasks drop; tasks check then failed with step_missing for the dropped tasks because their old headings were gone. tasks edit has --plan and --step but no --no-plan/--no-step, so the only way out was to repoint the dropped tasks at the surviving headings. Expected: either check ignores step links on closed tasks, or edit can clear them.

## Notes

- 2026-09-24T12:24:41Z (main): scope: scoped; todo P2 s low direct, check exempts done/dropped from doc_missing/step_missing (rejected: warning); the edit-flag half moved to tasks-136399
- 2026-09-24T12:26:01Z (main): started
  provenance: {"harness_session":"claude-code:14066760-8259-4a7a-ba17-31c6767931cc","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
