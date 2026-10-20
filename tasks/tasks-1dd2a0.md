---
id: tasks-1dd2a0
title: Explain ordinary-task fallback when a feedback owner has not opted in
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-01T23:14:04Z
updated: 2026-10-20T12:20:12Z
depends: []
parent: tasks-6ed79b
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Why: the user ruled on 2026-10-20 that `tasks feedback` accepts every registered project by default (ops-fec58a, renewing their 2026-10-12 ask "it may make more sense to accept `feedback` for any project registered with `ops`"). Today feedback::require_acceptance refuses any target without a `[feedback]` table, so a report about a tool owned by a project that has not opted in has no feedback route; the hand-filed `tasks add` workaround loses the feedback tag, the category and the from: provenance, so the report never reaches the owner's triage queue. The 2026-10-08 scope below kept opt-in and only explained that untagged fallback; the ruling supersedes it.

Established (2026-10-20, tasks main): 7 of the 35 registered projects carry `[feedback]` (flows, hq, lore, obs, ops, relay, tasks). The registry also holds four work-profile projects (nexc, nexp, nrp, rad), which "every registered project" includes. `owners()` (src/commands/feedback.rs:41-72) already skips retired and unreachable projects and is what both the refusal's owner list and completion (src/complete.rs:679-688) read. check's dictionary exemption for a report's tags is gated on `ctx.project.feedback.is_some()` (src/commands/check.rs:262-270). src/repo.rs:84-86 documents `feedback: Some` as "the project accepts feedback", and :254-257 ties its scope rule to ops's bin/ops-projects, which validates `[feedback]` as exactly one key, scope.

Done: every registered, reachable, unretired project accepts `tasks feedback`, and the report lands with the feedback, category and from: tags as it does today. A `[feedback]` table stays optional and keeps its one key: `scope` now declares the shared tooling a project owns for routing (the owner list, ops's generated Feedback owners block), not whether it accepts. No opt-out key: add one when a project asks for it, as a compatible change made in both readers. check holds the feedback tags as defined on feedback records in every project. Completion for `feedback --project` offers every accepting project, not only those with a scope. A retired target still refuses as it does now; JSON shapes, recurrence and deduplication are unchanged. README Feedback and skills/tasks/SKILL.md Feedback describe default acceptance; "In a project that accepts feedback" keeps its triage rules, and its opt-in paragraph becomes the scope declaration.

Where to look: src/commands/feedback.rs (require_acceptance, owners, locate_target and the acceptance recheck under the lock); src/commands/check.rs:262-270; src/complete.rs:679-688; src/repo.rs:84-86, 254-266; tests/cli.rs::feedback_refuses_an_owner_that_does_not_accept_and_lists_those_that_do; README.md:981; skills/tasks/SKILL.md:729-760. The approved design is ops docs/specs/2026-09-26-ecosystem-feedback-design.md; its amendment and the generated header are ops-92168d, which waits on this task.

Verification: replace the refusal test with one where a registered target without `[feedback]` accepts a report carrying all three tags and a target with `[feedback]` still does; a retired target still refuses; in a project with a tag dictionary and no `[feedback]`, `tasks check` is silent on a report's tags; completion lists a project without `[feedback]`. Use just test-one for the focused tests and just test-fast before the code commit.

Process: direct; the user's ruling settles the outcome and this body the approach. P2/s/low.

Superseded scope (2026-10-08, preserved): Keep the current validation error, no-write refusal and owner list. Add guidance that, when authorized to file in the actual owning project, the caller can use tasks add --project <owner> --status idea "<summary>" -b "<details>" after checking for an existing task. This is an explicit ordinary-task operation, not automatic retry or feedback routing. Do not add feedback/category/from tags to this fallback: check exempts them from vocabulary checks only in projects with [feedback]. Do not enable [feedback], change recurrence/deduplication behavior or alter JSON shapes. Its why: "feedback::require_acceptance already preserves explicit owner opt-in and lists accepting projects, but does not explain how to record an ordinary idea when none owns the tool. Automatically accepting feedback everywhere would change the routing contract unnecessarily." Its verification extended the refusal test to cover the fallback hint, and checked that the documented ordinary add creates one untagged idea that tasks check accepts. The original feedback design notes that its fixed-target rule is superseded.

Original report and prior context (preserved):
Filing friction about a repo-local tool with `tasks feedback --project <that project>` failed: '"<project>" does not accept feedback (no [feedback] in its tasks/.config.toml)'. The error lists the projects that do accept feedback, but none of them own the tool, so the agent had to fall back to a plain `tasks add` in the owning project and file by hand here. Options: let feedback land in any registered project as an idea tagged feedback (the [feedback] scope only steering routing), or have the error name that fallback.

Lane recommendation to evaluate: retain explicit feedback opt-in and make the refusal explain the supported tasks add fallback for a tool owned by a non-accepting project. Inspect the current diagnostic and feedback special-tag behavior before changing the contract; a guidance/diagnostic fix may cover the need without enabling unsolicited feedback everywhere. Keep this as an idea until that bounded choice is scoped.

## Notes

- 2026-10-06T16:52:04Z (main): organization: assigned to guidance/adoption; evaluate explaining the existing add fallback before broadening feedback routing and opt-in semantics.
- 2026-10-08T09:50:36Z (main): scope: scoped; P2/s/low/direct; retain feedback opt-in and explain an explicit untagged tasks add fallback; current tag exemptions apply only to accepting projects, so do not imitate feedback tags
- 2026-10-20T12:02:27Z (main): From the coverage-drop audit (tasks-fa3525): this task's scope (2026-10-08: keep opt-in, "Automatically accepting feedback everywhere would change the routing contract unnecessarily"; "Do not add feedback/category/from tags to this fallback") predates the user's own renewed ask. On 2026-10-12T23:40:59Z (claude-code:b7c6aaa3-e7cb-415b-9603-da5784af28fb) the user wrote: "> lit doesn't accept tasks feedback.. Let's file a friction report with ops; it seems like it may make more sense to accept `feedback` for any project registered with `ops`?" The report filed from it, ops-fec58a, also asked that the fallback keep the feedback tag, category and from: provenance ("so the report never reaches the owner's triage queue"). Its drop (2026-10-13) said "raise it there if opt-out is wanted", but nothing reached this task. Still live: src/commands/feedback.rs:84-105 (require_acceptance) refuses every project without [feedback]. The user's ruling between an opt-out default and this task's opt-in with an untagged fallback is pending; the audit follow-up raised it with the user.
  provenance: {"harness_session":"claude-code:40cb347f-4334-45a0-bd26-2650ae480230","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-20T12:20:10Z (main): ruling (user, 2026-10-20, in the session stamped here): "ops-fec58a: yes - accept every registered project by default." This replaces the 2026-10-08 scope that kept feedback opt-in.
  provenance: {"harness_session":"claude-code:40cb347f-4334-45a0-bd26-2650ae480230","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-20T12:20:12Z (main): scope: rescoped; premise "retain feedback opt-in" replaced by the user's 2026-10-20 ruling (accept every registered project by default); checked against tasks main (src/commands/feedback.rs:41-105, check.rs:262-270, complete.rs:679-688, repo.rs:84-86) and ops bin/ops-projects:251-262 and the 2026-09-26 design; Done now default acceptance with the three tags, [feedback] scope kept as a routing declaration, no opt-out key; the design amendment and generated header split to ops-92168d (depends on this); size, complexity and process unchanged (s/low/direct), priority unchanged.
