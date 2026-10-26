# Spec round 3 review

Reviewer identity supplied with the review: claude-code/claude-opus-5-5.
Frozen revision: tasks repository, c45fc7f:docs/specs/2026-10-25-blocker-ranking-design.md.
The following is the full review received from the user.

I'd send this back for revision. The direction is right: a separate `tasks blockers` read that leaves ready/next alone, and keeps "this would clear its dependencies" apart from "ready". But three problems would ship misleading output or stall delivery, so it shouldn't be approved as it stands. I reviewed the frozen revision c45fc7f (the worktree copy is identical to it) and checked its claims about the code against the source. I wrote nothing and changed no task records. The approval decision is still yours, and once you've given it, the task's owner records the review note and revises.

For the review note: `review: spec round 3 — verdict: revise; findings: Important 3, Minor 5; reviewer: claude-code/claude-opus-5-5`. That number assumes the round-2 builder check counts as a round.

## What checks out against the code

- **Which dependencies count as met:** `resolve_dependency` (`src/commands/list.rs:21`) canonicalizes the ID, then follows move records. `ready_tasks` tests `!status.is_open()`, and `is_open` excludes done, dropped and moved. So done and dropped satisfy and shelved does not, as the spec says.
- **Goals:** `is_goal(task, has_children) = has_children || task.lane` (`src/hierarchy.rs:222`). A parent must be in the same project as its child (`validate_parent`). So the spec is right to set `goal: null` for a target outside the scan unless its lane flag is set.
- **Parks and scope arguments:** the `WaitingOn` and `Reason` values the spec names exist, `ParkInfo` has the fields it relies on, and the scope arguments are as described.
- **The worked example:** the counts and the order B, R, X, L are correct.

## Important

1. **`coverage.complete` overstates coverage when a path runs through another project's task** (spec lines 86–91 and 142–149).
   - The spec deliberately doesn't follow the outgoing edges of tasks outside the scan. So a local-scope read misses dependents that are themselves in scope.
   - Example: in-scope `aa-A` depends on foreign `bb-X`, and `bb-X` depends on in-scope `aa-Y`. Under aa scope, `aa-Y` gets no row at all, because it has no direct dependent in scope. Yet `complete` is true, because aa was scanned and every reference resolved.
   - The same gap applies under `--group`.
   - Fix, either way:
     - Walk the outgoing edges of out-of-scope targets as uncounted intermediate hops. They're already loaded, and resolution is memoized, so it's cheap.
     - Or define `complete` as false whenever such a target's own dependencies weren't followed.
   - The fixture also needs this case. The retained case at line 280 says "Local aa scope ... a foreign target of a local dependency still gets a boundary row". But no aa record depends on a bb record, so that case can't happen as written. bb scope, or an extra edge, would exercise it.

2. **Rows don't say whether the blocker is itself blocked.**
   - In the worked example, L ranks 4th and R 2nd, but nothing on those rows shows they're waiting on B.
   - The success criterion is to "distinguish dependency relief from permission to execute". A blocker that can't start because of its own unmet dependencies is the most basic case of that, and you can't see it today.
   - Add a count of the row's own unmet dependencies (or a `root` flag) to JSON and pretty output, without making it a ranking input. The resolution it needs is already computed.

3. **It's unclear whether the report has to wait for tasks-e90b4f** (spec lines 62–66 and 307–311).
   - Lines 62–66 say a shared internal check is enough and the report needn't pick tasks-e90b4f's JSON shape. The handoff section then says to "sequence ... tasks-e90b4f before report graph integration".
   - tasks-e90b4f is an unstarted design task with the planned process. If the plan reads the handoff literally, the report waits on that whole design, plan and build.
   - State that this work may pull the existing rule out into one shared internal function that tasks-e90b4f later reuses. Only a public contract would be waited on.

## Minor

- **Existing places that judge whether dependencies are met aren't listed.**
  - `review::Deps::of_scan` (`src/review.rs:40`) treats an unresolvable reference as Open, where `ready` treats it as unknown.
  - `commands/mod.rs::open_deps` also exists.
  - The shared function has to reconcile both, so name them under "Existing contracts".
- **Depending on a goal has an unstated consequence.** When a task depends on a goal, the goal's open children are the real blockers, but they get no credit because parent/child edges aren't added. The decision is fine; the spec should say what it costs.
- **`awaiting_user` is narrower than line 171 suggests.** `review.rs:156` only allows it on reviewed ideas, so it's only a user-attention signal for blockers that are reviewed ideas. Say so.
- **The row name `needs` collides with `park.needs`.** `ParkInfo.needs` holds the host-capability `Needs` values (idle/headless), not the task's needs list. One row would carry two different things called `needs`.
- **Live claims aren't shown at all.** Keeping them out of ranking and authorization is right. But printing who holds the blocker ("someone is on it") costs little and helps you decide whether it needs you.

## For you to decide

The ranking counts shelved and idea dependents the same as todo/doing ones (lines 109–111). So a blocker holding up ten shelved ideas outranks one holding up three active tasks. Whether that matches "holding up the most work" is your call. I'd keep the ranking key as it is and add a per-status breakdown (active vs. shelved/idea) to each row, rather than adding status filters.

## Smaller wording issues

These are optional:
- Line 59 ("No park is evidence of no recorded park on this host") is hard to parse.
- The pretty table has ten columns before the title. The plan could set an expected width.

Nothing was pushed or merged. `.worktrees/tasks-e637a1` is unchanged, and the task is still parked waiting on your review.
