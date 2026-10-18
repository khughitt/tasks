---
id: tasks-52bd39
title: "tasks dep --on a shelved task, and tasks shelve on a task that open tasks depend on, both succeed without a warning; only a later tasks check reports shelved_dep"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-17T22:09:56Z
updated: 2026-10-18T09:43:28Z
depends: []
tags: [feedback, gap, "from:mind6"]
agent: claude-code
---

Why: `dep --on` (and `add`/`edit --depends`, which share `dep::add_dependencies`) accepts a shelved target, and `shelve` accepts a task that open, unshelved tasks depend on. Both print `warnings: []`. Only a later `tasks check` reports `shelved_dep` (src/commands/check.rs). In the meantime, a reviewed idea waiting on the shelved task stays out of the scope queue: an open dependency counts as a wait (wait-aware spec §4.1) and a shelf is open, so nothing surfaces the idea until someone runs check. Priority 3: pre-commit check still catches it.

Original report: Reproduced in a scratch project: shelve B then dep A --on B, and dep C --on D then shelve D, each printed warnings: []; check then reported both. Shelve already refuses a goal with unshelved open descendants but ignores dependents. Meanwhile a reviewed idea waiting on the shelved task stays out of the needs-review queue (a shelf counts as an open wait), so nothing surfaces it until someone runs check. In the case that prompted this, a batch dependency backfill added an edge onto a task shelved a week earlier, and the stranded idea was found only through a pre-commit check warning. A warning at write time naming the dependents (on shelve) or the shelved target (on dep), with check's three remedies (unshelve, drop the edge, shelve the dependent), would catch it when it happens.

Done:
- `add_dependencies` (src/commands/dep.rs) warns when a dependency it adds resolves to a shelved task and the dependent is open and not shelved, the same condition check uses. The warning names the shelved target and the remedies: unshelve it, drop the edge (`tasks dep <id> --rm <dep>`), or shelve the dependent. The write still succeeds. It already resolves each dependency through `resolver.resolve_task`, so this needs no new read.
- `shelve` (src/commands/status.rs) warns after the transition and names every open, unshelved task in this project's scan that depends on the shelved task, matching by canonical id so a retired-prefix spelling counts. It gives the same remedies, and the write still succeeds. Shelve's existing refusal of a goal with unshelved open descendants is unchanged.
- Dependents in other projects are out of scope: `shelve` reads only its own project's scan, and check in the dependent's project still reports them.
- Tests in tests/cli.rs: both orders from the report (shelve B, then dep A --on B; dep C --on D, then shelve D) warn. A shelved or closed dependent gives no warning. Existing dep and shelve outputs are otherwise unchanged.

Where to look: src/commands/dep.rs (`add_dependencies`), src/commands/status.rs (`shelve`), src/commands/check.rs (the `shelved_dep` condition and wording).

## Notes

- 2026-10-18T09:43:28Z (main): scope: scoped; todo, s/low/direct, P3 (check still catches it at pre-commit); warn in dep::add_dependencies, which already resolves each target (covers add/edit --depends too), and in shelve over the local scan; cross-project dependents left to check
