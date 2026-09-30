---
id: tasks-2325a1
title: check finds uncommitted plan/spec docs in the main checkout instead of failing a fresh worktree
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: feat/cross-checkout
created: 2026-09-20T10:08:20Z
updated: 2026-09-30T09:44:06Z
started: 2026-09-30T09:40:14Z
completed: 2026-09-30T09:44:06Z
depends: []
parent: tasks-c4ad8e
tags: [feedback, friction, "from:mind6"]
agent: opencode/glm-5.3
---

Why: check tests ctx.project.root.join(path).is_file() for open tasks' spec and plan (src/commands/check.rs, doc_missing). A project that keeps specs out of git (.git/info/exclude; whether specs are committed is each project's profile call) has them in the main checkout only, so every new worktree fails doc_missing and its pre-commit gate blocks all work. Reported by mind6 via tasks feedback (opencode/glm-5.3).

Decided 2026-09-29 (user): look in the main checkout, not hydrate docs through just setup.

Done: when a linked doc is absent under the current root, check looks for it at the same project offset in the main worktree (first entry of git worktree list --porcelain -z; reuse the discovery and offset logic behind Project::sibling_task_copies / git_toplevel in src/repo.rs). Found there: a warning finding (kind e.g. doc_in_main_checkout) naming that checkout, not an error. Absent in both, or not a git repo: doc_missing as today. step_missing reads the plan from wherever it was found, so a step check still runs (resolver in src/resolve.rs reads project.root today). Running in the main checkout itself behaves exactly as now.

Check: integration tests in tests/cli.rs against real git worktrees (the sibling-copy tests are the model): an excluded spec present only in main gives a warning and exit 0 from the worktree; a missing-everywhere spec still errors; a plan step resolves from main's copy. Update the doc-link section of docs/specs/2026-08-29-tasks-design.md (§7) and skills/tasks/SKILL.md if it describes doc_missing. Brief: docs/notes/2026-09-29-cross-checkout-records-brief.md.

## Notes

- 2026-09-29T20:50:25Z (main): scope: briefed; open question for the user/mind6: hydrate uncommitted specs into worktrees via just setup, or have check look in the main checkout and downgrade doc_missing to a warning naming it (current lean); parented to tasks-c4ad8e; brief: docs/notes/2026-09-29-cross-checkout-records-brief.md
- 2026-09-29T20:56:10Z (main): scope: scoped; user chose 'look in the main checkout' (2026-09-29); todo P2 s/mid/direct with approach and tests in the body; brief: docs/notes/2026-09-29-cross-checkout-records-brief.md
- 2026-09-30T09:40:14Z (feat/cross-checkout): started
  provenance: {"harness_session":"claude-code:6cd5b571-f527-4c6a-a3d0-bc87777c0b2c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T09:44:00Z (feat/cross-checkout): Found while implementing: show (io error) and add/edit step validation still read the plan from the local root in a worktree; reproduced on a scratch repo and filed as tasks-ace27b rather than widening this task.
- 2026-09-30T09:44:06Z (feat/cross-checkout): done
  provenance: {"harness_session":"claude-code:6cd5b571-f527-4c6a-a3d0-bc87777c0b2c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T09:44:06Z (feat/cross-checkout): check looks for a spec or plan missing from a linked worktree in the main worktree (Project::main_checkout_root, sharing worktree_roots with sibling_task_copies); found there it is the warning doc_in_main_checkout naming that checkout and step checks read that copy; absent from both is still doc_missing; design §7 updated; show/add/edit follow-up tasks-ace27b
  provenance: {"harness_session":"claude-code:6cd5b571-f527-4c6a-a3d0-bc87777c0b2c","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
