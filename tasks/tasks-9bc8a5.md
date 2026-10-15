---
id: tasks-9bc8a5
title: Cover edit and the note heartbeat in the sole-worker-row relay control
status: todo
priority: 4
size: xs
complexity: low
process: direct
created: 2026-10-15T12:41:35Z
updated: 2026-10-15T12:41:35Z
depends: []
tags: [relay]
source: tasks-af3aac
agent: claude-code/claude-opus-5-5
---

delegated_relay_sole_worker_row_needs_its_thread (tests/cli.rs) asserts start/park/done refuse byte-identically when a lone Codex worker row qualifies without its thread; delegated_relay_ownership also covers protected edits and the stamped-note heartbeat for the root-row case. Add refuse edit {held} --status done and --need, and a note --stamp asserting the 'not refreshed' warning with the claim store unchanged, in each of the absent / session-only / matching-session modes. Deferred minor from tasks-af3aac's final review.
