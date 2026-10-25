---
id: tasks-e637a1
title: "Surface and rank blockers: fan-out of open dependents, and how long and how many scope passes each has held work"
status: idea
priority: 2
size: m
complexity: high
process: planned
reviewed: true
reviewed_at: 2026-10-25T21:38:18Z
created: 2026-10-25T21:31:04Z
updated: 2026-10-25T21:58:22Z
depends: [tasks-e8d471, tasks-71c74d]
parent: tasks-6166d6
tags: [cli]
agent: claude-code
origin: mixed
---

Why: nothing surfaces which open tasks are holding up the most work, or which have held work up longest. A crew scope pass on 2026-10-25 re-briefed crew-1c3624 (waits on crew-8b52fb, autonomy-1aac03) and crew-9b4b47 (waits on crew-ba0a02) and changed nothing; the blockers themselves were never surfaced for action.

User's ask (2026-10-25): surface and prioritize blockers, measured two ways:
- tasks with many downstream dependents, direct or indirect;
- tasks that have blocked others for a long time and/or across several scoping iterations.

State today (checked against main 30c347e):
- `show` lists outgoing `depends` only; incoming edges are tasks-e8d471 (todo, scoped). This builds on its dependents scan, including the cross-project read.
- `graph` renders one project's edges as mermaid/dot; no counts, and cross-project edges dangle.
- `ready` sorts by priority then size; a P2 task gating P1 work ranks as P2.
- `depends` is a bare id list and `tasks dep` writes no note, so when an edge appeared is recorded only in git history.
- The "still waits on X" signal exists only as free text in scope notes.

Agent additions to weigh when scoping:
- Edge age needs a recorded time: a lifecycle note on `dep --on` (forward only), with git history as an optional backfill.
- Scoping iterations: count `scope:` notes on a dependent written after its edge appeared while the blocker stayed open.
- Effective priority: a blocker inherits the highest priority among its open transitive dependents (priority inversion).
- Classify the blocker's own state: unstarted todo, parked on the user (approval, spending), shelved, or idea. A blocker held by a user decision belongs in the person's queue.
- Exclude or separate goal-closeout edges (a hub goal depending on its cross-project pieces), which inflate fan-out without being real blockage.
- Surface: a `tasks blockers` view (or `--sort unblocks` on list/ready) plus a short section in `prime`; any CLI change starts in ops's cli.toml.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — originating ask, verified premises and blocker-report design questions

## Notes

- 2026-10-25T21:31:04Z (add): created
  provenance: {"harness_session":"claude-code:75e5d0a1-29d8-4f5c-8267-a25b055879ed","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"mixed"}
- 2026-10-25T21:38:18Z (main): scope: briefed; checked c8de557: incoming dependents still tasks-e8d471, ready orders by declared priority/size, dependency writers share add_dependencies but record no edge timestamps; full capture retained; advisory report recommended over picker reordering or duplicate allocator urgency (crew-a0952b); parented under tasks-6166d6 inside tasks-46d207, P2/m/high/planned; filed tasks-71c74d (P2/s/high/planned) for graph, age/pass evidence and read-contract design, with wait edge and completion wake; existing tasks-e8d471 wait retained, design need not await it; origin: stage 1 matched creation in claude-code:75e5d0a1-29d8-4f5c-8267-a25b055879ed, user ask line 17, unique add call/result lines 115-116 with created at 2026-10-25T21:31:04Z, proposals distinguished by line 132; mixed unchanged; stage 2 not applicable (no origin_session); brief: docs/notes/2026-10-25-blocker-ranking-brief.md
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T21:58:22Z (main): review: spec round 1 — verdict: revise; findings: ungraded 5, Minor 2; reviewer: claude-opus-5.5
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID"}
