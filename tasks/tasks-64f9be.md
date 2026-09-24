---
id: tasks-64f9be
title: "Quiet-park guidance: estimates name phases and refusal risk; runs end with a run: note"
status: done
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-09-24T10:01:10Z
updated: 2026-09-24T10:25:24Z
started: 2026-09-24T10:23:58Z
completed: 2026-09-24T10:25:24Z
depends: []
tags: [skills]
source: ops-2da76d
model: "claude-opus-5-5[1m]"
agent: claude-code/claude-opus-5-5
---

Why: agents park dedicated benchmark and capture runs with --reason quiet --minutes N, but the estimate says nothing about phases or refusal risk and nothing records the actual. Measured on 23 quiet parks (2026-09-16 to 09-24): runs that went as planned took 2 to 36 times less than estimated; 6 of 14 resumed parks ended in preflight refusals or hangs that no estimate named; half the runs (headless TTY, detached) are visible only in artifacts, so only the agent can report them.

Done when skills/tasks/SKILL.md step 5 (the quiet paragraph) and the README's park section say:
- A quiet estimate is the expected wall-clock of the run once started. The next step names its phases (for example build, run, analysis) and any refusal or hang risk known from earlier attempts.
- Each attempt ends with one note in this form: run: <actual> min (est <n>, <needs>); <phase> <m>[, ...]; <outcome>[: <cause>]. <outcome> is one of passed, failed, refused, hung, aborted. Refused attempts get a note too, timed up to the refusal. The agent that resumes writes it, reading the times from the run's artifacts when the person started it from a TTY or it ran detached.
- Example: run: 69 min (est 40, idle); preflight 1, build 4, smoke 64; hung: tracy-csvexport export

Keep it tool-generic: no project names or paths in the upstream text. The form is the contract obs parses; change it only together with the obs piece.

Where to look: skills/tasks/SKILL.md step 5, README park section, docs/specs/2026-09-13-quiet-queue-design.md. Origin: the ops scoping of ops-2da76d.

## Notes

- 2026-09-24T10:23:58Z (main): started
  provenance: {"harness_session":"claude-code:82bb7a6c-8663-4cef-a2a1-625d17bfb588","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T10:25:24Z (main): done
  provenance: {"harness_session":"claude-code:82bb7a6c-8663-4cef-a2a1-625d17bfb588","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T10:25:24Z (main): Skill step 5 and README: quiet estimates list phases and past refusal or hang risk; every attempt ends with one run: <actual> min (est <n>, <needs>); <phase> <m>, …; <outcome>[: <cause>] note (outcome passed|failed|refused|hung|aborted); quiet queue spec §6 notes the addition
  provenance: {"harness_session":"claude-code:82bb7a6c-8663-4cef-a2a1-625d17bfb588","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
