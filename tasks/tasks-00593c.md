---
id: tasks-00593c
title: "Relay-on end-to-end tests the relay identity design listed but the suite lacks: successful takeover, same-status release retry, operation without Node, plain shell with a corrupt registry"
status: todo
priority: 3
size: s
complexity: mid
process: direct
created: 2026-10-18T20:55:28Z
updated: 2026-10-18T20:55:28Z
depends: []
tags: [testing]
agent: claude-code/claude-opus-5-5
---

Why: relay's v0.1.0 verification map (relay docs/runbooks/release.md) checked the tasks consumer clause of relay's spec ('Required verification for the release', ops docs/specs/2026-09-16-relay-design.md) against tasks' tests. With relay identity on, four sub-clauses lack end-to-end evidence: (1) a successful explicit or stale takeover of a relay-acquired claim (only the refusal side is tested: an_acceptance_force_cannot_take_over_without_a_resolved_identity); (2) same-status release retries (done_retries_a_failed_release_after_a_status_edit and cleanup_retry_… run with relay off); (3) operation with no Node present (true by construction: src/relay spawns nothing, and tests write the registry with a shell function); (4) a plain shell with relay on and a missing or corrupt registry file (unit-level only: a_plain_chain_never_reads_an_executable_or_arguments, a_match_is_not_attempted_out_of_scope). Done: an end-to-end test for each, or a ruling recorded here that the unit evidence suffices.
