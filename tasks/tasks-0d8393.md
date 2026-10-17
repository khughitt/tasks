---
id: tasks-0d8393
title: "check warns moved_dep on a tombstone whose own depends names other tombstones; a tombstone refuses every write, so the warning can never be cleared and its swap command is refused with moved"
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: fix/tombstone-moved-dep
created: 2026-10-17T02:23:40Z
updated: 2026-10-17T08:50:28Z
started: 2026-10-17T08:47:34Z
completed: 2026-10-17T08:50:28Z
depends: []
tags: [feedback, friction, "from:ops", move]
agent: claude-code/claude-opus-5-5
---

Original report (from:ops): seen when a chain of mutually dependent tasks is moved together. Each source tombstone keeps its frozen depends on the other source ids, and check reports moved_dep on each tombstone, though every counterpart already depends on the live ids. dep on the tombstone refuses (moved) and redirects to a dep on the counterpart that would change nothing. Expected: no moved_dep for a tombstone dependent at all (its depends is history; the counterpart carries the live edges), not only dropping the command as tasks-183bfa M5 proposes. Leaves four permanent warnings in the source project's check on every host.

Why: the move design already settles this. docs/specs/2026-10-08-task-move-design.md (Dependency evaluation and Cycle detection) says a tombstone's "only edge is to its moved_to record; its own retained depends are history, not edges", and every write to a tombstone refuses with `moved`. src/commands/check.rs (the depends loop, ~line 291) still runs the moved_dep test for every task, tombstones included, so the warning and its swap command can never be acted on.

Done: check skips moved_dep when the dependent itself has status moved. A live dependent of a tombstone still warns with the swap, as today. Amend the design's `check` bullet ("warns moved_dep on every record that depends on a tombstone") to say "every live record". This also covers tasks-183bfa's M5 ("omit impossible dep repair commands for immutable tombstones"): note that on tasks-183bfa when this lands. Out of scope: retired_prefix on a tombstone's depends. If it proves equally unclearable, file it separately.

Check: a test in tests/move.rs beside check_warns_moved_dep_with_the_swap (~line 680). Move two tasks that depend on each other, then confirm check in the source reports no moved_dep on either tombstone while a third live task depending on a tombstone still gets the swap. Then `just test-fast`.

## Notes

- 2026-10-17T02:57:33Z (main): scope: scoped; todo P2 xs/low/direct; move design already treats a tombstone's depends as history and refuses its writes, so check's moved_dep on a tombstone dependent (check.rs depends loop, at 721c87a) contradicts it; gate on dependent status != moved; supersedes tasks-183bfa M5
- 2026-10-17T08:47:34Z (main): started
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T08:47:35Z (fix/tombstone-moved-dep): resumed
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T08:50:28Z (fix/tombstone-moved-dep): done
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T08:50:28Z (fix/tombstone-moved-dep): check skips moved_dep when the dependent is itself a tombstone; live dependents keep the swap warning; move design's check bullet amended; test check_skips_moved_dep_on_a_tombstone_dependent in tests/move.rs; noted on tasks-183bfa (settles M5)
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
