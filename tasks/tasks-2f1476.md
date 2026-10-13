---
id: tasks-2f1476
title: "edit has no precondition on the record's updated stamp, so curate's revalidate-then-write leaves a window it documents as accepted"
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-08T20:35:39Z
updated: 2026-10-13T10:13:04Z
depends: []
tags: [feedback, idea, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Original report: curate re-reads show, compares task.updated, then edits. An option such as edit --if-updated <stamp> that refuses on mismatch would close the race the skill accepts and drop the separate re-read.

Why: curate (step 4) and scope (§6) revalidate with a second show and then write. Their check is not a lock: a session that starts the task between the re-read and the write gets a curator's prose edit and note on a task it holds, a window both skills document as accepted. Every write already stamps strictly after the stamp it loaded (tasks-cff04e, time::after), so any intervening write to the record, a start included, changes updated. A precondition on that stamp, checked under the mutation lock, therefore detects the race exactly.

Done:
- edit and note take --if-updated <stamp>. After load (stale_copy check, guard_write) and before reading any other input or writing, a loaded task.updated that differs from <stamp> refuses with a new typed error kind (stamp_mismatch) whose detail names both stamps; nothing is written or committed. Without the flag, behavior is unchanged.
- The new error kind is added to the design spec's error table and the JSON contract docs; no other JSON shape changes.
- skills/curate/SKILL.md step 4 and skills/scope/SKILL.md §6 pass the first show's stamp on the first write and chain later writes from the returned record; the status and live-claim checks stay, and the "check, not a lock" caveat is replaced by the refusal's meaning (skip and report). skills/tasks/SKILL.md mentions the flag where it lists edit and note.

Where: src/cli.rs (edit and note args), src/commands/edit.rs, src/commands/status.rs::note, src/commands/mod.rs::load, src/error.rs, docs/specs/2026-08-29-tasks-design.md error table, the three skills above.

Verification: tests/cli.rs cases: a matching stamp writes; a mismatched stamp refuses stamp_mismatch with record bytes and git HEAD unchanged; a start between show and edit makes the edit refuse; stdin --body input is not consumed on refusal. just test-one for each, then just test-fast; reinstall with cargo install --path . after the change.

## Notes

- 2026-10-13T10:13:04Z (main): scope: scoped; strictly increasing per-write stamps (tasks-cff04e) make an updated-stamp precondition exact; scoped --if-updated on edit and note with a stamp_mismatch error and the curate/scope revalidation rewritten to use it; P2/m/mid/direct
