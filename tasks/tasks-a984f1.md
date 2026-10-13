---
id: tasks-a984f1
title: Establish safe note delivery to a task active in another checkout
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-08T09:43:57Z
updated: 2026-10-13T09:21:39Z
depends: []
parent: tasks-984769
tags: []
source: tasks-955c2a
agent: codex
---

Question: Can an explicitly targeted note in the current holder's checkout safely record another session's finding using the existing CLI and shared mutation lock, or does the occupied-checkout workflow need a delivery mechanism?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/commands/status.rs::note, src/commands/mod.rs::open_id_write_ctx/load/refuse_stale_copy/occupants/follow_holder, src/repo.rs::write_task, src/stale.rs::refusal, record-home spec sections 3 and 8; tests/cli.rs::concurrent_notes_and_a_status_change_lose_nothing and a_note_from_a_foreign_session_leaves_the_claim_alone.

Bound: One disposable main/worktree pair with isolated XDG_CONFIG_HOME and XDG_STATE_HOME, two explicit session identities and a live owner claim. Record refusal from the stale checkout without byte changes; explicitly target the current checkout with -C and compare record bytes and claim before/after. Exercise concurrent owner CLI writes and a foreign note under the existing lock, then retry an already-landed note to document duplicate behavior. State the limits for editor/Git writes outside that lock and another host's state. Compare coordinated explicit targeting with the report's queued-delivery proposal; do not write into a real session's checkout, weaken stale_copy, implement a queue, or grant automatic routing.

Expected result: Attach a reproducible transcript and recommend supported guidance or a narrowly stated contract gap; update the brief. Explain why whole-record serialization matters despite append-only notes. If a queue is necessary, identify durability, retry/deduplication, absent-owner delivery, visibility and host-sync questions before filing planned design. Coordinate any routing recommendation with tasks-10c968, without expanding halt-audit scope. P2/s/mid/direct: bounded evidence collection with fixed checks; no product implementation.

Ideas it wakes: On completion, run tasks note tasks-955c2a with the finding before the result's branch merges, update the brief, and re-scope that idea.
