---
id: tasks-c87f7d
title: "The park command returns success when park metadata persistence fails, leaving no park entry or released claim; clients must verify the postcondition to avoid falsely reporting completion."
status: done
priority: 2
size: s
complexity: low
process: direct
owner: fix/park-store-failure
created: 2026-10-14T16:18:01Z
updated: 2026-10-17T03:05:12Z
started: 2026-10-17T03:00:05Z
completed: 2026-10-17T03:05:12Z
depends: []
tags: [feedback, gap, "from:crew"]
agent: codex
---

Original report (from:crew): the park command returns success when park metadata persistence fails, leaving no park entry or released claim; clients must verify the postcondition to avoid falsely reporting completion.

Why: confirmed in src/commands/mod.rs (save, `ClaimIntent::Park` arm, ~line 1791). The record and note are written first, and a failed `store.save()` then only pushes the warning "the note landed, but parking on <id> was not updated (...)" and returns Ok, so `tasks park` exits 0 with no park entry and the claim still held. Only the escalation case already fails. This follows park design docs/specs/2026-09-09-park-design.md §3 (lines ~69-71). The later claim-release design (docs/specs/2026-10-15-claim-release-design.md §4) names this as the silent-success shape not to repeat, and sets the replacement: fail with the store's own error (kind `io`) carrying a suffix that the note landed and naming the rerun. A rerun appends a second note, which that design accepts. `release` implements it (src/commands/release.rs, `landed()`).

Done: in the Park arm, a store save failure is the command's failure for every park: the store error with a suffix like "; the note on <id> landed but it was not parked; rerun the same `tasks park` command". The escalation message keeps its rating detail inside the same shape. Exit 1, nothing reported as success. Amend park design §3 to match and cite claim-release §4. Update skills/tasks/SKILL.md (line ~166 already tells the escalation case to rerun) so it covers every park.

Check: an integration test in tests/cli.rs modelled on `release_store_failure_after_the_note_names_the_rerun` (~line 29577) and the read-only state-dir pattern in `done_retries_a_failed_release_after_a_status_edit` (~line 11063): park with the claim store's directory at 0o500 exits non-zero with kind `io` and the rerun suffix, the note is present, and after restoring permissions the rerun parks. `just test-fast`.

## Notes

- 2026-10-17T02:37:36Z (main): scope: scoped; todo P2 s/low/direct; confirmed the warning-and-exit-0 path in save's Park arm at 721c87a; claim-release design §4 already fixes the replacement shape (io error + landed/rerun suffix, as release does); body names the test pattern
- 2026-10-17T03:00:05Z (main): started
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T03:00:17Z (fix/park-store-failure): resumed
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T03:05:12Z (fix/park-store-failure): done
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T03:05:12Z (fix/park-store-failure): park fails with the store's io error and a 'landed … rerun the same tasks park command' suffix on every store save failure, escalation included; park design §3 and complexity §5.2 amended, skill covers every park; tests in tests/cli.rs
  provenance: {"harness_session":"claude-code:c3a7dc2a-af0a-4a67-b54c-66a822fad9ae","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
