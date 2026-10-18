---
id: tasks-8af396
title: "note --reviewed on an idea with no open dependency, proposal, awaiting-user or deferral is accepted silently, so a 'briefed' verdict whose follow-up edge was forgotten only resurfaces weeks later as stranded; a warning at write time would catch it"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-17T20:36:55Z
updated: 2026-10-18T09:43:57Z
depends: []
tags: [feedback, idea, "from:beliefs"]
agent: claude-code
---

Why: under the wait-aware review state (docs/specs/2026-10-17-wait-aware-review-design.md §4.1), a reviewed idea stays out of the scope queue only while it waits on something recorded: an open dependency, a pending proposal, `awaiting_user`, or a future deferral. A `briefed` verdict whose `dep --on` edge was forgotten waits on nothing. It falls into the `stranded` tier, which scope draws only after every unreviewed and woken idea, and nothing tells the writer at the time: `note --reviewed` and `edit --reviewed` accept it silently (`review::apply`, src/review.rs). The stranded-ideas brief (docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md, option 2) rejected warnings at done, drop and check as the way back into the queue, because a check warning repeats on every commit. This is different: a one-time warning at the write that sets the flag, which no earlier design weighed.

Reported as: "note --reviewed on an idea with no open dependency, proposal, awaiting-user or deferral is accepted silently, so a 'briefed' verdict whose follow-up edge was forgotten only resurfaces weeks later as stranded; a warning at write time would catch it" (from beliefs).

Decided at scoping: a warning, not a refusal. A reviewed idea that waits on nothing is a designed state (`stranded`, spec §4.2), legacy records and editor saves already reach it, and the report asks for a warning.

Done:
- A write that sets `reviewed` (`note --reviewed`, `edit --reviewed`, including `edit --status idea --reviewed`) judges the record after its note is appended, with the same wait test the queue uses: `review::waits`, made callable for one task with a `Deps` view built from the write's resolver, following moved chains as `list::resolve_dependency` does. An unresolvable dependency counts as a wait, as it does in the queue. If nothing waits, the write succeeds and warns: the idea is reviewed but waits on nothing recorded, so it returns to the scope queue as stranded. The warning names the fixes: `tasks dep <id> --on <follow-up>`, `--awaiting-user`, or a deferral.
- No warning on a `drop` audit note that carries `proposal:`, a `question` note with `--awaiting-user`, or a reviewed idea with an open edge or a future `defer`.
- skills/scope/SKILL.md orders a pass so a correct one never sees the warning: a `briefed` member's `dep --on` edges (§5) are recorded before its `--reviewed` audit note (§3).
- Tests in tests/cli.rs: each wait kind gives no warning and the bare case warns. The queue's own tests are unchanged.

Where to look: src/review.rs (`waits`, `apply`), src/commands/status.rs (`note`), src/commands/edit.rs (the `review::apply` call), src/commands/list.rs (`resolve_dependency`, `review_deps`), skills/scope/SKILL.md §3 and §5.

Related: tasks-3704b5 and tasks-d2cdfe cover what a later plain note does to a pending proposal. This warning calls the same `waits`, so it needs no change if that rule changes.
