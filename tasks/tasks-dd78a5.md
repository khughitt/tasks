---
id: tasks-dd78a5
title: "The tasks skill's worktree setup sentence (just setup, otherwise only the root guide's explicit command) is narrower than the global instructions' setup order, which also allows a README/CONTRIBUTING command that writes only inside the worktree and a lockfile's frozen worktree-local install, reports a bare manifest before the first commit, and requires hydration before the first commit; align the skill with that order"
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: docs/tasks-dd78a5
created: 2026-10-17T12:57:57Z
updated: 2026-10-23T00:44:59Z
started: 2026-10-23T00:44:22Z
depends: []
tags: [feedback, gap, "from:lore"]
agent: claude-code/claude-opus-5-5
---

Why: the tasks skill's worktree setup sentence is narrower than the global instructions' setup order, so on a checkout with only a package-lock.json an agent cannot tell whether to run `npm ci`; unhydrated worktrees make hooks stop at an npx install prompt or fetch unpinned tools.

Established (scope 2026-10-17): the narrow sentence came from tasks-63742b/tasks-5e6971 (2026-09-15, docs/notes/2026-09-15-fresh-worktree-brief.md), before the global instructions grew the longer order. Copies: skills/tasks/SKILL.md:348 (Process and workspace: 'Then run `just setup` when defined; otherwise run only the root guide's explicit setup command. Do not guess an installer.'), AGENTS.md:41-43 (paraphrase of the skill's policy), docs/specs/2026-08-29-tasks-design.md:963. docs/specs/2026-09-13-task-process-design.md:52 is a dated design record; leave it.

Done: the skill states the global order: `just setup` when the justfile defines it; else the setup command the root guide names, or one its README or CONTRIBUTING names that writes only inside the worktree; else the frozen install of a lockfile that names its package manager and installs inside the worktree (package-lock.json: `npm ci`; uv.lock: `uv sync --frozen`). A bare manifest, or a lockfile whose install can leave the worktree, declares no installer: never guess one, report the missing route before the first commit. Hydrate before the first commit. AGENTS.md and the tasks design spec say the same in their own words. Keep it compact; the order's wording follows the global instructions so the two cannot drift apart in meaning.

Where: the three paths above. Verify with `grep -n "only the root guide" skills AGENTS.md docs/specs/2026-08-29-tasks-design.md` (no hits) and `just check`.

Original report (preserved): Process and workspace section, 'Then run just setup when defined; otherwise run only the root guide's explicit setup command'. Two instructions now disagree on a checkout with only a package-lock.json, so an agent cannot tell whether to run npm ci; unhydrated worktrees make hooks stop at an npx install prompt or fetch unpinned tools.

## Notes

- 2026-10-17T17:50:15Z (main): scope: scoped; narrow sentence traced to tasks-63742b (2026-09-15) predating the global order; copies at skills/tasks/SKILL.md:348, AGENTS.md:41, docs/specs/2026-08-29-tasks-design.md:963; doc-only alignment
- 2026-10-23T00:31:05Z (main): Follow-up check after tasks-9a9ef3: the seven-area trim deliberately left Process and workspace setup policy unchanged. The narrow root-guide-only sentence remains; this existing task is the appropriate next fix, not a new concern against the completed trim.
- 2026-10-23T00:44:22Z (main): started
  provenance: {"harness_session":"codex:01a16636-449d-7a42-8ebb-98f3b41b15a5","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-23T00:44:59Z (docs/tasks-dd78a5): resumed
  provenance: {"harness_session":"codex:01a16636-449d-7a42-8ebb-98f3b41b15a5","harness_session_source":"CODEX_THREAD_ID"}
