---
id: tasks-70a7a7
title: Name the reopen command when start refuses a done or dropped task
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: fix/reopen-hint
created: 2026-10-12T11:48:42Z
updated: 2026-10-14T18:48:57Z
started: 2026-10-14T18:45:55Z
depends: []
tags: [feedback, friction, "from:mind6", cli]
agent: codex
---

Why: reopening a closed task for a correction (a reviewer hands back a step after `tasks done`) needs `tasks edit <id> --status todo` before `tasks start`, because `Status::can_transition` allows a closed status back only to todo. `start` on a done or dropped task fails with the bare `invalid transition done -> doing` and does not say so, while the shelved case already names its fix (`doing (\`tasks unshelve {id}\` first)` in src/commands/status.rs::start).

Done: `tasks start <id>` on a done or dropped task (without `every`) still fails with kind `invalid_transition`, and its message names `tasks edit <id> --status todo` as the step before `start`, in the same `(... first)` form as the shelved hint. A moved task keeps its current refusal (it cannot reopen). Cadence reopening (done -> doing with `every`) is unchanged. RED→GREEN integration tests in tests/cli.rs for done, dropped, and the unchanged moved and cadence cases; `just test-one`, `just test-fast`, `just check`. Mention the reopen path in skills/tasks/SKILL.md near the `edit --status todo` text if it is not already explicit.

Where to look: src/commands/status.rs::start (shelved hint at the top), src/commands/mod.rs::transition, src/model.rs::Status::can_transition, src/error.rs InvalidTransition.

Original report (preserved): A correction handed back after a step completion makes start fail with invalid transition done -> doing, without naming the edit-to-todo command that permits the correction claim.

## Notes

- 2026-10-13T10:48:02Z (main): scope: scoped; start's done/dropped refusal gains the edit --status todo hint, matching the shelved hint in status.rs::start; checked against main d698a36 (tasks 0.2.0); P2/xs/low/direct
- 2026-10-14T18:45:55Z (main): started
  provenance: {"harness_session":"claude-code:2943584c-eb0b-4cf1-b9ce-0209355d5649","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T18:45:57Z (fix/reopen-hint): resumed
  provenance: {"harness_session":"claude-code:2943584c-eb0b-4cf1-b9ce-0209355d5649","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T18:48:57Z (fix/reopen-hint): RED: new cli test saw bare 'invalid transition done -> doing'; GREEN after the status.rs hint. Moved stays refused at load (kind moved, tests/move.rs); cadence reopen asserted in the new test and cli.rs recurrence test. just test-fast and just check exit 0.
