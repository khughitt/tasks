---
id: tasks-73f253
title: The tasks skill's Process and workspace setup-hydration sentence lags the global rule on confining a declared setup command and on read-only worktrees
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: docs/hydration-confinement
created: 2026-10-23T10:20:40Z
updated: 2026-10-23T10:50:48Z
started: 2026-10-23T10:44:55Z
completed: 2026-10-23T10:50:48Z
closed: 2026-10-23T10:50:48Z
depends: []
tags: [feedback, gap, "from:lore"]
agent: claude-code/claude-opus-5-5
---

Why: the global Git rule's worktree-hydration sentences gained two clauses that the copies here lack, so an agent reading only the tasks skill (or this repo's AGENTS.md) can treat confining a declared setup command as an approval stop, or hydrate and baseline a worktree checked out only to be read. Established at scoping (2026-10-23, against main 1524a4a): the three local copies were last aligned in 15c2f98 ("align worktree setup policy with declared hydration order") and carry neither clause.

The global wording to mirror, in its order:
- After "else the frozen install of a lockfile ...": a command the root guide, README or CONTRIBUTING names may have its default build or install destinations pointed inside the worktree by its own documented settings or flags (e.g. `cargo install --locked --path .` with `--root` and `CARGO_TARGET_DIR` under the worktree), and must have them when it would otherwise replace a tool on the host's PATH; a shared download cache is not an environment and may stay where it is. That confinement is not guessing an installer and needs no approval, provided the confined run neither swaps the package manager nor writes a shared environment. A README or CONTRIBUTING command no documented setting confines, or any named command that would still replace a host PATH tool, is reported as a missing setup route before the first commit.
- The missing-setup-route report informs, asks no approval, and the work continues.
- A worktree checked out only to be read (e.g. a historical snapshot under review, where nothing is committed, tested or run) needs no hydration until something is to run or be committed in it, when hydration comes first.

Done: skills/tasks/SKILL.md's "Process and workspace" worktree paragraph (around line 300), AGENTS.md's "Process and workspace" paragraph (around line 41), and docs/specs/2026-08-29-tasks-design.md's hydration paragraph (around line 963) each carry these clauses in the global rule's order and exemptions, adapted to their own voice, without the baseline-test clauses the skill does not cover; `just check` passes. Docs-only: no CLI or JSON change, no cli.toml row.

Where: the global rule is the Git section of the user's global instructions (owned by lore); this task only mirrors it.

## Notes

- 2026-10-23T10:24:48Z (main): scope: scoped; premises checked against main 1524a4a: skills/tasks/SKILL.md, AGENTS.md and the design spec still carry the 15c2f98 wording without the confinement/no-approval and read-only-worktree clauses; body rewritten with the clauses and all three locations; todo, P2 unchanged, xs, low, direct
- 2026-10-23T10:44:55Z (main): started
  provenance: {"harness_session":"claude-code:c2975679-c527-4213-b8e2-df18920e1084","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-23T10:45:02Z (docs/hydration-confinement): resumed
  provenance: {"harness_session":"claude-code:c2975679-c527-4213-b8e2-df18920e1084","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-23T10:50:48Z (docs/hydration-confinement): done
  provenance: {"harness_session":"claude-code:c2975679-c527-4213-b8e2-df18920e1084","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-23T10:50:48Z (docs/hydration-confinement): skill, AGENTS.md and design spec hydration paragraphs now carry the global rule's setup-command confinement, no-approval report, and read-only worktree exemption
  provenance: {"harness_session":"claude-code:c2975679-c527-4213-b8e2-df18920e1084","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
