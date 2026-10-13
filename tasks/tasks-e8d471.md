---
id: tasks-e8d471
title: show lists a task's own dependencies but not the tasks that depend on it
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-09T14:58:40Z
updated: 2026-10-13T11:10:30Z
depends: []
tags: [quick-add]
source: "mindful:thought:f489a269e04a438ab5732282730531aa"
agent: claude-code/claude-opus-5-5
---

Why: `tasks show` prints the edges a task declares (`depends_on`, `parent`) and its `children`, but not the dependency edges other tasks hold toward it. A task that others depend on shows no dependents; for example, a cross-project piece does not list the hub goal that depends on it. Source thought (mindful:thought:f489a269e04a438ab5732282730531aa, "tasks: inferred relations"): show conveys outgoing edges only; tasks should convey every relation they take part in, by default or optionally. Its other example ("A lists B as a child while B reports no parent") does not occur: a parent must be in the same project (src/hierarchy.rs:18), `show` already derives `children` from its scan, and the child stores its parent. Only incoming `depends` edges are missing.

Established at scoping (2026-10-13): `describe` in src/commands/show.rs builds `children` from the `all` scan it is handed, and `next` passes it the scope-wide scan, so dependents computed the same way need no new lookup. `move` already finds inbound dependents across every registered project (`moving::inbound`, src/moving/mod.rs:163: `Scope::open_all` plus `registry.canonical_id` on each `depends` entry). Cost measured on this host: a local `show` takes about 0.03 s, and a scan of all 28 projects (`list --all-projects`) about 1.3 s for open tasks and 3.5 s for done ones (5130 records). That is why the cross-project read is opt-in: scanning every project by default would make every `show` (which agents call constantly) about 40× slower.

Done:
- `ShowFields` gains `dependents: Vec<Related>` (id, title, status), omitted when empty like `children`: every task in `all` whose `depends` names this task under any spelling (`registry.canonical_id`), sorted as `children` are. Open and closed dependents are both listed; status is in the row.
- Plain `show` reads dependents from the project scan it already holds, at no extra cost.
- `show --all-projects` widens the dependents scan to every reachable registered project through `Scope::open_all` and carries its unreachable-project warnings. Share the scan with `moving::inbound` rather than copying it. Other `show` fields are unchanged by the flag.
- `next` gets `dependents` from the scan it already holds (it calls `describe`), so `next --all-projects` lists cross-project dependents too.
- Pretty `show` prints a dependents section beside depends-on and children, in the same row style.
- `skills/tasks/SKILL.md` says how to see who depends on a task (`show`, and `show --all-projects` for other projects).
- Tests in tests/cli.rs: a local dependent is listed; a different-prefix dependent appears only with `--all-projects`; an alias spelling of the id still counts; no `dependents` key when none.

Where: src/commands/show.rs (`describe`, `run`), src/output.rs (`ShowFields`, `Related`, pretty show), src/moving/mod.rs (`inbound`), src/scope.rs (`open_all`), src/cli.rs (show args), src/commands/list.rs (`next` calls `describe`).
