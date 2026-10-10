---
id: tasks-6939f6
title: "Commit task records as tasks writes them, so sessions stop leaving tasks/ dirty"
status: doing
priority: 1
size: l
complexity: high
process: planned
owner: design/record-autocommit
created: 2026-10-10T15:21:59Z
updated: 2026-10-10T17:46:02Z
started: 2026-10-10T15:22:05Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-10-record-commit-design.md
plan: docs/plans/2026-10-10-record-commit.md
---

Agents create and modify task records and rarely commit them, so every project accumulates
uncommitted changes under tasks/ (ops on 2026-10-10: 4 modified, 7 untracked). The user
then sorts and commits them by hand, and the next agent starts from a dirty tree whose
changes it cannot attribute. Tasks are already vetted before anyone acts on them (scope,
process, reviews), so a record write does not need a human gate before it is committed.

Goal: a tasks write leaves the record committed, and a dirty tasks/ becomes an exception
that names its cause, not the normal state.

Observed sources of the backlog (ops, 2026-10-10):
- Own-session lifecycle writes in the checkout the session stands in: start, note, park
  (ops-b46491, ops-0a6ba1, ops-a0318e).
- Cross-project writes routed by id prefix into another project's registered checkout:
  all 7 untracked records are feedback filed from sci/hq/ops sessions, and one modified
  record is a feedback recurrence note from vdocs. These land in a repository the writing
  session does not own, possibly while another session works there.

Design questions the spec must settle:
1. Mechanism: commit inside the CLI after each write (pathspec-only, `git commit --only`
   or a temporary index, so a person's staged work is never swept in), a batch command
   (`tasks commit`), or instructions alone. Instructions alone is what exists today and
   is the failure being fixed.
2. Opt-in and scope: per-project config (e.g. `[git] commit = true` in
   tasks/.config.toml), which commands commit, and message format (conventional
   `chore(tasks): …` naming id and verb).
3. Worktrees: on a task branch the record commit lands on the branch, so the "done in the
   same commit as the code" rule becomes "on the same branch"; reconcile with stale_copy,
   claim moves, and the commit-before-worktree rule (tasks-5a46b9).
4. Cross-project writes: commit in the target checkout or not; the feedback protocol uses
   "uncommitted" as its redaction/triage queue, so committing feedback needs another
   marker for untriaged reports (e.g. the feedback tag on an idea without a triage note).
5. Hooks and refusals: pre-commit content gates can refuse a record-only commit for
   unrelated content (tasks-850488, tasks-cee8d9, ops-429f24, ops-89d93b). The write must
   fail loudly or warn with the cause and route, never silently leave the record dirty,
   and never bypass hooks.
6. Linked projects (overlay repository) and non-git roots; Dropbox-synced .git;
   concurrent sessions committing in one repository (index.lock contention).
7. What prime/check report about whatever remains uncommitted (tasks-80fec3 warns today).

Also owed: the tasks skill's session protocol and lore's global instructions change with
whichever mechanism lands.

## Notes

- 2026-10-10T15:22:05Z (main): started
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T15:22:18Z (design/record-autocommit): resumed
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T15:25:12Z (design/record-autocommit): spec drafted: docs/specs/2026-10-10-record-commit-design.md — CLI commits each write by pathspec under opt-in [git] commit; skips with typed warnings when other changes are staged or an operation is in progress; feedback committed (triage marker moves to untriaged feedback ideas); tasks commit for recovery/backlog sweep
- 2026-10-10T15:25:43Z (design/record-autocommit): parked (waiting on user, review): User reviews docs/specs/2026-10-10-record-commit-design.md in .worktrees/record-autocommit; on approval the agent writes the implementation plan there (writing-plans)
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T15:42:10Z (design/record-autocommit): resumed
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T15:42:10Z (design/record-autocommit): review: spec round 1 — verdict: revise; findings: must-fix 3, should-fix 1; reviewer: unverified (pasted review, harness and model not supplied)
- 2026-10-10T15:43:47Z (design/record-autocommit): spec revised for round 1: plain commit (no pathspec) over a clean index, hook-staged files allowed in the commit; defer on any staged change; repository commit lock keyed by git common dir held through check/add/commit/reset; rollout lists tasks AGENTS.md, README, scope skill, and lore instruction-provenance T12/T21 before any project enables it
- 2026-10-10T15:43:56Z (design/record-autocommit): parked (waiting on user, review): User reviews revised spec docs/specs/2026-10-10-record-commit-design.md (round 2) in .worktrees/record-autocommit; on approval the agent writes the implementation plan there
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T15:55:33Z (design/record-autocommit): resumed
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T15:55:33Z (design/record-autocommit): correction: review spec round 1 reviewer was codex (model unverified)
- 2026-10-10T15:55:33Z (design/record-autocommit): review: spec round 2 — verdict: revise; findings: P1 1; reviewer: codex
- 2026-10-10T15:55:53Z (design/record-autocommit): spec revised for round 2: failed commit unstages only the command's paths; other staged paths are kept and named under 'cleanup needs attention'; test for outside staging during a refusing hook
- 2026-10-10T15:55:55Z (design/record-autocommit): parked (waiting on user, review): User reviews revised spec docs/specs/2026-10-10-record-commit-design.md (round 3) in .worktrees/record-autocommit; on approval the agent writes the implementation plan there
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T16:05:06Z (design/record-autocommit): resumed
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T16:05:06Z (design/record-autocommit): review: spec round 3 — verdict: accept; findings: P3 1; reviewer: codex/gpt-6.1-sol
- 2026-10-10T16:05:06Z (design/record-autocommit): correction: spec rounds 1 and 2 reviewer was codex/gpt-6.1-sol (model supplied by the user)
- 2026-10-10T16:15:22Z (design/record-autocommit): plan written: docs/plans/2026-10-10-record-commit.md, 7 steps (tasks-52539c, -645ead, -b479b6, -fef9bc, -6b4f6c, -de5277, -aeeaeb); spec P3 qualifiers applied in bdbee6c
- 2026-10-10T16:15:22Z (design/record-autocommit): parked (waiting on user, review): User reviews docs/plans/2026-10-10-record-commit.md in .worktrees/record-autocommit and picks subagent-driven or native execution; then the agent runs Task 1 (tasks-52539c)
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T16:30:06Z (design/record-autocommit): resumed
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T16:30:06Z (design/record-autocommit): review: plan round 1 — verdict: revise; findings: P2 5; reviewer: codex
- 2026-10-10T16:35:16Z (design/record-autocommit): plan revised for round 1: 4 tasks (engine + tasks commit; write-command wiring incl. config/journal; docs; rollout) so every commit has callers under clippy -D warnings; cleanup failures reported; one commit per working tree; feedback/start warnings computed after the commit; cli.toml row split to ops-166dab with the ops workspace protocol
- 2026-10-10T16:35:25Z (design/record-autocommit): parked (waiting on user, review): User reviews revised plan docs/plans/2026-10-10-record-commit.md (round 2) in .worktrees/record-autocommit and picks subagent-driven or native; execution starts with ops-166dab (cli.toml row), then Task 1 (tasks-b479b6)
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T16:52:01Z (design/record-autocommit): resumed
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T16:52:01Z (design/record-autocommit): review: plan round 2 — verdict: accept; findings: P3 1; reviewer: codex
- 2026-10-10T16:52:11Z (design/record-autocommit): plan approved after round 2; P3 applied (skill subsection names hook-staged files and the §3.7 outside-staging exception)
- 2026-10-10T16:52:13Z (design/record-autocommit): parked (waiting on user, decision): Paused by the user with the plan approved. Next: an agent runs ops-166dab (cli.toml row) in ops, then executes the plan subagent-driven from Task 1 (tasks-b479b6) in .worktrees/record-autocommit
  provenance: {"harness_session":"claude-code:3f1edfe9-f250-48e8-be83-b3bc38bb116f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-10T17:01:33Z (design/record-autocommit): resumed
  provenance: {"harness_session":"codex:01a126c1-985a-7180-9638-8c3316f76f55","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-10T17:02:50Z (design/record-autocommit): execution: approved plan resumed subagent-driven in .worktrees/record-autocommit; just test-fast baseline passed; ops-166dab prerequisite assigned before Task 1
- 2026-10-10T17:43:58Z (design/record-autocommit): review: impl round 1 — verdict: revise; findings: Important 2, Minor 2; reviewer: codex/gpt-6-astra
- 2026-10-10T17:46:01Z (design/record-autocommit): final corrective round 1: fix paused cherry-pick/revert sequencer detection and silent Git/hook diagnostics under spec section 3.3; two Minor recovery/documentation findings remain deferred
- 2026-10-10T17:46:01Z (design/record-autocommit): integration gate: installed tasks and scope skills resolve to main checkout, so merging activates shared agent guidance; live-global-surface approval required before merge, installation and post-merge rollout filing
