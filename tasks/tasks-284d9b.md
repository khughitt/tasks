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
updated: 2026-10-25T10:52:11Z
started: 2026-10-25T10:29:37Z
depends: []
tags: [cli]
source: "user:2026-10-25 list outside project"
agent: claude-code
spec: docs/specs/2026-10-25-filtered-read-routing-design.md
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
- 2026-10-25T10:31:04Z (design/tasks-284d9b): Process: planned; workspace .worktrees/tasks-284d9b on design/tasks-284d9b. Hydrated with README cargo install --locked --path . confined to this worktree (installed files retained under target/hydrated); shared host binary unchanged. Full-suite baseline deferred to execution Setup under the planned-work rule. Design will retain source's literal semantics and recommend explicit --all-projects for cross-project provenance searches; tasks-e147b9 has no finding yet, so preserve existing NoProject-only fallback and obstruction errors.
- 2026-10-25T10:34:08Z (design/tasks-284d9b): review: spec round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-25T10:34:10Z (design/tasks-284d9b): attached: filtered-read-builder-check.md (467 bytes): Full read-only builder check of spec ce08cdc; codex/gpt-6.1-sol accepted with no findings
- 2026-10-25T10:34:13Z (design/tasks-284d9b): retained: {"host":"titan","kind":"worktree","path":".worktrees/tasks-284d9b","state":"kept","verification":"verified","purpose":"Frozen spec ce08cdc awaiting user review; branch design/tasks-284d9b includes the full builder report attachment and review ledger under .superpowers/sdd/tasks-284d9b","next_owner":"task:tasks-284d9b","cleanup_trigger":"After user spec approval, prepare and review the implementation plan; after implementation integration, inspect ignored files and run removal checks"}
- 2026-10-25T10:34:15Z (design/tasks-284d9b): parked (waiting on user, review): User reviews spec ce08cdc6be0068351729c6e0b94ab63dd84a0394 at docs/specs/2026-10-25-filtered-read-routing-design.md in .worktrees/tasks-284d9b; after approval, agent resumes here and writes the implementation plan. Builder check accepted with no findings; --source remains literal and requires explicit scope outside every project.
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T10:49:04Z (design/tasks-284d9b): resumed
  provenance: {"harness_session":"codex:01a1729a-f51f-74f3-85e6-4695b73e5254","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T10:49:07Z (design/tasks-284d9b): review: spec round 2 — verdict: revise; findings: Important 1, Minor 3; reviewer: claude-code/claude-opus-5-5
- 2026-10-25T10:49:10Z (design/tasks-284d9b): Review numbering: the builder acceptance was recorded at 2026-10-25T10:34:08Z as spec round 1, so the relayed ce08cdc review is round 2. Verified hierarchy::validate_parent rejects foreign parents. Accept Important routing correction: without explicit scope, filters follow positional-ID routing inside or outside a project; same-prefix filters retain the local checkout. Accept all three minors: actionable no_project filter guidance, shared tree/filter completion using local_or_foreign, and an explicit statement that cross-project hierarchy predicates intersect to empty even in a wide scope. Process remains planned; revise only the spec in .worktrees/tasks-284d9b before a new frozen review.
- 2026-10-25T10:50:41Z (design/tasks-284d9b): attached: filtered-read-spec-round-2-review.md (4430 bytes): Full relayed claude-code/claude-opus-5-5 review of ce08cdc; Important 1 and Minor 3, all accepted for spec revision
- 2026-10-25T10:52:06Z (design/tasks-284d9b): review: spec round 3 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-25T10:52:09Z (design/tasks-284d9b): attached: filtered-read-spec-round-3-review.md (1536 bytes): Full fresh codex/gpt-6.1-sol builder check of f03f0b4; accept, no findings, all four prior findings addressed
- 2026-10-25T10:52:11Z (design/tasks-284d9b): attached: filtered-read-spec-review-ledger.md (3126 bytes): Spec review rounds 1–3 and four accepted rulings; f03f0b4 awaits user review, no deferred minors
