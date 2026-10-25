---
id: tasks-284d9b
title: Route list and other filtered reads to the project an id-valued flag names when run outside every project
status: doing
priority: 2
size: m
complexity: mid
process: planned
owner: design/tasks-284d9b
created: 2026-10-25T10:22:14Z
updated: 2026-10-25T10:31:02Z
started: 2026-10-25T10:29:37Z
depends: []
tags: [cli]
source: "user:2026-10-25 list outside project"
agent: claude-code
---

Motivation: commands that take a task id positionally (show, tree <id>, note, start) work from anywhere on the host: the id's prefix selects the registered root (README "Routing across projects"; src/commands/mod.rs::open_id_ctx_unwarned falls back to the registry on NoProject). Filtered reads do not. From a directory outside every project:

    tasks list --source ops-2fe949 --status todo   -> {"error":{"kind":"no_project",...}}
    tasks list --under ops-2fe949                  -> no_project
    (and --parent the same)

while `tasks show ops-2fe949` works there. The read path (open_read_ctx) only knows the local project, --project, --group and --all-projects.

Goal: when no local project is found, a read whose id-valued filter names a full task id opens that id's registered project, through one shared helper rather than per-command code, so list, search, ready and any other command with such flags behave alike.

Decisions the design must settle:
- Which flags route. --parent and --under are id-valued (REF) and are the clear candidates. --source is documented as "matched byte for byte, never interpreted"; routing on an id-shaped source would interpret it, so either keep it out, route only when the value parses as a full id of a registered prefix, or route --source to --all-projects-style search. The user's example uses --source, so the answer must say what that command does.
- Conflicts: two id-valued flags naming different prefixes; an id flag together with --project, --group or --all-projects; a bare six-hex suffix outside a project (stays invalid_id).
- Inside a project: whether an id flag for another prefix should also route (as positional ids do) or keep filtering the local project, and what warning it gives.
- Which commands share the helper (list, search, ready, next, tree, lanes, any other read with id-valued flags) and whether open_id_ctx_unwarned and the new helper share one resolver.
- Whether JSON output changes (it should not without saying so).

Constraints: no silent fallback; a prefix not in the registry stays a typed error; preserve checkout and worktree precedence for the local prefix; any new option starts on ops's cli.toml (AGENTS.md Rules), though this should need none.

Related: tasks-e147b9 investigates full-id routing for attach from scratch directories (broken ancestor .git markers); keep this design consistent with its finding and do not duplicate it.

## Notes

- 2026-10-25T10:29:37Z (main): started
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T10:31:02Z (design/tasks-284d9b): resumed
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
