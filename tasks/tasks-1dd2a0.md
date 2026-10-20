---
id: tasks-1dd2a0
title: Explain ordinary-task fallback when a feedback owner has not opted in
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-01T23:14:04Z
updated: 2026-10-20T12:02:27Z
depends: []
parent: tasks-6ed79b
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Why: feedback::require_acceptance already preserves explicit owner opt-in and lists accepting projects, but does not explain how to record an ordinary idea when none owns the tool. Automatically accepting feedback everywhere would change the routing contract unnecessarily.

Done: Keep the current validation error, no-write refusal and owner list. Add guidance that, when authorized to file in the actual owning project, the caller can use tasks add --project <owner> --status idea "<summary>" -b "<details>" after checking for an existing task. This is an explicit ordinary-task operation, not automatic retry or feedback routing. Do not add feedback/category/from tags to this fallback: check exempts them from vocabulary checks only in projects with [feedback]. Do not enable [feedback], change recurrence/deduplication behavior or alter JSON shapes.

Where to look: src/commands/feedback.rs::require_acceptance and owners; src/commands/add.rs; src/commands/check.rs tag validation; tests/cli.rs::feedback_refuses_an_owner_that_does_not_accept_and_lists_those_that_do; README.md Feedback and skills/tasks/SKILL.md Feedback. The original feedback design notes that its fixed-target rule is superseded; use current owner opt-in behavior.

Verification: Extend the existing focused refusal test to cover the fallback hint while retaining error.kind=validation, the owner list, unreadable-owner details and no writes. In an isolated registered target without [feedback] and with a tag dictionary, the documented ordinary add creates one untagged idea and tasks check reports no undefined tags; no fallback is executed automatically. Use just test-one for the focused tests and just test-fast before the code commit.

Process: direct; retain opt-in and document an existing command, with a bounded diagnostic change. P2/s/low.

Original report and prior context (preserved):
Filing friction about a repo-local tool with `tasks feedback --project <that project>` failed: '"<project>" does not accept feedback (no [feedback] in its tasks/.config.toml)'. The error lists the projects that do accept feedback, but none of them own the tool, so the agent had to fall back to a plain `tasks add` in the owning project and file by hand here. Options: let feedback land in any registered project as an idea tagged feedback (the [feedback] scope only steering routing), or have the error name that fallback.

Lane recommendation to evaluate: retain explicit feedback opt-in and make the refusal explain the supported tasks add fallback for a tool owned by a non-accepting project. Inspect the current diagnostic and feedback special-tag behavior before changing the contract; a guidance/diagnostic fix may cover the need without enabling unsolicited feedback everywhere. Keep this as an idea until that bounded choice is scoped.

## Notes

- 2026-10-06T16:52:04Z (main): organization: assigned to guidance/adoption; evaluate explaining the existing add fallback before broadening feedback routing and opt-in semantics.
- 2026-10-08T09:50:36Z (main): scope: scoped; P2/s/low/direct; retain feedback opt-in and explain an explicit untagged tasks add fallback; current tag exemptions apply only to accepting projects, so do not imitate feedback tags
- 2026-10-20T12:02:27Z (main): From the coverage-drop audit (tasks-fa3525): this task's scope (2026-10-08: keep opt-in, "Automatically accepting feedback everywhere would change the routing contract unnecessarily"; "Do not add feedback/category/from tags to this fallback") predates the user's own renewed ask. On 2026-10-12T23:40:59Z (claude-code:b7c6aaa3-e7cb-415b-9603-da5784af28fb) the user wrote: "> lit doesn't accept tasks feedback.. Let's file a friction report with ops; it seems like it may make more sense to accept `feedback` for any project registered with `ops`?" The report filed from it, ops-fec58a, also asked that the fallback keep the feedback tag, category and from: provenance ("so the report never reaches the owner's triage queue"). Its drop (2026-10-13) said "raise it there if opt-out is wanted", but nothing reached this task. Still live: src/commands/feedback.rs:84-105 (require_acceptance) refuses every project without [feedback]. The user's ruling between an opt-out default and this task's opt-in with an untagged fallback is pending; the audit follow-up raised it with the user.
  provenance: {"harness_session":"claude-code:40cb347f-4334-45a0-bd26-2650ae480230","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
