---
id: tasks-e637a1
title: Design ranked downstream blocker reporting
status: doing
priority: 2
size: s
complexity: high
process: planned
owner: design/tasks-e637a1
created: 2026-10-25T21:31:04Z
updated: 2026-10-25T22:43:13Z
started: 2026-10-25T22:34:30Z
depends: []
parent: tasks-6166d6
tags: [cli]
agent: claude-code
origin: mixed
spec: docs/specs/2026-10-25-blocker-ranking-design.md
---

Why: the user asks to surface and prioritize prerequisites by their direct and indirect downstream impact. [ref:brief] now separates downstream ranking from dependency-history collection (tasks-71c74d). This task designs a ranked report, with existing ready/next order unchanged as the recommended default; the user may override that choice at design review.

Done: produce a focused written design spec for user review, then an implementation plan for user review. Define unique direct/transitive open dependent counts, deterministic report ranking and tie-breaks, goals/closeout presentation, aliases/moves, incomplete/unreachable project coverage and one initial JSON/pretty read surface. Distinguish impacted work from work made ready by completing one blocker when others remain. Reuse tasks-e8d471 and canonical dependency-satisfaction work; design does not require their implementations to have landed. Declare the necessary implementation dependency in the reviewed plan rather than blocking this design.

Prioritize means an explicitly ordered list of blockers, initially by downstream impact with the exact order and tie-breaks stated in the design. Keep a middle alternative visible: show inherited urgency and its beneficiary/cause in ready without re-sorting or changing eligibility. Reuse crew-a0952b's active-beneficiary semantics if that option is adopted; do not invent a second allocator or mutate stored priority. Any picker reordering requires an explicit reviewed choice. Historical age and confirmed-pass counts are not prerequisites for the first ranking report; absent evidence stays unknown and is added only after the collection contract lands.

Verification: a small cross-project graph with a diamond, mixed priorities, multiple blockers, closed/moved/missing targets, a goal-closeout edge and incomplete coverage. Include an unstarted todo with an approval gate only in prose, matching crew-ba0a02: do not report it as definitely agent-actionable. Compare its explicit user-held park (--waiting-on user --reason approval) and a declared-need alternative, including the absence of shared host-local park evidence. Explain that an owner must record the gate before the report can classify it reliably; a need is a resource requirement, not approval. Include the resulting user-only blocker in ranking/explanation tests. No paid calls or changes to that foreign task are part of this work.

Where to look: [ref:brief]; src/commands/show.rs, src/commands/list.rs (resolve_dependency, ready, prime), src/moving/mod.rs (inbound), src/scope.rs, src/query.rs, src/output.rs, src/claims.rs, src/commands/parked.rs and tests/cli.rs; tasks-e8d471, tasks-1bbe49/tasks-606dd3, tasks-298107 and crew-a0952b. New CLI vocabulary still lands in ops before parser changes.

Closeout: record the design/plan result on this task and in the brief; retain accepted report implementation steps as children. Goal tasks-6166d6 closes on shipped report and evidence collection, not this task merely receiving a disposition. The full original capture below is historical input; this revised scope governs the work.

## Original capture (2026-10-25)

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
- 2026-10-25T22:02:30Z (main): scope: scoped; revised the 7085e03 handoff after review round 1; promoted to P2/s/high/planned downstream-ranking design under tasks-6166d6, preserving the entire original capture; removed waits on history tasks-71c74d and reverse-scan tasks-e8d471 so design can proceed independently, with implementation sequencing left to its plan; prioritize explicitly means ranked blockers, unchanged ready order is an overridable recommendation and display-only inherited urgency is an alternative; prose-only approval gate and explicit/unknown park cases required; origin: stage 1 matched claude-code:75e5d0a1-29d8-4f5c-8267-a25b055879ed, user UUID e6c8baae-a7c8-4416-a6d1-bbd718fd15ca and add/result toolu_019n9n6vmJWzyw3xEcaaa8Xy (created 2026-10-25T21:31:04Z); mixed unchanged; stage 2 not applicable; brief: docs/notes/2026-10-25-blocker-ranking-brief.md
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T22:34:30Z (main): started
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T22:34:56Z (design/tasks-e637a1): resumed
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-25T22:35:50Z (design/tasks-e637a1): Process: planned; design in .worktrees/tasks-e637a1 on design/tasks-e637a1. Hydrated with README cargo install --locked --path . confined by --root target/local-install and CARGO_TARGET_DIR=target; no host launcher changed. Full-suite baseline deferred to execution Setup as required for planned work. Origin recovered from the named filing session: user asked for downstream and persistent blocking measures; urgency inheritance and picker changes were agent proposals. Draft will recommend impact-ranked read without scheduler changes and make goal/approval metadata limits explicit.
- 2026-10-25T22:40:50Z (design/tasks-e637a1): Frozen design revision c45fc7f:docs/specs/2026-10-25-blocker-ranking-design.md sent to a read-only builder check; user approval is pending. just test-fast ran 1728 tests and exited 101 with the move integration target failing; no product code changed. Focused move rerun is capturing diagnostics in target/blocker-design-move-check.log. Document pre-commit checks passed.
- 2026-10-25T22:41:40Z (design/tasks-e637a1): review: spec round 2 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-25T22:42:43Z (design/tasks-e637a1): attached: spec-builder-check-report.md (609 bytes): Full read-only builder review of frozen spec c45fc7f; codex/gpt-6.1-sol found nothing important left; user approval pending.
- 2026-10-25T22:42:45Z (design/tasks-e637a1): attached: review-ledger.md (403 bytes): Builder review ledger: accept, no findings or rulings; user design gate remains.
- 2026-10-25T22:42:55Z (design/tasks-e637a1): Verification: focused move target reproduced 87 passed/3 failed from the fixture git helper inheriting host Git configuration; filed tasks-2518ad. With GIT_CONFIG_GLOBAL=/dev/null and GIT_CONFIG_NOSYSTEM=1, the same target passed 90/90, and just test-fast passed 1728 tests (5 ignored). No fixture code was changed and the normal environment remains affected. tasks check and document pre-commit checks passed. Full-suite baseline remains deferred to execution Setup; no new report runtime exists.
- 2026-10-25T22:43:11Z (design/tasks-e637a1): Correction to the preceding verification note: the isolated just test-fast output reports 1728 passed, 0 failed, 4 ignored across 15 test binaries; the earlier 5 ignored was a transcription error.
- 2026-10-25T22:43:13Z (design/tasks-e637a1): retained: {"host": "titan", "kind": "worktree", "path": ".worktrees/tasks-e637a1", "state": "kept", "verification": "verified", "purpose": "Frozen spec review at c45fc7f, then author the separately reviewed plan; includes attached builder-review copies and isolated install/build outputs. No host pointers changed.", "next_owner": "task:tasks-e637a1", "cleanup_trigger": "After user design and plan reviews and final integration, inspect ignored files, preserve remaining evidence, run tt-report and host-pointer checks before removal"}
