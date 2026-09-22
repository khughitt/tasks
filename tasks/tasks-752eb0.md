---
id: tasks-752eb0
title: "The relay level — match, harness agreement, scoped hints, errors"
status: done
priority: 2
size: m
complexity: mid
process: direct
created: 2026-09-22T14:15:49Z
updated: 2026-09-22T16:16:57Z
completed: 2026-09-22T16:16:57Z
depends: [tasks-d84052, tasks-171677]
parent: tasks-8921f4
tags: []
model: "claude-opus-5[1m]"
agent: "claude-code/claude-opus-5[1m]"
plan: docs/plans/2026-09-22-relay-ancestry-identity.md
step: "Task 4: The relay level — match, harness agreement, scoped hints, errors"
---

## Notes

- 2026-09-22T16:16:57Z (design/relay-ancestry): done
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T16:16:57Z (design/relay-ancestry): The relay level in src/relay/resolve.rs: resolve() takes the ancestry Scope and a snapshot closure, so the registry is opened only once a harness boundary is established. Out of scope returns Ok(None) and the native ladder applies unchanged; unknown ancestry refuses before any registry read. A match needs host, pid, starttime, a linux handle, the current boot id and a harness agreeing with the ancestor comm; non-qualifying rows are excluded before cardinality so a stale or wrong-harness row never fakes an ambiguity. Session hints are scoped to the nearest harness, so a nested Codex session ignores an inherited CLAUDE_CODE_SESSION_ID, and same_session compares a claim's session across its raw, tagged and agent-id forms.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
