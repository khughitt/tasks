---
id: tasks-1675ac
title: Make shelved-dependency guidance preserve intentional waits
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-22T21:18:57Z
updated: 2026-10-23T18:27:17Z
depends: []
parent: tasks-606dd3
tags: [feedback, friction, "from:ops"]
agent: codex
---

Why: check correctly reports that a shelved prerequisite keeps open work blocked, but its advice forces a choice between unshelving and removing the dependency. A documented future-stage wait can intentionally retain both. [ref:brief] establishes the warning path and bounds this to guidance, not new state or suppression.

Original report: A goal deliberately depends on a later stage that is shelved until an earlier prerequisite produces evidence. The shelving note explicitly retains the goal dependency. Validation reports shelved_dep and tells the caller to unshelve or drop the dependency, but either would undo the recorded sequencing decision. Consider distinguishing this intentional future-stage case or giving a disposition route; the observed warning is retained rather than changing another goal.

Done:
- Keep the shelved_dep warning kind, trigger, severity and readiness/closure behavior unchanged. Revise its detail to explain the blocking effect and allow retaining the edge and shelf when the wait is intentional and has a recorded wake condition; offer unshelving or removing the edge when the wait is no longer intended. Do not infer intent by parsing notes or add an acknowledgement/suppression field.
- Update the exact assertion in check_warns_when_open_work_depends_on_shelved_work and cover a deliberately shelved future stage whose dependent goal remains blocked. The warning remains present; a shelved or closed dependent remains excluded under the existing rule.
- Keep README shelf guidance and skills/tasks/SKILL.md consistent: a shelf stays open and blocks dependents; retaining that warning is a valid disposition for an intentional wait, and a note does not suppress it.
- Coordinate with existing tasks-52bd39 before implementation: its pending dep/shelve write-time warnings must use the same disposition if they land first or concurrently. That work is related, not a new child or a duplicate of this text correction.
- Verify with `just test-one --test cli check_warns_when_open_work_depends_on_shelved_work` (and the focused new case), then `just test-fast` and the normal commit checks.

Where to look: [ref:brief]; src/commands/check.rs (shelved_dep); tests/cli.rs; README.md shelf paragraph; skills/tasks/SKILL.md shelf guidance; tasks-52bd39 for pending write-time warning text. Source checked at d0564d1; warning introduced by 4a7bc5b. The originating goal is not linked in the report and was not independently reproduced.

Assessment: P2 retained; s/low/direct because the outcome, existing condition, text-only remedy and regression checks are established.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-dependency-state-brief.md — observed dependency rules and remaining contract decisions

## Notes

- 2026-10-22T23:04:45Z (main): scope: scoped; P2/s/low/direct; d0564d1 confirms shelved_dep warns without reading intent (introduced by 4a7bc5b); retain the warning and add the valid disposition of preserving an intentional wait with a recorded wake condition; no suppression state or readiness change; coordinate pending wording with tasks-52bd39; brief: docs/notes/2026-10-22-dependency-state-brief.md
- 2026-10-23T18:27:17Z (main): proposal from ops scope, with user agreement 2026-10-23: lead the shelved_dep remedy list with 'replace the shelf with a dependency on the task it waits for' (tasks unshelve <dep>, then tasks dep <dep> --on <awaited>). Reserve keeping the shelf and its warning for waits that no task can stand for (an upstream release, say); a warning that is acceptable to ignore and fires on every commit teaches agents to ignore check output. Case: autonomy-bf446b was shelved until autonomy-9e8a43 ran, while goal ops-3f5aa6 depended on it; it was converted to idea + dep edge, and the warning cleared with no loss of sequencing. tasks-52bd39's write-time warnings should use the same wording. Judge this against the reviewed scope before implementing.
