---
id: tasks-479a6f
title: Show quiet-host waits in ordinary pretty task tables
status: done
priority: 2
size: s
complexity: low
process: direct
owner: feat/quiet-marker
created: 2026-09-23T17:07:01Z
updated: 2026-09-30T14:38:35Z
started: 2026-09-30T14:36:22Z
completed: 2026-09-30T14:38:35Z
depends: []
parent: tasks-46d207
tags: [feedback, idea, "from:material"]
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Why: ordinary pretty rows hide the quiet-host wait even though TaskSummary already embeds park.reason, needs, and minutes.

Done: src/output.rs::table (the shared pretty task table) appends a textual marker for quiet parks, reusing claims::describe_stop (for example, "waits on user, quiet; idle, 40 min") and the existing wrapped suffix rendering. It appears anywhere that shared table renders an eligible row, including plain list and prime; ready continues to omit user-waiting parks. Preserve JSON and picker behavior. Existing list --parked and quiet already show the recipe. Keep non-quiet and unparked rows unchanged. The optional park-command discovery hint is deferred; the existing README and skill already name tasks quiet.

Where to look: src/output.rs::TaskSummary and table (its defer, due, and claim-owner row markers are the pattern to follow), src/claims.rs::describe_stop, docs/specs/2026-09-13-quiet-queue-design.md section 4, and tests/cli.rs::a_quiet_park_records_its_recipe_in_the_entry_the_note_and_every_park_view.

Verification: extend focused coverage for idle and headless parks in list/prime pretty output, an agent-waiting quiet park that remains eligible for ready, and removal of the marker after re-park or resume. Assert existing JSON fields and user-waiting omission remain unchanged; cover redirected/narrow output through the existing renderer checks. Run just test-one for the focused case and just test-fast before committing.

Original report:
Parked-quiet work (park --reason quiet, with --needs and --minutes) shows up only in 'tasks quiet' and 'list --parked'. Plain 'tasks list --pretty' shows it like any other doing task. Suggest a column or a coloured tag in list/ready/prime pretty output, e.g. 'quiet idle 40m' or 'quiet headless 50m', so it stands out in the everyday views. Also consider having 'park --reason quiet' print a hint that 'tasks quiet' (across all projects) is the end-of-day queue, since neither the person nor the agent that parked it may know the command exists.

## Notes

- 2026-09-30T10:05:33Z (main): scope: scoped; existing park fields and shared table formatter support a display-only fix; priority 2, small/low/direct; optional discovery hint deferred; brief: docs/notes/2026-09-30-work-selection-brief.md
- 2026-09-30T10:23:47Z (main): scope: corrected the table function name: task_table does not exist; the shared pretty table is src/output.rs::table
- 2026-09-30T14:36:22Z (main): started
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:36:24Z (feat/quiet-marker): resumed
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:38:35Z (feat/quiet-marker): done
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:38:35Z (feat/quiet-marker): The shared pretty task table marks a quiet park with its recipe (e.g. 'waits on user, quiet; idle, 40 min') via claims::describe_stop, so list, prime and ready show it; JSON and picker behaviour unchanged; README notes it; integration test covers idle/headless, agent-waiting eligibility, and removal on re-park and resume, plus a wrap/paint unit test.
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
