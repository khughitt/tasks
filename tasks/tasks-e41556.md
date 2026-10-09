---
id: tasks-e41556
title: "check judges undeclared needs on live records only, not closed history"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-03T16:18:26Z
updated: 2026-10-09T11:17:27Z
depends: []
parent: tasks-46d207
tags: []
source: docs/specs/2026-10-03-lanes-needs-groups-design.md
agent: claude-code
---

Why: `check` errors when any record, closed ones included, names a need its project no longer declares (lanes/needs design §4.2 'On check'). Retiring or renaming a need then forces edits to finished records purely to satisfy the lint, although nothing reads needs on closed work (holds, --without and the lanes view only consider work that can still run), and the tag check already treats closed records as history.

Done when: `check` reports `unknown_need` only for live records: open tasks plus `done` recurrences (`every` set), which come back as live work. Closed non-recurring and dropped records are not judged. Reads stay lenient, and add/edit keep refusing undeclared names they add. Amend spec §4.2 ('On check') and the needs paragraph in skills/tasks/SKILL.md, and update the test that expects an error on a done record (it should now pass silently, with a done recurrence still erroring).

Where: src/commands/check.rs (the unknown_need loop); tests/cli.rs needs_check_*; docs/specs/2026-10-03-lanes-needs-groups-design.md §4.2; skills/tasks/SKILL.md.

## Notes

- 2026-10-09T11:17:27Z (main): Task-move review D12: include immutable moved tombstones in the historical-needs regression. A moved record can retain every, but remains terminal history; only done recurrences are live. Removing a source need vocabulary entry must not force an impossible edit of the tombstone.
