---
id: tasks-3c611e
title: closeout lists a goal whose children are closed while its depends are still open
status: done
priority: 2
size: xs
owner: fix/closeout-depends
created: 2026-09-07T21:57:03Z
updated: 2026-09-08T09:54:26Z
depends: []
tags: [feedback, friction, "from:ops", cli]
---

prime's closeout lists a goal with no open children even when its depends are still open, which reads as 'work is all done, confirm and close'. Acting on it is refused: done returns open_dependencies. Reproduced with a goal, one closed child and one open dep.

closeout and ready are sibling predicates and ready already filters on depends (ready_tasks, src/commands/list.rs). closeout is the one that does not.

Fix: exclude a goal whose depends are still open, so closeout keeps its single meaning -- what done will accept -- and emit a prime warning naming the goal and the open dependencies so it does not go silent. Dependencies resolve through resolve_dependency, so an unreachable one counts as open exactly as done's open_deps treats it. No JSON shape change: warnings are already free-form strings, and prime uses that channel for stale claims, uncommitted files, and a newer copy elsewhere.

Definition to update: docs/specs/2026-09-03-task-hierarchy-design.md, the closeout bullet.

## Notes

- 2026-09-08T09:54:26Z (fix/closeout-depends): closeout now applies the dependency gate as well as the children gate, so it holds only what done will accept; a goal its depends still hold is named in a prime warning instead of vanishing. Dependencies resolve through resolve_dependency, so unreachable counts as open exactly as done's open_deps treats it. Verified against the reporting case: ops-500adb, held by four cross-project deps, left closeout and gained the warning. Definition updated in the hierarchy design doc 4.3 and its testing section. No JSON shape change.
