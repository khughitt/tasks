---
id: tasks-2e7d03
title: "tasks add records no creating-session provenance and no requester/origin marker, so an agent picking up a task cannot find the session or ask that led to it"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-25T16:48:10Z
created: 2026-10-25T11:03:09Z
updated: 2026-10-25T16:48:10Z
depends: [tasks-a58a90]
tags: [feedback, gap, "from:obs"]
agent: claude-code/claude-opus-5-5
origin: mixed
---

Action: tracing which session created a task, and whether the user asked for it, to direct design questions correctly. Observed: add stamps only agent (harness/model) and an opaque source; no harness_session as start/park/done carry, no creation note, commit trailers absent, and the creating session may leave the record for another session's sweep commit, so commit time misleads. Workaround: grep session stores for the id and pick the rollout whose first hit matches created, then read the preceding human turn (about a minute by hand). Expected: add stamps the creating harness_session like lifecycle notes do, and optionally records whether the user's own prompt asked for the task versus an agent-originated filing (follow-up, feedback, decomposition, scope pass), generalizing the user-relayed marker already being designed for feedback.

Scope evidence (2026-10-25, 79b8253): the creation/session gap is confirmed in src/commands/add.rs and commands::create; native provenance already exists for stamped notes. The original report above is retained. [ref:brief] carries the ordinary-add requirements alongside the existing feedback design, including the distinction between filer identity and user intent. Reuse tasks-a58a90 to settle the shared representation and optional requester/origin vocabulary before implementation; its existing feedback requirements remain intact. This idea waits on that design, not on a duplicate research task.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — shared creation provenance and requester/origin questions

## Notes

- 2026-10-25T11:13:25Z (main): scope: briefed; at 79b8253 confirmed add and feedback share an unstamped constructor and native stamped-note provenance already exists; extended existing tasks-a58a90 with ordinary-add session provenance and explicit requester/origin distinctions, retained all feedback requirements and original report, and recorded its dependency and wake note; storage, optional origin vocabulary and correction semantics require design review; brief: docs/notes/2026-10-20-feedback-ask-preservation-brief.md (8350622); no duplicate design or research task
- 2026-10-25T12:27:25Z (design/tasks-a58a90): tasks-a58a90 finished its design and plan (same spec/plan as tasks-516ab4's note): add writes a stamped 'created' note naming the filing session (absent-but-explicit when unknown), add/feedback take --origin user|agent|mixed (user = the user's prompt led to the asks, not who ran the command) and --origin-session for an older ask, edit corrects origin, and the tasks/scope skills say how to follow a record to its originating ask. Implementation is goal tasks-053e89.
- 2026-10-25T13:43:36Z (tasks-053e89): Landed on tasks-053e89 (spec docs/specs/2026-10-25-creation-occurrence-provenance-design.md): tasks add writes a stamped created note naming the filing session, takes --origin user|agent|mixed (correctable with edit --origin/--no-origin) and --origin-session, and the tasks and scope skills give the retrieval procedure from a record back to its originating ask.
- 2026-10-25T16:48:10Z (main): scope: drop; verified at 31e12f6 that merge 7c3b9d2 / tasks-053e89 delivers both asks: add::run writes the stamped created occurrence and origin/origin-session fields preserve requester evidence; edit corrects origin without rewriting occurrence history. README and skills provide retrieval, and tests/cli.rs covers known/unknown/conflicting identity, origin capture, correction and reuse. No speculative backfill: historical records can still lack a stamp; the broader blind pilot remains tasks-e407a1. origin: filing matched via the historical session pointer on tasks-a58a90 (this record has no creation stamp): claude-code:572ee51e-2f53-4729-96f8-f122a2cc8305, call toolu_01Ek5JnaVhp4HaqVGTPKY4je at 11:03:07.986Z, transcript lines 212/218, unique matching command and full created result naming this id. Human turn line 130 asks whether tasks distinguish user-originated and agent-originated asks; the agent adds the concrete creation-session stamping request, hence mixed. Later human turns 229/350 concern the separate guidance work and do not retract these asks; no older origin_session is named. Existing brief unchanged because this member needs no further design. proposal: drop tasks-2e7d03 as delivered by tasks-053e89 in 7c3b9d2; no drop executed
  provenance: {"harness_session":"codex:01a173f4-2451-7342-bfa4-ae5a150adde4","harness_session_source":"CODEX_THREAD_ID"}
